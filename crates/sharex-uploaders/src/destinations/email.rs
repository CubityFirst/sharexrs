//! `FileUploaders/Email.cs` and `SharingServices/EmailSharingService.cs`:
//! the file sent as an attachment (or the URL as the body) through SMTP
//! with STARTTLS, as .NET's `SmtpClient` and `MailMessage` do it for
//! ShareX (checked against ShareX's own code on a mock server):
//!
//! - `EHLO`, `STARTTLS`, `EHLO`, `AUTH login <user>` + password, `MAIL FROM`,
//!   `RCPT TO` (each address), `DATA`, `QUIT` (not after a failure);
//! - the message: `MIME-Version`, `From`, `To`, `Date`, `Subject`
//!   (`=?utf-8?B?…?=` if not ASCII), then a `text/plain` body (us-ascii
//!   quoted-printable, or utf-8 base64) or `multipart/mixed` with
//!   `--boundary_<n>_<guid>` and the attachment as
//!   `application/octet-stream; name=…`, base64 in 68-column lines;
//!   headers folded at 76 columns.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;

use native_tls::{TlsConnector, TlsStream};

use super::cloud_storage::mime_parameter_value;
use super::uploader_base;
use crate::http::SharedStream;
use crate::result::UploadResult;
use crate::uploader::{CoreResult, UResult, UploadError, Uploader, WritesBackConfig};

static ACCEPT_INVALID_CERTS: AtomicBool = AtomicBool::new(false);
static BOUNDARY_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Accepts any server certificate. Only for tests against the mock server
/// (the harness does the same to .NET's `SmtpClient`); ShareX checks it.
pub fn set_accept_invalid_certificates_for_tests(accept: bool) {
    ACCEPT_INVALID_CERTS.store(accept, Ordering::SeqCst);
}

/// What `EmailWindow` returns: to, subject, body.
pub type EmailFields = (String, String, String);

/// Shows `EmailWindow` (to, subject, body to edit): `None` cancels.
pub type EmailUi = Arc<dyn Fn(&str, &str, &str) -> Option<EmailFields> + Send + Sync>;

static EMAIL_UI: LazyLock<RwLock<Option<EmailUi>>> = LazyLock::new(|| RwLock::new(None));

/// Sets the window email uploads and shares open when automatic sending
/// is off. Without one they're cancelled.
pub fn set_email_ui(ui: Option<EmailUi>) {
    *EMAIL_UI.write().expect("email ui") = ui;
}

/// `EmailWindowIntegration.Show(to, subject, body)`.
pub fn show_email_window(to: &str, subject: &str, body: &str) -> Option<EmailFields> {
    let ui = EMAIL_UI.read().expect("email ui").clone()?;
    ui(to, subject, body)
}

/// An SMTP failure (`SmtpException`) or an invalid address (`FormatException`).
fn smtp_error(message: impl Into<String>) -> UploadError {
    UploadError::Exception(message.into())
}

const INVALID_ADDRESS: &str = "The specified string is not in the form required for an e-mail address.";

/// `new MailAddress(text)` for plain addresses (`local@domain`); a list
/// for `MailAddressCollection.Add` (comma-separated).
fn parse_addresses(text: &str) -> UResult<Vec<String>> {
    let mut out = Vec::new();
    for part in text.split(',') {
        let a = part.trim();
        let valid = match a.rsplit_once('@') {
            Some((local, domain)) => {
                !local.is_empty()
                    && !domain.is_empty()
                    && !a.chars().any(|c| c.is_whitespace() || "<>()[]\\,;:\"".contains(c))
                    && !domain.starts_with('.')
                    && !domain.ends_with('.')
            }
            None => false,
        };
        if !valid {
            return Err(smtp_error(INVALID_ADDRESS));
        }
        out.push(a.to_owned());
    }
    if out.is_empty() {
        return Err(smtp_error(INVALID_ADDRESS));
    }
    Ok(out)
}

fn b64(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Base64 in lines of 68 characters, CRLF between.
fn b64_lines(data: &[u8]) -> String {
    let encoded = b64(data);
    encoded.as_bytes().chunks(68).map(|c| std::str::from_utf8(c).expect("ASCII")).collect::<Vec<_>>().join("\r\n")
}

/// .NET's quoted-printable body: `=` and anything outside printable ASCII
/// as `=XX`, CRLF kept, soft breaks before 76 columns.
fn quoted_printable(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut line = 0;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            out.push_str("\r\n");
            line = 0;
            i += 2;
            continue;
        }
        let piece = if (b == b' ' || b == b'\t' || (33..=126).contains(&b)) && b != b'=' { (b as char).to_string() } else { format!("={b:02X}") };
        if line + piece.len() > 75 {
            out.push_str("=\r\n");
            line = 0;
        }
        line += piece.len();
        out.push_str(&piece);
        i += 1;
    }
    out
}

