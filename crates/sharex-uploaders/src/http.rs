//! The HTTP layer under `Uploader`: what `HttpClientFactory` (HelpersLib),
//! `Uploader.CreateRequest`/`GetResponseAsync`, `ProgressStreamContent`,
//! `MultipartFormDataContent` and `ProgressManager` do in ShareX, on ureq.
//!
//! Requests go out as .NET's `HttpClient` sends them: ShareX's user agent
//! and `Cache-Control: no-cache` on every request, a `Content-Length` (never
//! chunked) for every body, multipart bodies laid out byte for byte as
//! `MultipartContent` writes them, URLs escaped as `System.Uri` escapes them,
//! no `Accept`/`Accept-Encoding`, no cookies kept, redirects followed (up to
//! 50), 30 s to connect and no overall timeout unless the uploader sets one.
//!
//! Requests run on a worker thread so a stop ([`CancellationToken`]) returns
//! at once, as cancelling `HttpClient.SendAsync` does; the upload body also
//! stops reading as soon as it's cancelled.

use std::collections::VecDeque;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};

use crate::enums::HttpMethod;
pub use crate::proxy::{ProxyInfo, ProxyPlan};

/// `CRLF`, as `Environment.NewLine` on Windows and in HTTP.
const CRLF: &str = "\r\n";

/// `RequestHelpers` content types.
pub const CONTENT_TYPE_MULTIPART_FORM_DATA: &str = "multipart/form-data";
pub const CONTENT_TYPE_JSON: &str = "application/json";
pub const CONTENT_TYPE_XML: &str = "application/xml";
pub const CONTENT_TYPE_URL_ENCODED: &str = "application/x-www-form-urlencoded";
pub const CONTENT_TYPE_OCTET_STREAM: &str = "application/octet-stream";

// ---------------------------------------------------------------------------
// Settings shared by every request (`HttpClientFactory`, `ShareXResources`).

/// Rewrites a request's absolute URL just before it's sent, and a
/// response's URL back. Only for tests and the validation harness, which
/// point every destination at a local mock server; `None` in the app.
#[derive(Clone)]
pub struct UrlRewriter {
    pub forward: Arc<dyn Fn(&str) -> String + Send + Sync>,
    pub back: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

struct HttpSettings {
    user_agent: String,
    proxy: ProxyPlan,
    rewriter: Option<UrlRewriter>,
}

static SETTINGS: LazyLock<RwLock<HttpSettings>> = LazyLock::new(|| {
    RwLock::new(HttpSettings { user_agent: default_user_agent(), proxy: ProxyPlan::Direct, rewriter: None })
});

/// `ShareXResources.UserAgent` is `ShareX/<version>`; ShareXrs identifies
/// itself as `ShareXrs/<version>` rather than present as ShareX to third
/// parties. A `.sxcu` that needs another can set its own `User-Agent`.
pub fn default_user_agent() -> String {
    format!("ShareXrs/{}", env!("CARGO_PKG_VERSION"))
}

/// The user agent requests send (`ShareXResources.UserAgent`).
pub fn user_agent() -> String {
    SETTINGS.read().expect("http settings").user_agent.clone()
}

/// Changes the user agent every request sends.
pub fn set_user_agent(user_agent: impl Into<String>) {
    SETTINGS.write().expect("http settings").user_agent = user_agent.into();
    reset_agents();
}

/// `HelpersOptions.CurrentProxy = settings.ProxySettings`: where every
/// request goes ([`ProxyInfo::plan`]). Until it's set (tests, harnesses)
/// requests connect directly.
pub fn set_proxy(proxy: &ProxyInfo) {
    let plan = proxy.plan();
    let mut settings = SETTINGS.write().expect("http settings");
    if settings.proxy != plan {
        settings.proxy = plan;
        drop(settings);
        reset_agents();
    }
}

/// See [`UrlRewriter`].
pub fn set_url_rewriter(rewriter: Option<UrlRewriter>) {
    SETTINGS.write().expect("http settings").rewriter = rewriter;
}

/// An agent, keyed by its redirect setting and proxy URI.
type AgentEntry = (bool, Option<String>, ureq::Agent);

/// The agents made so far, by redirect setting and proxy.
static AGENTS: LazyLock<Mutex<Vec<AgentEntry>>> = LazyLock::new(|| Mutex::new(Vec::new()));

fn reset_agents() {
    AGENTS.lock().expect("agents").clear();
}

/// `HttpClientFactory.Create(allowAutoRedirect, infiniteTimeout: true)`:
/// one shared client per redirect setting, and (ureq takes one proxy per
/// agent) per proxy the system picks for the URL.
fn agent(allow_auto_redirect: bool, proxy_uri: Option<String>) -> ureq::Agent {
    let mut agents = AGENTS.lock().expect("agents");
    if let Some((_, _, a)) = agents.iter().find(|(r, p, _)| *r == allow_auto_redirect && *p == proxy_uri) {
        return a.clone();
    }
    if agents.len() >= 16 {
        agents.clear();
    }
    let proxy = proxy_uri.as_deref().and_then(|u| ureq::Proxy::new(u).ok());
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(if allow_auto_redirect { 50 } else { 0 })
        .max_redirects_will_error(false)
        .save_redirect_history(true)
        .timeout_connect(Some(Duration::from_secs(30)))
        .user_agent(ureq::config::AutoHeaderValue::None)
        .accept(ureq::config::AutoHeaderValue::None)
        .accept_encoding(ureq::config::AutoHeaderValue::None)
        .allow_non_standard_methods(true)
        .proxy(proxy)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build();
    let a: ureq::Agent = config.into();
    agents.push((allow_auto_redirect, proxy_uri, a.clone()));
    a
}

// ---------------------------------------------------------------------------
// Cancellation and progress.

/// Stops an operation: `CancellationTokenSource` / `Uploader.StopUpload`.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// `ProgressManager`: bytes sent, speed (averaged over the last 10 samples
/// of at least 250 ms) and the time left.
#[derive(Debug, Clone)]
pub struct ProgressManager {
    pub position: u64,
    pub length: u64,
    /// Bytes per second.
    pub speed: f64,
    start: Instant,
    stopped: Option<Duration>,
    smooth: Instant,
    speed_test: u64,
    average_speed: VecDeque<f64>,
}

impl ProgressManager {
    pub fn new(length: u64, position: u64) -> Self {
        let now = Instant::now();
        Self {
            position,
            length,
            speed: 0.0,
            start: now,
            stopped: None,
            smooth: now,
            speed_test: 0,
            average_speed: VecDeque::with_capacity(10),
        }
    }

    /// `Percentage`.
    pub fn percentage(&self) -> f64 {
        self.position as f64 / self.length as f64 * 100.0
    }

    /// `Elapsed`.
    pub fn elapsed(&self) -> Duration {
        self.stopped.unwrap_or_else(|| self.start.elapsed())
    }

    /// `Remaining`.
    pub fn remaining(&self) -> Duration {
        if self.speed > 0.0 {
            Duration::from_secs_f64(self.length.saturating_sub(self.position) as f64 / self.speed)
        } else {
            Duration::ZERO
        }
    }

    /// `UpdateProgress`: true when it's worth reporting (finished, or
    /// 250 ms since the last report).
    pub fn update_progress(&mut self, bytes_read: u64) -> bool {
        self.position += bytes_read;
        self.speed_test += bytes_read;
        if self.position >= self.length {
            self.position = self.length;
            self.stopped.get_or_insert_with(|| self.start.elapsed());
            return true;
        }
        let since = self.smooth.elapsed();
        if since.as_millis() > 250 {
            if self.average_speed.len() == 10 {
                self.average_speed.pop_front();
            }
            self.average_speed.push_back(self.speed_test as f64 / since.as_secs_f64());
            self.speed = self.average_speed.iter().sum::<f64>() / self.average_speed.len() as f64;
            self.speed_test = 0;
            self.smooth = Instant::now();
            return true;
        }
        false
    }
}

/// `Uploader.ProgressChanged`.
pub type ProgressCallback = Arc<dyn Fn(&ProgressManager) + Send + Sync>;

// ---------------------------------------------------------------------------
// Request bodies.

/// A seekable upload source (ShareX requires seekable streams so it can
/// send their length).
pub trait UploadStream: Read + Seek + Send {}
impl<T: Read + Seek + Send> UploadStream for T {}

/// Shared ownership of an upload stream: the body reads it on the worker
/// thread while the uploader keeps it for retries.
pub type SharedStream = Arc<Mutex<Box<dyn UploadStream>>>;

/// Wraps a stream for [`Content::Stream`].
pub fn shared_stream(stream: impl UploadStream + 'static) -> SharedStream {
    Arc::new(Mutex::new(Box::new(stream)))
}

/// The length of a stream (`Stream.Length`).
pub fn stream_length(stream: &SharedStream) -> io::Result<u64> {
    let mut s = stream.lock().expect("upload stream");
    let pos = s.stream_position()?;
    let len = s.seek(SeekFrom::End(0))?;
    s.seek(SeekFrom::Start(pos))?;
    Ok(len)
}

/// A slice of a stream sent with progress (`ProgressStreamContent`).
#[derive(Clone)]
pub struct StreamPart {
    pub source: SharedStream,
    pub position: u64,
    pub length: u64,
    /// The whole stream's length, for the progress total.
    pub total_length: u64,
    pub buffer_size: usize,
    /// Called with each read's byte count (S3's multipart progress,
    /// `SharedStreamSegment`'s callback).
    pub on_read: Option<Arc<dyn Fn(u64) + Send + Sync>>,
}

/// One part of a multipart body: its headers (in order) and content.
#[derive(Clone)]
pub struct MultipartPart {
    pub headers: Vec<(String, String)>,
    pub body: PartBody,
}

#[derive(Clone)]
pub enum PartBody {
    Bytes(Vec<u8>),
    Stream(StreamPart),
}

/// A request body: `HttpContent` and its content headers.
#[derive(Clone)]
pub enum Content {
    /// `ByteArrayContent`/`StringContent`.
    Bytes { data: Vec<u8>, content_type: Option<String>, headers: Vec<(String, String)> },
    /// `ProgressStreamContent`.
    Stream { part: StreamPart, content_type: Option<String>, headers: Vec<(String, String)> },
    /// `MultipartContent` / `MultipartFormDataContent`.
    Multipart { content_type: String, boundary: String, parts: Vec<MultipartPart>, headers: Vec<(String, String)> },
}

impl Content {
    /// `new ByteArrayContent(bytes)` with an optional content type.
    pub fn bytes(data: Vec<u8>, content_type: Option<&str>) -> Self {
        Content::Bytes { data, content_type: content_type.filter(|c| !c.trim().is_empty()).map(str::to_owned), headers: Vec::new() }
    }