/// A header value that may need MIME encoding (`Subject`).
fn encode_header_text(text: &str) -> String {
    if text.is_ascii() && !text.contains(['\r', '\n']) { text.to_owned() } else { format!("=?utf-8?B?{}?=", b64(text.as_bytes())) }
}

/// `Name: value; p=v; …`, a parameter moved to its own line (`;\r\n `)
/// when the line would pass 76 columns.
fn header_with_params(name: &str, value: &str, params: &[(&str, String)]) -> String {
    let mut out = format!("{name}: {value}");
    let mut line_len = out.len();
    for (k, v) in params {
        let p = format!("{k}={v}");
        if line_len + 2 + p.len() > 76 {
            out.push_str(";\r\n ");
            line_len = 1;
        } else {
            out.push_str("; ");
            line_len += 2;
        }
        out.push_str(&p);
        line_len += p.len();
    }
    out
}

/// `MailMessage.Date`: `d MMM yyyy HH:mm:ss +hhmm`, local time.
fn mail_date() -> String {
    chrono::Local::now().format("%-d %b %Y %H:%M:%S %z").to_string()
}

fn new_boundary() -> String {
    let n = BOUNDARY_COUNT.fetch_add(1, Ordering::SeqCst);
    let mut g = [0u8; 16];
    getrandom::fill(&mut g).expect("OS random numbers");
    g[6] = (g[6] & 0x0f) | 0x40;
    g[8] = (g[8] & 0x3f) | 0x80;
    let h = hex::encode(g);
    format!("--boundary_{n}_{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

/// The text part's headers and body: us-ascii quoted-printable, or utf-8
/// base64.
fn text_part(body: &str) -> (String, String) {
    if body.is_ascii() {
        ("Content-Type: text/plain; charset=us-ascii\r\nContent-Transfer-Encoding: quoted-printable".into(), quoted_printable(body))
    } else {
        ("Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64".into(), b64_lines(body.as_bytes()))
    }
}

/// The message as `MailMessage` writes it for `SmtpClient.Send`.
pub fn build_message(from: &str, to: &[String], subject: &str, body: &str, attachment: Option<(&str, &[u8])>) -> String {
    let mut m = format!("MIME-Version: 1.0\r\nFrom: {from}\r\nTo: {}\r\nDate: {}\r\n", to.join(", "), mail_date());
    m.push_str(&format!("Subject: {}\r\n", encode_header_text(subject)));
    let (text_headers, text_body) = text_part(body);
    match attachment {
        None => {
            m.push_str(&format!("{text_headers}\r\n\r\n{text_body}\r\n"));
        }
        Some((name, data)) => {
            let boundary = new_boundary();
            m.push_str(&header_with_params("Content-Type", "multipart/mixed", &[("boundary", boundary.clone())]));
            m.push_str("\r\n\r\n\r\n");
            m.push_str(&format!("--{boundary}\r\n{text_headers}\r\n\r\n{text_body}\r\n"));
            m.push_str(&format!("--{boundary}\r\n"));
            m.push_str(&header_with_params("Content-Type", "application/octet-stream", &[("name", mime_parameter_value(name))]));
            m.push_str("\r\nContent-Transfer-Encoding: base64\r\nContent-Disposition: attachment\r\n\r\n");
            m.push_str(&b64_lines(data));
            m.push_str(&format!("\r\n--{boundary}--\r\n\r\n"));
        }
    }
    m
}

enum Conn {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.read(buf),
            Conn::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.write(buf),
            Conn::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Conn::Plain(s) => s.flush(),
            Conn::Tls(s) => s.flush(),
        }
    }
}

struct Smtp {
    reader: BufReader<Conn>,
}

/// A reply: code, the last line's text, every line's text.
struct Reply {
    code: u16,
    text: String,
    lines: Vec<String>,
}

impl Smtp {
    fn io(e: std::io::Error) -> UploadError {
        smtp_error(format!("Failure sending mail. {e}"))
    }

    fn reply(&mut self) -> UResult<Reply> {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line).map_err(Self::io)? == 0 {
                return Err(smtp_error("Failure sending mail. Unable to read data from the transport connection: net_io_connectionclosed."));
            }
            let line = line.trim_end_matches(['\r', '\n']).to_owned();
            let code = line.get(..3).and_then(|c| c.parse().ok()).unwrap_or(0);
            let last = line.as_bytes().get(3) != Some(&b'-');
            lines.push(line.get(4..).unwrap_or("").to_owned());
            if last {
                return Ok(Reply { code, text: lines.last().cloned().unwrap_or_default(), lines });
            }
        }
    }

    fn send(&mut self, line: &str) -> UResult<Reply> {
        let w = self.reader.get_mut();
        w.write_all(format!("{line}\r\n").as_bytes()).map_err(Self::io)?;
        w.flush().map_err(Self::io)?;
        self.reply()
    }
}

/// SMTP settings of a send.
pub struct EmailSettings {
    pub smtp_server: String,
    pub smtp_port: i32,
    pub from_email: String,
    pub password: String,
}

/// `SmtpClient.Send(MailMessage)` with ShareX's settings (STARTTLS,
/// the from address as the login).
pub fn send_mail(settings: &EmailSettings, to: &str, subject: &str, body: &str, attachment: Option<(&str, &[u8])>) -> UResult<()> {
    let from = parse_addresses(&settings.from_email)?.remove(0);
    let to = parse_addresses(to)?;
    let message = build_message(&from, &to, subject, body, attachment);
    let port = u16::try_from(settings.smtp_port).map_err(|_| smtp_error("Specified argument was out of the range of valid values. (Parameter 'value')"))?;
    let addrs: Vec<_> = (settings.smtp_server.as_str(), port).to_socket_addrs().map_err(|e| smtp_error(format!("Failure sending mail. {e}")))?.collect();
    let stream = addrs
        .iter()
        .find_map(|a| TcpStream::connect_timeout(a, Duration::from_secs(100)).ok())
        .ok_or_else(|| smtp_error("Failure sending mail. No connection could be made because the target machine actively refused it."))?;
    stream.set_read_timeout(Some(Duration::from_secs(100))).map_err(Smtp::io)?;
    let mut s = Smtp { reader: BufReader::new(Conn::Plain(stream.try_clone().map_err(Smtp::io)?)) };
    let greeting = s.reply()?;
    if greeting.code != 220 {
        return Err(smtp_error(format!("Failure sending mail. The server response was: {}", greeting.text)));
    }
    let client_domain = local_host_name();
    let mut ehlo = s.send(&format!("EHLO {client_domain}"))?;
    if ehlo.code != 250 {
        ehlo = s.send(&format!("HELO {client_domain}"))?;
    }
    if !ehlo.lines.iter().any(|l| l.eq_ignore_ascii_case("STARTTLS")) {
        return Err(smtp_error("Server does not support secure connections."));
    }
    let r = s.send("STARTTLS")?;
    if r.code != 220 {
        return Err(smtp_error(format!("Server does not support secure connections. The server response was: {}", r.text)));
    }
    let connector = TlsConnector::builder()
        .danger_accept_invalid_certs(ACCEPT_INVALID_CERTS.load(Ordering::SeqCst))
        .danger_accept_invalid_hostnames(ACCEPT_INVALID_CERTS.load(Ordering::SeqCst))
        .build()
        .map_err(|e| smtp_error(e.to_string()))?;
    let tls = connector
        .connect(&settings.smtp_server, stream)
        .map_err(|_| smtp_error("Failure sending mail. The remote certificate is invalid according to the validation procedure."))?;
    s = Smtp { reader: BufReader::new(Conn::Tls(Box::new(tls))) };
    let ehlo = s.send(&format!("EHLO {client_domain}"))?;
    let auth_login = ehlo.lines.iter().any(|l| {
        let u = l.to_ascii_uppercase();
        u.starts_with("AUTH") && u[4..].split([' ', '=']).any(|m| m == "LOGIN")
    });
    if auth_login {
        // .NET goes on to MAIL FROM whatever the server says to the login.
        let r = s.send(&format!("AUTH login {}", b64(settings.from_email.as_bytes())))?;
        if r.code == 334 {
            s.send(&b64(settings.password.as_bytes()))?;
        }
    }
    let r = s.send(&format!("MAIL FROM:<{from}>"))?;
    if r.code != 250 {
        return Err(smtp_error(match r.code {
            530 => format!("The SMTP server requires a secure connection or the client was not authenticated. The server response was: {}", r.text),
            _ => format!("Mailbox unavailable. The server response was: {}", r.text),
        }));
    }
    for address in &to {
        let r = s.send(&format!("RCPT TO:<{address}>"))?;
        if r.code != 250 && r.code != 251 {
            return Err(smtp_error(format!("Mailbox unavailable. The server response was: {}", r.text)));
        }
    }
    let r = s.send("DATA")?;
    if r.code != 354 {
        return Err(smtp_error(format!("Failure sending mail. The server response was: {}", r.text)));
    }
    let mut data = String::new();
    for line in message.split("\r\n") {
        if line.starts_with('.') {
            data.push('.');
        }
        data.push_str(line);
        data.push_str("\r\n");
    }
    // The split above gave the message's last CRLF an empty line: drop it.
    data.truncate(data.len() - 2);
    let w = s.reader.get_mut();
    w.write_all(data.as_bytes()).map_err(Smtp::io)?;
    w.write_all(b"\r\n.\r\n").map_err(Smtp::io)?;
    w.flush().map_err(Smtp::io)?;
    let r = s.reply()?;
    if r.code != 250 {
        return Err(smtp_error(format!("Failure sending mail. The server response was: {}", r.text)));
    }
    let _ = s.send("QUIT");
    Ok(())
}