    /// The `Content-Type` value as .NET writes it.
    pub fn content_type(&self) -> Option<String> {
        match self {
            Content::Bytes { content_type, .. } | Content::Stream { content_type, .. } => content_type.clone(),
            Content::Multipart { content_type, boundary, .. } => Some(format!("{content_type}; boundary=\"{boundary}\"")),
        }
    }

    /// Other content headers (`Content-Range`, a custom `Content-*`).
    pub fn extra_headers_mut(&mut self) -> &mut Vec<(String, String)> {
        match self {
            Content::Bytes { headers, .. } | Content::Stream { headers, .. } | Content::Multipart { headers, .. } => headers,
        }
    }

    fn extra_headers(&self) -> &[(String, String)] {
        match self {
            Content::Bytes { headers, .. } | Content::Stream { headers, .. } | Content::Multipart { headers, .. } => headers,
        }
    }

    /// `TryComputeLength`.
    pub fn length(&self) -> u64 {
        match self {
            Content::Bytes { data, .. } => data.len() as u64,
            Content::Stream { part, .. } => part.length,
            Content::Multipart { boundary, parts, .. } => {
                let mut len = 0u64;
                for (i, p) in parts.iter().enumerate() {
                    len += multipart_part_header(boundary, i, &p.headers).len() as u64;
                    len += match &p.body {
                        PartBody::Bytes(b) => b.len() as u64,
                        PartBody::Stream(s) => s.length,
                    };
                }
                len + multipart_footer(boundary).len() as u64
            }
        }
    }
}

/// `MultipartContent`'s start boundary or divider, then a part's headers.
fn multipart_part_header(boundary: &str, index: usize, headers: &[(String, String)]) -> Vec<u8> {
    let mut s = String::new();
    if index == 0 {
        s.push_str(&format!("--{boundary}{CRLF}"));
    } else {
        s.push_str(&format!("{CRLF}--{boundary}{CRLF}"));
    }
    for (name, value) in headers {
        s.push_str(&format!("{name}: {value}{CRLF}"));
    }
    s.push_str(CRLF);
    // ShareX's `HeaderEncodingSelector` writes part headers as UTF-8.
    s.into_bytes()
}

fn multipart_footer(boundary: &str) -> Vec<u8> {
    format!("{CRLF}--{boundary}--{CRLF}").into_bytes()
}

/// `RequestHelpers.CreateBoundary`: 20 dashes and a GUID's 32 hex digits.
pub fn create_boundary() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("the OS random number generator");
    // A version 4 GUID, as Guid.NewGuid.
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}{hex}", "-".repeat(20))
}

/// Reads a body out as .NET serialises it, reporting progress for stream
/// parts and stopping on cancellation.
struct BodyReader {
    chunks: VecDeque<Chunk>,
    current: Option<Chunk>,
    cancel: CancellationToken,
    progress: Option<(Arc<Mutex<ProgressManager>>, ProgressCallback)>,
}

enum Chunk {
    Bytes(Vec<u8>, usize),
    Stream { part: StreamPart, remaining: u64, started: bool },
}

impl BodyReader {
    fn new(content: &Content, cancel: CancellationToken, progress: Option<ProgressCallback>) -> Self {
        let mut chunks = VecDeque::new();
        let mut total: Option<(u64, u64)> = None;
        let mut push_stream = |chunks: &mut VecDeque<Chunk>, part: &StreamPart| {
            total.get_or_insert((part.total_length, part.position));
            chunks.push_back(Chunk::Stream { part: part.clone(), remaining: part.length, started: false });
        };
        match content {
            Content::Bytes { data, .. } => chunks.push_back(Chunk::Bytes(data.clone(), 0)),
            Content::Stream { part, .. } => push_stream(&mut chunks, part),
            Content::Multipart { boundary, parts, .. } => {
                for (i, p) in parts.iter().enumerate() {
                    chunks.push_back(Chunk::Bytes(multipart_part_header(boundary, i, &p.headers), 0));
                    match &p.body {
                        PartBody::Bytes(b) => chunks.push_back(Chunk::Bytes(b.clone(), 0)),
                        PartBody::Stream(s) => push_stream(&mut chunks, s),
                    }
                }
                chunks.push_back(Chunk::Bytes(multipart_footer(boundary), 0));
            }
        }
        let progress = match (progress, total) {
            (Some(cb), Some((length, position))) => Some((Arc::new(Mutex::new(ProgressManager::new(length, position))), cb)),
            _ => None,
        };
        Self { chunks, current: None, cancel, progress }
    }
}

impl Read for BodyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.cancel.is_cancelled() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "The operation was canceled."));
            }
            if self.current.is_none() {
                self.current = self.chunks.pop_front();
            }
            let Some(chunk) = self.current.as_mut() else { return Ok(0) };
            match chunk {
                Chunk::Bytes(data, pos) => {
                    if *pos >= data.len() {
                        self.current = None;
                        continue;
                    }
                    let n = buf.len().min(data.len() - *pos);
                    buf[..n].copy_from_slice(&data[*pos..*pos + n]);
                    *pos += n;
                    return Ok(n);
                }
                Chunk::Stream { part, remaining, started } => {
                    if *remaining == 0 {
                        self.current = None;
                        continue;
                    }
                    let mut source = part.source.lock().expect("upload stream");
                    if !*started {
                        source.seek(SeekFrom::Start(part.position))?;
                        *started = true;
                    }
                    let want = buf.len().min(part.buffer_size.max(1)).min(*remaining as usize);
                    let n = source.read(&mut buf[..want])?;
                    drop(source);
                    if n == 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "The upload stream ended before the declared content length was reached.",
                        ));
                    }
                    *remaining -= n as u64;
                    if let Some(on_read) = &part.on_read {
                        on_read(n as u64);
                    }
                    if let Some((pm, cb)) = &self.progress {
                        let mut pm = pm.lock().expect("progress");
                        if pm.update_progress(n as u64) {
                            cb(&pm);
                        }
                    }
                    return Ok(n);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// URLs, as System.Uri sends them.

/// The request target .NET sends for `url`: `System.Uri`'s escaped
/// `AbsoluteUri` without the fragment (spaces, quotes, `<>^\`{|}` and
/// non-ASCII escaped, existing `%XX` kept, `\` as `/` in the path, dot
/// segments removed, scheme and host in lower case, default port dropped).
pub fn dotnet_absolute_uri(url: &str) -> Result<String, String> {
    let url = url.trim_matches(|c: char| c.is_whitespace() || c.is_control());
    let colon = url.find(':').ok_or_else(|| "Invalid URI: The format of the URI could not be determined.".to_owned())?;
    let scheme = url[..colon].to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https") {
        return Err(format!("The '{scheme}' scheme is not supported."));
    }
    let rest = url[colon + 1..].trim_start_matches(['/', '\\']);
    let end = rest.find(['/', '\\', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let mut tail = &rest[end..];
    if authority.is_empty() {
        return Err("Invalid URI: The hostname could not be parsed.".to_owned());
    }
    let (userinfo, host_port) = match authority.rfind('@') {
        Some(i) => (Some(&authority[..i]), &authority[i + 1..]),
        None => (None, authority),
    };
    let (host, port) = if host_port.starts_with('[') {
        let close = host_port.find(']').ok_or_else(|| "Invalid URI: The hostname could not be parsed.".to_owned())?;
        (&host_port[..=close], host_port[close + 1..].strip_prefix(':'))
    } else {
        match host_port.rfind(':') {
            Some(i) => (&host_port[..i], Some(&host_port[i + 1..])),
            None => (host_port, None),
        }
    };
    let port = match port.filter(|p| !p.is_empty()) {
        Some(p) => {
            let n: u16 = p.parse().map_err(|_| "Invalid URI: Invalid port specified.".to_owned())?;
            let default = if scheme == "http" { 80 } else { 443 };
            if n == default { String::new() } else { format!(":{n}") }
        }
        None => String::new(),
    };
    let fragment_at = tail.find('#');
    if let Some(i) = fragment_at {
        tail = &tail[..i];
    }
    let (path, query) = match tail.find('?') {
        Some(i) => (&tail[..i], Some(&tail[i + 1..])),
        None => (tail, None),
    };
    // IRI parsing kicks in for the whole URI when it holds any non-ASCII
    // character, raw or as escaped UTF-8 (in .NET's IRI ranges).
    let iri = url.bytes().any(|b| b >= 0x80) || has_escaped_iri_char(path, false) || query.is_some_and(|q| has_escaped_iri_char(q, true));
    let path = escape_uri_part(&path.replace('\\', "/"), iri, false);
    let path = remove_dot_segments(if path.is_empty() { "/" } else { &path });
    let mut out = format!("{scheme}://");
    if let Some(u) = userinfo {
        out.push_str(&escape_uri_part(u, iri, false));
        out.push('@');
    }
    out.push_str(&host.to_lowercase());
    out.push_str(&port);
    out.push_str(&path);
    if let Some(q) = query {
        out.push('?');
        out.push_str(&escape_uri_part(q, iri, true));
    }
    Ok(out)
}

fn hex_at(bytes: &[u8], i: usize) -> Option<u8> {
    if i + 3 > bytes.len() || bytes[i] != b'%' {
        return None;
    }
    let h = (bytes[i + 1] as char).to_digit(16)?;
    let l = (bytes[i + 2] as char).to_digit(16)?;
    Some((h * 16 + l) as u8)
}

/// `IriHelper.CheckIriUnicodeRange`.
fn is_iri_char(c: char, is_query: bool) -> bool {
    let u = c as u32;
    match u {
        0xA0..=0xD7FF | 0xF900..=0xFDCF | 0xFDF0..=0xFFEF => true,
        0xE000..=0xF8FF => is_query,
        0x10000..=0xEFFFF => (u & 0xFFFF) <= 0xFFFD,
        0xF0000..=0x10FFFF => is_query && (u & 0xFFFF) <= 0xFFFD,
        _ => false,
    }
}

/// An escaped UTF-8 sequence for an IRI character at `i`: its length in
/// source bytes (3 per escaped byte).
fn escaped_iri_char_at(bytes: &[u8], i: usize, is_query: bool) -> Option<usize> {
    let lead = hex_at(bytes, i)?;
    let n = match lead {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return None,
    };
    let mut buf = Vec::with_capacity(n);
    for k in 0..n {
        buf.push(hex_at(bytes, i + 3 * k)?);
    }
    let s = std::str::from_utf8(&buf).ok()?;
    let c = s.chars().next()?;
    is_iri_char(c, is_query).then_some(3 * n)
}

fn has_escaped_iri_char(s: &str, is_query: bool) -> bool {
    let bytes = s.as_bytes();
    (0..bytes.len()).any(|i| escaped_iri_char_at(bytes, i, is_query).is_some())
}

/// Escapes a path or query as `System.Uri` does: raw spaces, quotes,
/// ``<>^`{|}``, `\` (in the query), controls and non-ASCII as `%XX`; `%XX`
/// of unreserved characters decoded, others kept as written, except that in
/// IRI mode valid UTF-8 sequences and ``<>^`{|}`` come out in upper case; a
/// `%` that doesn't start an escape as `%25`.
fn escape_uri_part(s: &str, iri: bool, is_query: bool) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' {
            match hex_at(bytes, i) {
                Some(v) if v.is_ascii_alphanumeric() || b"-._~".contains(&v) => {
                    out.push(v as char);
                    i += 3;
                }
                Some(v) if iri && b" \"<>^`{|}".contains(&v) => {
                    out.push_str(&format!("%{v:02X}"));
                    i += 3;
                }
                Some(_) => {
                    if iri && let Some(len) = escaped_iri_char_at(bytes, i, is_query) {
                        out.push_str(&s[i..i + len].to_ascii_uppercase());
                        i += len;
                    } else {
                        out.push_str(&s[i..i + 3]);
                        i += 3;
                    }
                }
                None => {
                    out.push_str("%25");
                    i += 1;
                }
            }
            continue;
        }
        match b {
            b' ' | b'"' | b'<' | b'>' | b'^' | b'`' | b'{' | b'|' | b'}' | b'\\' | 0x7f | 0..=0x1f | 0x80..=0xff => {
                out.push_str(&format!("%{b:02X}"))
            }
            _ => out.push(b as char),
        }
        i += 1;
    }
    out
}