/// `IPGlobalProperties.HostName`: the machine's name.
fn local_host_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|n| !n.is_empty())
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_owned()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "localhost".into())
}

/// `Email`: the file as an attachment.
pub struct Email {
    pub base: Uploader,
    pub settings: EmailSettings,
    pub to_email: String,
    pub subject: String,
    pub body: String,
}

uploader_base!(Email, FileUploader);
impl WritesBackConfig for Email {}

impl Email {
    pub fn new(settings: EmailSettings, to_email: &str, subject: &str, body: &str) -> Self {
        Self { base: Uploader::new(), settings, to_email: to_email.to_owned(), subject: subject.to_owned(), body: body.to_owned() }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if self.base.stop_requested() {
            return Err(UploadError::Cancelled);
        }
        let mut bytes = Vec::new();
        {
            use std::io::{Seek, SeekFrom};
            let mut s = data.lock().expect("upload stream");
            s.seek(SeekFrom::Start(0)).map_err(|e| UploadError::Exception(e.to_string()))?;
            s.read_to_end(&mut bytes).map_err(|e| UploadError::Exception(e.to_string()))?;
        }
        send_mail(&self.settings, &self.to_email, &self.subject, &self.body, Some((file_name, &bytes)))?;
        Ok(Some(UploadResult { is_url_expected: false, ..Default::default() }))
    }
}

/// `EmailSharer`: the URL as the body; without automatic sending, the
/// email window first.
pub struct EmailSharer {
    pub base: Uploader,
    pub settings: EmailSettings,
    /// `EmailAutomaticSendTo` when sending automatically.
    pub automatic_to: Option<String>,
    pub subject: String,
    /// The window's first address (`EmailLastTo` when remembered).
    pub last_to: String,
    pub remember_last_to: bool,
    /// The address the window gave, written back as `EmailLastTo`.
    pub sent_to: Option<String>,
}

uploader_base!(EmailSharer, UrlSharer);

impl WritesBackConfig for EmailSharer {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        if self.remember_last_to
            && let Some(to) = &self.sent_to
        {
            config.EmailLastTo = Some(to.clone());
        }
    }
}

impl EmailSharer {
    pub fn new(settings: EmailSettings, subject: &str) -> Self {
        Self {
            base: Uploader::new(),
            settings,
            automatic_to: None,
            subject: subject.to_owned(),
            last_to: String::new(),
            remember_last_to: true,
            sent_to: None,
        }
    }

    fn share(&mut self, url: &str) -> CoreResult {
        if self.base.stop_requested() {
            return Err(UploadError::Cancelled);
        }
        let result = UploadResult { url: Some(url.to_owned()), is_url_expected: false, ..Default::default() };
        match self.automatic_to.clone() {
            Some(to) => send_mail(&self.settings, &to, &self.subject, url, None)?,
            None => {
                let first = if self.remember_last_to { self.last_to.clone() } else { String::new() };
                if let Some((to, subject, body)) = show_email_window(&first, &self.subject, url) {
                    self.sent_to = Some(to.clone());
                    send_mail(&self.settings, &to, &subject, &body, None)?;
                }
            }
        }
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_as_mail_message_writes_them() {
        assert_eq!(quoted_printable("a=1&b=two words"), "a=3D1&b=3Dtwo words");
        assert_eq!(encode_header_text("Écran"), "=?utf-8?B?w4ljcmFu?=");
        let h = header_with_params("Content-Type", "multipart/mixed", &[("boundary", "--boundary_0_3dcb2e09-a8db-4da5-85e8-38b47ed1ee63".into())]);
        assert_eq!(h, "Content-Type: multipart/mixed;\r\n boundary=--boundary_0_3dcb2e09-a8db-4da5-85e8-38b47ed1ee63");
        assert!(parse_addresses("user").is_err());
        assert_eq!(parse_addresses("a@b.c, d@e.f").unwrap(), vec!["a@b.c", "d@e.f"]);
        let m = build_message("a@b.c", &["d@e.f".into()], "S", "https://x/?a=1", None);
        assert!(m.ends_with("Content-Transfer-Encoding: quoted-printable\r\n\r\nhttps://x/?a=3D1\r\n"), "{m}");
    }
}