fn remove_dot_segments(path: &str) -> String {
    if !path.split('/').any(|s| s == "." || s == "..") {
        return path.to_owned();
    }
    let mut out: Vec<&str> = Vec::new();
    let segments: Vec<&str> = path.split('/').skip(1).collect();
    let last = segments.len().saturating_sub(1);
    for (i, seg) in segments.iter().enumerate() {
        match *seg {
            "." => {
                if i == last {
                    out.push("");
                }
            }
            ".." => {
                out.pop();
                if i == last {
                    out.push("");
                }
            }
            s => out.push(s),
        }
    }
    format!("/{}", out.join("/"))
}

// ---------------------------------------------------------------------------
// Requests and responses.

/// A request as `Uploader.CreateRequest` builds it.
#[derive(Clone)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    /// Request headers in order (`NameValueCollection`, values joined).
    pub headers: Vec<(String, String)>,
    pub content: Option<Content>,
}

/// `ResponseInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResponseInfo {
    pub status_code: u16,
    pub status_description: String,
    /// `RequestMessage.RequestUri.OriginalString`: the URL as given, or
    /// where redirects ended.
    pub response_url: Option<String>,
    /// `WebHeaderCollection`: response headers, then content headers, with
    /// repeated headers joined by `, `.
    pub headers: Vec<(String, String)>,
    pub response_text: Option<String>,
    pub response_data: Option<Vec<u8>>,
}

impl ResponseInfo {
    /// `IsSuccess` (`WebHelpers.IsSuccessStatusCode`: 2xx).
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    /// `Headers[name]`: case-insensitive.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// `WebHeaderCollection.ToString()`.
    pub fn headers_to_string(&self) -> String {
        let mut s = String::new();
        for (k, v) in &self.headers {
            s.push_str(&format!("{k}: {v}{CRLF}"));
        }
        s.push_str(CRLF);
        s
    }

    /// `ToReadableString(includeResponseText)`.
    pub fn to_readable_string(&self, include_response_text: bool) -> String {
        let nl = crate::NEW_LINE;
        let mut s = format!("Status code:{nl}({}) {}", self.status_code, self.status_description);
        if let Some(u) = self.response_url.as_deref().filter(|u| !u.is_empty()) {
            s.push_str(&format!("{nl}{nl}Response URL:{nl}{u}"));
        }
        if !self.headers.is_empty() {
            s.push_str(&format!("{nl}{nl}Headers:{nl}{}", self.headers_to_string().trim_end()));
        }
        if include_response_text && let Some(t) = self.response_text.as_deref().filter(|t| !t.is_empty()) {
            s.push_str(&format!("{nl}{nl}Response text:{nl}{t}"));
        }
        s
    }
}

/// Why a request didn't get a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// Stopped by its [`CancellationToken`].
    Cancelled,
    /// `TimeoutException`.
    Timeout(String),
    /// Anything else (`Exception.Message`).
    Failed(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::Cancelled => f.write_str("The operation was canceled."),
            HttpError::Timeout(m) | HttpError::Failed(m) => f.write_str(m),
        }
    }
}

/// Content headers in .NET (`HttpContentHeaders`): they come after the
/// request headers and in a response after the other headers.
pub fn is_content_header(name: &str) -> bool {
    const NAMES: [&str; 11] = [
        "Allow",
        "Content-Disposition",
        "Content-Encoding",
        "Content-Language",
        "Content-Length",
        "Content-Location",
        "Content-MD5",
        "Content-Range",
        "Content-Type",
        "Expires",
        "Last-Modified",
    ];
    NAMES.iter().any(|n| n.eq_ignore_ascii_case(name))
}

/// A header name as servers usually write it (ureq gives names in lower
/// case; .NET keeps the server's): `content-type` → `Content-Type`.
fn title_case(name: &str) -> String {
    name.split('-')
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Options for one request.
#[derive(Clone)]
pub struct RequestOptions {
    pub allow_auto_redirect: bool,
    /// `Uploader.RequestTimeout` (`None`: infinite).
    pub timeout: Option<Duration>,
    pub cancel: CancellationToken,
    pub progress: Option<ProgressCallback>,
    /// Read the response body (`readResponseText`).
    pub read_body: bool,
    /// Sent through `HttpClientFactory`'s shared client, whose default
    /// headers add `Cache-Control: no-cache`. `false` for code that makes a
    /// plain `new HttpClient()` (no default headers; ShareXrs still sends
    /// its user agent).
    pub factory_client: bool,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            allow_auto_redirect: true,
            timeout: None,
            cancel: CancellationToken::new(),
            progress: None,
            read_body: true,
            factory_client: true,
        }
    }
}

/// The headers a request goes out with, in .NET's order: the request's
/// headers, then `HttpClient`'s defaults it doesn't override (user agent,
/// `Cache-Control: no-cache`), then the content headers.
pub fn wire_headers(request: &HttpRequest) -> Vec<(String, String)> {
    wire_headers_for(request, true)
}

/// [`wire_headers`], for a request sent through `HttpClientFactory`'s
/// client (`factory_client`) or a plain `HttpClient` (no `Cache-Control`).
pub fn wire_headers_for(request: &HttpRequest, factory_client: bool) -> Vec<(String, String)> {
    let user_agent = SETTINGS.read().expect("http settings").user_agent.clone();
    let mut out: Vec<(String, String)> = request.headers.clone();
    if !out.iter().any(|(k, _)| k.eq_ignore_ascii_case("User-Agent")) {
        out.push(("User-Agent".into(), user_agent));
    }
    if factory_client && !out.iter().any(|(k, _)| k.eq_ignore_ascii_case("Cache-Control")) {
        out.push(("Cache-Control".into(), "no-cache".into()));
    }
    match &request.content {
        Some(content) => {
            if let Some(ct) = content.content_type() {
                out.push(("Content-Type".into(), ct));
            }
            out.extend(content.extra_headers().iter().cloned());
            out.push(("Content-Length".into(), content.length().to_string()));
        }
        None => {
            // SocketsHttpHandler sends an explicit empty body for these.
            if matches!(request.method, HttpMethod::POST | HttpMethod::PUT) {
                out.push(("Content-Length".into(), "0".into()));
            }
        }
    }
    out
}

/// Sends a request and reads its response (`HttpClient.SendAsync` with
/// `ResponseHeadersRead`, then `CreateResponseInfoAsync`).
pub fn send(request: HttpRequest, options: RequestOptions) -> Result<ResponseInfo, HttpError> {
    let cancel = options.cancel.clone();
    if cancel.is_cancelled() {
        return Err(HttpError::Cancelled);
    }
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .name("upload-request".into())
        .spawn(move || {
            let _ = tx.send(send_blocking(request, options));
        })
        .map_err(|e| HttpError::Failed(e.to_string()))?;
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(result) => {
                let _ = worker.join();
                return result;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if cancel.is_cancelled() {
                    // The worker finishes on its own; its result is dropped.
                    return Err(HttpError::Cancelled);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(HttpError::Failed("The request thread stopped.".into())),
        }
    }
}

fn send_blocking(request: HttpRequest, options: RequestOptions) -> Result<ResponseInfo, HttpError> {
    let (mut info, mut response) = start(&request, &options)?;
    if options.read_body {
        let mut data = Vec::new();
        let mut reader = response.body_mut().as_reader();
        let mut buf = [0u8; 16384];
        let started = Instant::now();
        loop {
            if options.cancel.is_cancelled() {
                return Err(HttpError::Cancelled);
            }
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => data.extend_from_slice(&buf[..n]),
                Err(e) => return Err(map_io_error(e, &options, started)),
            }
        }
        let text = decode_text(&data, info.header("Content-Type"));
        info.response_data = Some(data);
        info.response_text = Some(text);
    }
    Ok(info)
}

/// Sends a request on the calling thread and hands its response's status,
/// headers and body stream to `read` (`HttpClient.SendAsync` with
/// `ResponseHeadersRead`, then `ReadAsStreamAsync`), for downloads that
/// write the body as it arrives. The body stops with an error once
/// `options.cancel` is cancelled.
pub fn send_streaming<T>(
    request: HttpRequest,
    options: RequestOptions,
    read: impl FnOnce(&ResponseInfo, &mut dyn Read) -> Result<T, HttpError>,
) -> Result<T, HttpError> {
    if options.cancel.is_cancelled() {
        return Err(HttpError::Cancelled);
    }
    let (info, mut response) = start(&request, &options)?;
    let mut body = response.body_mut().as_reader();
    let mut reader = CancelReader { inner: &mut body, options: &options, started: Instant::now() };
    read(&info, &mut reader)
}

/// A response body that stops once its request is cancelled, with errors
/// worded as [`HttpError`]'s.
struct CancelReader<'a, R: Read> {
    inner: &'a mut R,
    options: &'a RequestOptions,
    started: Instant,
}

impl<R: Read> Read for CancelReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.options.cancel.is_cancelled() {
            return Err(io::Error::other(HttpError::Cancelled.to_string()));
        }
        self.inner.read(buf).map_err(|e| io::Error::other(map_io_error(e, self.options, self.started).to_string()))
    }
}

/// Sends `request` and reads the response's status and headers, leaving
/// its body unread.
fn start(
    request: &HttpRequest,
    options: &RequestOptions,
) -> Result<(ResponseInfo, ureq::http::Response<ureq::Body>), HttpError> {
    let absolute = dotnet_absolute_uri(&request.url).map_err(HttpError::Failed)?;
    let (rewriter, proxy) = {
        let settings = SETTINGS.read().expect("http settings");
        (settings.rewriter.clone(), settings.proxy.clone())
    };
    let target = match &rewriter {
        Some(r) => (r.forward)(&absolute),
        None => absolute.clone(),
    };
    let method = ureq::http::Method::from_bytes(request.method.name().as_bytes()).map_err(|e| HttpError::Failed(e.to_string()))?;
    let mut builder = ureq::http::Request::builder().method(method).uri(target.as_str());
    for (name, value) in wire_headers_for(request, options.factory_client) {
        let value = ureq::http::HeaderValue::from_maybe_shared(value.into_bytes())
            .map_err(|_| HttpError::Failed(format!("The format of value for header '{name}' is invalid.")))?;
        builder = builder.header(name.as_str(), value);
    }
    let agent = agent(options.allow_auto_redirect, proxy.route(&target));
    let started = Instant::now();
    let send_result = match &request.content {
        Some(content) => {
            let reader = BodyReader::new(content, options.cancel.clone(), options.progress.clone());
            let req = builder.body(ureq::SendBody::from_owned_reader(reader)).map_err(|e| HttpError::Failed(e.to_string()))?;
            let mut req = agent.configure_request(req);
            if let Some(t) = options.timeout {
                req = req.timeout_global(Some(t));
            }
            agent.run(req.build())
        }
        None => {
            let req = builder.body(()).map_err(|e| HttpError::Failed(e.to_string()))?;
            let mut req = agent.configure_request(req);
            if let Some(t) = options.timeout {
                req = req.timeout_global(Some(t));
            }
            agent.run(req.build())
        }
    };
    let response = send_result.map_err(|e| map_error(e, options, started))?;
    let status = response.status();
    let final_uri = {
        use ureq::ResponseExt;
        let redirected = response.get_redirect_history().is_some_and(|h| h.len() > 1);
        if redirected {
            let uri = response.get_uri().to_string();
            Some(match &rewriter {
                Some(r) => (r.back)(&uri),
                None => uri,
            })
        } else {
            None
        }
    };
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut content_headers: Vec<(String, String)> = Vec::new();
    for (name, value) in response.headers() {
        let name = title_case(name.as_str());
        let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
        let list = if is_content_header(&name) { &mut content_headers } else { &mut headers };
        match list.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(&name)) {
            Some((_, v)) => {
                v.push_str(", ");
                v.push_str(&value);
            }
            None => list.push((name, value)),
        }
    }
    headers.extend(content_headers);
    let info = ResponseInfo {
        status_code: status.as_u16(),
        status_description: status.canonical_reason().unwrap_or("").to_owned(),
        response_url: Some(final_uri.unwrap_or_else(|| request.url.clone())),
        headers,
        response_text: None,
        response_data: None,
    };
    Ok((info, response))
}

fn timeout_message(options: &RequestOptions) -> String {
    match options.timeout {
        None => "The HTTP request timed out.".to_owned(),
        Some(t) => format!("The request timed out after {}.", format_timespan(t)),
    }
}

/// `TimeSpan.ToString()` (`[d.]hh:mm:ss[.fffffff]`).
fn format_timespan(t: Duration) -> String {
    let total = t.as_secs();
    let (d, h, m, s) = (total / 86400, total / 3600 % 24, total / 60 % 60, total % 60);
    let ticks = t.subsec_nanos() / 100;
    let mut out = if d > 0 { format!("{d}.{h:02}:{m:02}:{s:02}") } else { format!("{h:02}:{m:02}:{s:02}") };
    if ticks > 0 {
        out.push_str(&format!(".{ticks:07}"));
    }
    out
}

fn map_error(e: ureq::Error, options: &RequestOptions, _started: Instant) -> HttpError {
    if options.cancel.is_cancelled() {
        return HttpError::Cancelled;
    }
    match e {
        ureq::Error::Timeout(_) => HttpError::Timeout(timeout_message(options)),
        ureq::Error::Io(io) => map_io_error(io, options, _started),
        other => HttpError::Failed(other.to_string()),
    }
}

fn map_io_error(e: io::Error, options: &RequestOptions, _started: Instant) -> HttpError {
    if options.cancel.is_cancelled() {
        return HttpError::Cancelled;
    }
    match e.kind() {
        io::ErrorKind::TimedOut => HttpError::Timeout(timeout_message(options)),
        _ => HttpError::Failed(e.to_string()),
    }
}

/// `HttpContent.ReadAsStringAsync`: the `charset` of the content type,
/// else a byte order mark, else UTF-8.
pub fn decode_text(data: &[u8], content_type: Option<&str>) -> String {
    let charset = content_type.and_then(|ct| {
        ct.split(';').skip(1).find_map(|p| {
            let (k, v) = p.split_once('=')?;
            k.trim().eq_ignore_ascii_case("charset").then(|| v.trim().trim_matches('"').to_owned())
        })
    });
    if let Some(enc) = charset.as_deref().and_then(|c| encoding_rs::Encoding::for_label(c.as_bytes())) {
        let (text, _) = enc.decode_with_bom_removal(data);
        return text.into_owned();
    }
    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(data) {
        let (text, _) = enc.decode_without_bom_handling(&data[bom_len..]);
        return text.into_owned();
    }
    String::from_utf8_lossy(data).into_owned()
}

/// `HttpStatusCode.ToString()`: the enum member's name, or the number for
/// a code .NET doesn't name. Where two names share a value, .NET's
/// `Enum.ToString` gives the one listed here.
pub fn status_code_name(code: u16) -> String {
    let name = match code {
        100 => "Continue",
        101 => "SwitchingProtocols",
        102 => "Processing",
        103 => "EarlyHints",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "NonAuthoritativeInformation",
        204 => "NoContent",
        205 => "ResetContent",
        206 => "PartialContent",
        207 => "MultiStatus",
        208 => "AlreadyReported",
        226 => "IMUsed",
        300 => "Ambiguous",
        301 => "Moved",
        302 => "Redirect",
        303 => "RedirectMethod",
        304 => "NotModified",
        305 => "UseProxy",
        306 => "Unused",
        307 => "TemporaryRedirect",
        308 => "PermanentRedirect",
        400 => "BadRequest",
        401 => "Unauthorized",
        402 => "PaymentRequired",
        403 => "Forbidden",
        404 => "NotFound",
        405 => "MethodNotAllowed",
        406 => "NotAcceptable",
        407 => "ProxyAuthenticationRequired",
        408 => "RequestTimeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "LengthRequired",
        412 => "PreconditionFailed",
        413 => "RequestEntityTooLarge",
        414 => "RequestUriTooLong",
        415 => "UnsupportedMediaType",
        416 => "RequestedRangeNotSatisfiable",
        417 => "ExpectationFailed",
        421 => "MisdirectedRequest",
        422 => "UnprocessableContent",
        423 => "Locked",
        424 => "FailedDependency",
        426 => "UpgradeRequired",
        428 => "PreconditionRequired",
        429 => "TooManyRequests",
        431 => "RequestHeaderFieldsTooLarge",
        451 => "UnavailableForLegalReasons",
        500 => "InternalServerError",
        501 => "NotImplemented",
        502 => "BadGateway",
        503 => "ServiceUnavailable",
        504 => "GatewayTimeout",
        505 => "HttpVersionNotSupported",
        506 => "VariantAlsoNegotiates",
        507 => "InsufficientStorage",
        508 => "LoopDetected",
        510 => "NotExtended",
        511 => "NetworkAuthenticationRequired",
        _ => return code.to_string(),
    };
    name.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_are_escaped_as_dotnet() {
        assert_eq!(dotnet_absolute_uri("HTTPS://Example.COM:443/a b/ä?q=1 2&x={y}#frag").unwrap(), "https://example.com/a%20b/%C3%A4?q=1%202&x=%7By%7D");
        assert_eq!(dotnet_absolute_uri("http://h:8080/a/./b/../c%2f%zz").unwrap(), "http://h:8080/a/c%2f%25zz");
        assert_eq!(dotnet_absolute_uri("http://h").unwrap(), "http://h/");
        assert_eq!(dotnet_absolute_uri("http://h\\a\\b").unwrap(), "http://h/a/b");
        // Probed from System.Uri (see tools/uploaders-harness/README.md).
        let t = |u: &str| dotnet_absolute_uri(u).unwrap().trim_start_matches("http://h").to_owned();
        assert_eq!(t("http://h/a/%2e%2e/b"), "/b");
        assert_eq!(t("http://h/%7e~"), "/~~");
        assert_eq!(t("http://h/a?q=%3c%0a%c3"), "/a?q=%3c%0a%c3");
        assert_eq!(t("http://h/a?q=%c3%a9%3c%0a&r=%ff"), "/a?q=%C3%A9%3C%0a&r=%ff");
        assert_eq!(t("http://h/a%c3%a9?q=%7b"), "/a%C3%A9?q=%7B");
        assert_eq!(t("http://h/a?x=%ef%bf%bf"), "/a?x=%ef%bf%bf");
        assert_eq!(t("http://h/a?q=a\\b"), "/a?q=a%5Cb");
        assert_eq!(t("http://h/a?q=%"), "/a?q=%25");
        assert!(dotnet_absolute_uri("ftp://h/").is_err());
    }

    #[test]
    fn multipart_length_matches_body() {
        let content = Content::Multipart {
            content_type: CONTENT_TYPE_MULTIPART_FORM_DATA.into(),
            boundary: "b".into(),
            parts: vec![
                MultipartPart { headers: vec![("Content-Disposition".into(), "form-data; name=\"a\"".into())], body: PartBody::Bytes(b"1".to_vec()) },
                MultipartPart {
                    headers: vec![("Content-Disposition".into(), "form-data; name=\"f\"; filename=\"x\"".into())],
                    body: PartBody::Stream(StreamPart {
                        source: shared_stream(io::Cursor::new(b"hello".to_vec())),
                        position: 0,
                        length: 5,
                        total_length: 5,
                        buffer_size: 2,
                        on_read: None,
                    }),
                },
            ],
            headers: Vec::new(),
        };
        let mut body = Vec::new();
        BodyReader::new(&content, CancellationToken::new(), None).read_to_end(&mut body).unwrap();
        assert_eq!(body.len() as u64, content.length());
        assert_eq!(
            String::from_utf8(body).unwrap(),
            "--b\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--b\r\nContent-Disposition: form-data; name=\"f\"; filename=\"x\"\r\n\r\nhello\r\n--b--\r\n"
        );
    }

    #[test]
    fn boundaries_look_like_dotnets() {
        let b = create_boundary();
        assert_eq!(b.len(), 52);
        assert!(b.starts_with("--------------------"));
    }

    #[test]
    fn text_is_decoded_by_charset_then_bom() {
        assert_eq!(decode_text(b"\xef\xbb\xbfhi", None), "hi");
        assert_eq!(decode_text(b"caf\xe9", Some("text/plain; charset=iso-8859-1")), "café");
        assert_eq!(decode_text("é".as_bytes(), Some("application/json")), "é");
    }
}

