//! Where requests go: `ProxyInfo` (HelpersLib, `ApplicationConfig.ProxySettings`)
//! as `HttpClientFactory.Create` hands it to its `SocketsHttpHandler`, and
//! .NET's system proxy (`HttpClient.DefaultProxy`) behind it.
//!
//! Since ShareX #9319 (7d17d32), only `ProxyMethod.None` connects directly.
//! A valid manual proxy (`IsValidProxy`: Manual, a host and a port) is used
//! as given; anything else (Automatic, or Manual without a host or port, the
//! default) leaves `Proxy` null with `UseProxy` on, which is the system
//! proxy, with the user name and password as `DefaultProxyCredentials` when
//! Automatic has a user name. Nothing resolved is written back into the
//! settings.
//!
//! The system proxy is .NET's `SystemProxyInfo.ConstructSystemProxy`: the
//! proxy environment variables when any is set (`HttpEnvironmentProxy`, on
//! every platform), else on Windows the user's Internet settings
//! (`HttpWindowsProxy`: auto-detect (WPAD) and PAC scripts through WinHTTP's
//! `WinHttpGetProxyForUrl`, falling back to the manual proxy and its bypass
//! list) and on macOS the system's (`MacProxy`: `CFNetworkCopyProxiesForURL`).
//! On Linux .NET has only the environment variables, and so has ShareXrs.
//!
//! Differences, from ureq: one proxy per request (.NET's `MultiProxy` fails
//! over to the next proxy of a list; ShareXrs takes the first), proxy
//! credentials go as Basic (preemptively; .NET answers a 407's
//! Basic/Digest/NTLM/Negotiate challenge), and macOS PAC entries
//! (`kCFProxyTypeAutoConfigurationURL`) are skipped.

use sharex_settings::ProxyMethod;

/// `ShareX.HelpersLib.ProxyInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyInfo {
    pub proxy_method: ProxyMethod,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub password: String,
}

impl Default for ProxyInfo {
    /// `new ProxyInfo()`: Manual, with nothing filled in.
    fn default() -> Self {
        Self { proxy_method: ProxyMethod::Manual, host: String::new(), port: 0, username: String::new(), password: String::new() }
    }
}

impl ProxyInfo {
    /// Reads `ApplicationConfig.ProxySettings` (missing or null fields as
    /// .NET's defaults).
    pub fn from_json(value: &serde_json::Value) -> Self {
        let text = |key: &str| value.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_owned();
        Self {
            proxy_method: value
                .get("ProxyMethod")
                .and_then(|v| serde_json::from_value::<ProxyMethod>(v.clone()).ok())
                .unwrap_or(ProxyMethod::Manual),
            host: text("Host"),
            port: value.get("Port").and_then(|v| v.as_i64()).unwrap_or(0),
            username: text("Username"),
            password: text("Password"),
        }
    }

    /// `IsValidProxy`: only a manual proxy with a host and a port.
    pub fn is_valid_proxy(&self) -> bool {
        self.proxy_method == ProxyMethod::Manual && !self.host.is_empty() && self.port > 0
    }

    /// What `HttpClientFactory.Create` gives its handler.
    pub fn plan(&self) -> ProxyPlan {
        if self.proxy_method == ProxyMethod::None {
            return ProxyPlan::Direct;
        }
        if self.is_valid_proxy() {
            // `GetWebProxy`: `new WebProxy($"{Host}:{Port}", BypassOnLocal: true, null,
            // new NetworkCredential(Username, Password))`; `WebProxy` reads an
            // address without a scheme as http.
            let address = format!("{}:{}", self.host, self.port);
            let uri = if address.contains("://") { address } else { format!("http://{address}") };
            return ProxyPlan::Manual { uri, credentials: self.credentials() };
        }
        let credentials = if self.proxy_method == ProxyMethod::Automatic { self.credentials() } else { None };
        ProxyPlan::System { credentials }
    }

    fn credentials(&self) -> Option<Credentials> {
        (!self.username.is_empty()).then(|| Credentials { username: self.username.clone(), password: self.password.clone() })
    }
}

/// A user name and password for the proxy (`NetworkCredential`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// How requests reach the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyPlan {
    /// `UseProxy = false` (`ProxyMethod.None`).
    Direct,
    /// A manual proxy (`WebProxy` with `BypassOnLocal`).
    Manual { uri: String, credentials: Option<Credentials> },
    /// The system proxy, with `DefaultProxyCredentials` if any.
    System { credentials: Option<Credentials> },
}

impl ProxyPlan {
    /// The proxy a request to `url` goes through (a URI ureq takes, with
    /// any credentials in it), or `None` to connect directly.
    pub fn route(&self, url: &str) -> Option<String> {
        let target = Target::parse(url)?;
        match self {
            ProxyPlan::Direct => None,
            ProxyPlan::Manual { uri, credentials } => {
                // `WebProxy.IsBypassed` with `BypassOnLocal`: `IsLocal`.
                (!is_local(&target.host)).then(|| with_credentials(uri, credentials.as_ref()))
            }
            ProxyPlan::System { credentials } => {
                system_proxy(&target, url).map(|p| with_credentials(&p, credentials.as_ref()))
            }
        }
    }
}

/// The parts of a request URI the proxy choice looks at (`Uri.Scheme`,
/// `Uri.Host`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub scheme: String,
    pub host: String,
}

impl Target {
    pub(crate) fn parse(url: &str) -> Option<Self> {
        let (scheme, rest) = url.split_once("://")?;
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let host = if host_port.starts_with('[') {
            host_port.split_inclusive(']').next().unwrap_or(host_port)
        } else {
            host_port.split(':').next().unwrap_or_default()
        };
        Some(Self { scheme: scheme.to_ascii_lowercase(), host: host.to_ascii_lowercase() })
    }

    /// `HttpUtilities.IsSupportedSecureScheme`: https and wss.
    pub(crate) fn is_secure(&self) -> bool {
        self.scheme == "https" || self.scheme == "wss"
    }

    /// The host without an IPv6 address's brackets.
    #[cfg_attr(not(windows), allow(dead_code))] // only `HttpWindowsProxy` (and the tests) use it
    fn bare_host(&self) -> &str {
        self.host.trim_start_matches('[').trim_end_matches(']')
    }

    /// `Uri.IsLoopback`: localhost or a loopback address.
    #[cfg_attr(not(windows), allow(dead_code))] // only `HttpWindowsProxy` (and the tests) use it
    fn is_loopback(&self) -> bool {
        is_loopback_host(self.bare_host())
    }
}

fn is_loopback_host(host: &str) -> bool {
    host == "localhost" || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// `WebProxy.IsLocal`: a loopback address or a name without a dot. (.NET
/// also counts this machine's own addresses and domain; ShareXrs doesn't
/// look them up.)
fn is_local(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if is_loopback_host(host) {
        return true;
    }
    host.parse::<std::net::IpAddr>().is_err() && !host.contains('.')
}

/// Puts `credentials` into a proxy URI that has none.
fn with_credentials(uri: &str, credentials: Option<&Credentials>) -> String {
    let Some(c) = credentials else { return uri.to_owned() };
    let Some((scheme, rest)) = uri.split_once("://") else { return uri.to_owned() };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return uri.to_owned();
    }
    format!("{scheme}://{}:{}@{rest}", escape_userinfo(&c.username), escape_userinfo(&c.password))
}

/// `Uri.EscapeDataString`: everything but the unreserved characters.
fn escape_userinfo(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn unescape(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// The system proxy (`SystemProxyInfo.ConstructSystemProxy`).

fn system_proxy(target: &Target, url: &str) -> Option<String> {
    if let Some(env) = EnvProxy::from_env(&|name| std::env::var(name).ok(), cfg!(windows)) {
        return env.route(target);
    }
    #[cfg(windows)]
    {
        windows::system_proxy(target, url)
    }
    #[cfg(target_os = "macos")]
    {
        macos::system_proxy(url)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = url;
        None
    }
}

/// `HttpEnvironmentProxy`: `http_proxy`, `https_proxy`, `all_proxy` and
/// `no_proxy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnvProxy {
    http: Option<String>,
    https: Option<String>,
    bypass: Vec<String>,
}

impl EnvProxy {
    /// `HttpEnvironmentProxy.TryCreate`. Windows reads the upper-case names
    /// (its variables ignore case); elsewhere the lower-case name wins. The
    /// http proxy is ignored under CGI (`GATEWAY_INTERFACE` set).
    pub(crate) fn from_env(get: &dyn Fn(&str) -> Option<String>, windows: bool) -> Option<Self> {
        let var = |lc: &str, uc: &str| -> Option<String> {
            if windows {
                env_uri(get(uc).as_deref())
            } else {
                env_uri(get(lc).as_deref()).or_else(|| env_uri(get(uc).as_deref()))
            }
        };
        let mut http = if get("GATEWAY_INTERFACE").is_none() { var("http_proxy", "HTTP_PROXY") } else { None };
        let mut https = var("https_proxy", "HTTPS_PROXY");
        if http.is_none() || https.is_none() {
            let all = var("all_proxy", "ALL_PROXY");
            http = http.or_else(|| all.clone());
            https = https.or(all);
        }
        if http.is_none() && https.is_none() {
            return None;
        }
        let no_proxy = if windows { get("NO_PROXY") } else { get("no_proxy").or_else(|| get("NO_PROXY")) };
        let bypass = no_proxy
            .map(|list| list.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect())
            .unwrap_or_default();
        Some(Self { http, https, bypass })
    }

    /// `GetProxy`, `IsBypassed`.
    pub(crate) fn route(&self, target: &Target) -> Option<String> {
        let proxy = if target.is_secure() { self.https.as_ref() } else { self.http.as_ref() }?;
        (!self.is_match_in_bypass_list(&target.host)).then(|| proxy.clone())
    }

    /// `IsMatchInBypassList`: `.foo.com` is foo.com and its subdomains,
    /// anything else a whole host name, ignoring case.
    fn is_match_in_bypass_list(&self, host: &str) -> bool {
        self.bypass.iter().any(|s| {
            let s = s.to_ascii_lowercase();
            match s.strip_prefix('.') {
                Some(domain) => host == domain || host.ends_with(&s),
                None => s == host,
            }
        })
    }
}

/// `HttpEnvironmentProxy.GetUriFromString`: `[scheme://][user[:password]@]host[:port]`,
/// http, https, socks4, socks4a or socks5 (http and port 80 by default).
fn env_uri(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    let mut protocol = "http";
    let mut port: u16 = 80;
    let mut rest = value;
    for (prefix, p) in [("http://", "http"), ("https://", "https"), ("socks4://", "socks4"), ("socks5://", "socks5"), ("socks4a://", "socks4a")] {
        if lower.starts_with(prefix) {
            protocol = p;
            if p == "https" {
                port = 443;
            }
            rest = &value[prefix.len()..];
            break;
        }
    }
    if rest.contains("://") {
        return None;
    }
    let mut userinfo = None;
    if let Some(i) = rest.rfind('@') {
        let auth = &rest[..i];
        rest = &rest[i + 1..];
        userinfo = Some(match auth.split_once(':') {
            Some((u, p)) => (unescape(u), Some(unescape(p))),
            None => (unescape(auth), None),
        });
    }
    let ipv6_end = rest.find(']');
    let host = match rest.rfind(':') {
        Some(sep) if ipv6_end.is_none_or(|end| sep > end) => {
            let digits: String = rest[sep + 1..].chars().take_while(|c| c.is_ascii_digit()).collect();
            port = digits.parse().ok()?;
            &rest[..sep]
        }
        _ => rest.trim_end_matches('/'),
    };
    if host.is_empty() {
        return None;
    }
    let auth = match userinfo {
        Some((u, Some(p))) => format!("{}:{}@", escape_userinfo(&u), escape_userinfo(&p)),
        Some((u, None)) => format!("{}@", escape_userinfo(&u)),
        None => String::new(),
    };
    Some(format!("{protocol}://{auth}{host}:{port}"))
}

/// The manual part of `HttpWindowsProxy`: the proxy string (`MultiProxy.Parse`)
/// and the bypass list.
#[cfg_attr(not(windows), allow(dead_code))] // only `HttpWindowsProxy` (and the tests) use it
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ManualSettings {
    secure: Option<String>,
    insecure: Option<String>,
    bypass_local: bool,
    bypass: Vec<String>,
}

#[cfg_attr(not(windows), allow(dead_code))] // only `HttpWindowsProxy` (and the tests) use it
impl ManualSettings {
    pub(crate) fn parse(proxy: &str, bypass: &str) -> Self {
        let (bypass_local, bypass) = parse_bypass(bypass);
        Self { secure: parse_proxy_part(proxy, true), insecure: parse_proxy_part(proxy, false), bypass_local, bypass }
    }

    /// The manual settings' half of `HttpWindowsProxy.GetMultiProxy`.
    pub(crate) fn route(&self, target: &Target) -> Option<String> {
        if self.bypass_local {
            // Loopback is always local; an address is local when it's this
            // machine's (only loopback here), a name when it has no dot.
            if target.is_loopback() {
                return None;
            }
            let host = target.bare_host();
            if host.parse::<std::net::IpAddr>().is_err() && !host.contains('.') {
                return None;
            }
        }
        if self.bypass.iter().any(|entry| star_match(target.bare_host(), entry)) {
            return None;
        }
        if target.is_secure() { self.secure.clone() } else { self.insecure.clone() }
    }
}

/// `MultiProxy.TryParseProxyConfigPart`: the first entry for this kind of
/// request in `[http=|https=][http://|https://]host[:port]` entries
/// separated by `;` or white space, as an http proxy.
#[cfg_attr(not(windows), allow(dead_code))] // only `HttpWindowsProxy` (and the tests) use it
fn parse_proxy_part(proxy: &str, secure: bool) -> Option<String> {
    const SECURE: u8 = 1;
    const INSECURE: u8 = 2;
    let wanted = if secure { SECURE } else { INSECURE };
    for part in proxy.split([';', ' ', '\n', '\r', '\t']).filter(|p| !p.is_empty()) {
        let mut kind = SECURE | INSECURE;
        let mut rest = part;
        if let Some(r) = strip_prefix_ignore_case(rest, "http=") {
            kind = INSECURE;
            rest = r;
        } else if let Some(r) = strip_prefix_ignore_case(rest, "https=") {
            kind = SECURE;
            rest = r;
        }
        // Manual settings: a scheme on the address doesn't change which
        // requests the entry is for.
        if let Some(r) = strip_prefix_ignore_case(rest, "http://").or_else(|| strip_prefix_ignore_case(rest, "https://")) {
            rest = r;
        }
        if kind & wanted != 0 && valid_authority(rest) {
            return Some(format!("http://{rest}"));
        }
    }
    None
}

#[cfg_attr(not(windows), allow(dead_code))] // only `HttpWindowsProxy` (and the tests) use it
fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    (s.len() >= prefix.len() && s.is_char_boundary(prefix.len()) && s[..prefix.len()].eq_ignore_ascii_case(prefix))
        .then(|| &s[prefix.len()..])
}

/// `Uri.TryCreate("http://" + entry)` as far as proxy entries go: a host
/// name or address and an optional port.
#[cfg_attr(not(windows), allow(dead_code))] // only `HttpWindowsProxy` (and the tests) use it
fn valid_authority(s: &str) -> bool {
    let (host, port) = match s.rfind(':') {
        Some(i) if !s.ends_with(']') => (&s[..i], Some(&s[i + 1..])),
        _ => (s, None),
    };
    let host_ok = !host.is_empty()
        && (host.starts_with('[') && host.ends_with(']')
            || host.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '.' || c == '_'));
    host_ok && port.is_none_or(|p| p.is_empty() || p.parse::<u16>().is_ok())
}

/// `HttpWindowsProxy`'s bypass list: entries separated by `;`, each
/// without a leading http:// or https:// or an IPv6 address's brackets;
/// `<local>` turns on the local bypass.
#[cfg_attr(not(windows), allow(dead_code))] // only `HttpWindowsProxy` (and the tests) use it
fn parse_bypass(list: &str) -> (bool, Vec<String>) {
    let chars: Vec<char> = list.chars().collect();
    let mut bypass_local = false;
    let mut out = Vec::new();
    let mut idx = 0;
    let starts_with = |idx: usize, p: &str| {
        let s: String = chars[idx.min(chars.len())..].iter().take(p.len()).collect();
        s.eq_ignore_ascii_case(p)
    };
    while idx < chars.len() {
        while idx < chars.len() && chars[idx] == ' ' {
            idx += 1;
        }
        if starts_with(idx, "http://") {
            idx += 7;
        } else if starts_with(idx, "https://") {
            idx += 8;
        }
        if idx < chars.len() && chars[idx] == '[' {
            idx += 1;
        }
        let start = idx;
        while idx < chars.len() && chars[idx] != ' ' && chars[idx] != ';' && chars[idx] != ']' {
            idx += 1;
        }
        let entry: String = chars[start.min(chars.len())..idx.min(chars.len())].iter().collect();
        let tmp = if entry.is_empty() {
            None
        } else if entry.eq_ignore_ascii_case("<local>") {
            bypass_local = true;
            None
        } else {
            Some(entry)
        };
        while idx < chars.len() && chars[idx] != ';' {
            idx += 1;
        }
        if idx < chars.len() {
            idx += 1;
        }
        if let Some(t) = tmp {
            out.push(t);
        }
    }
    (bypass_local, out)
}

/// `SimpleRegex.IsMatchWithStarWildcard`: `*` matches any run of
/// characters, ignoring case.
#[cfg_attr(not(windows), allow(dead_code))] // only `HttpWindowsProxy` (and the tests) use it
fn star_match(input: &str, pattern: &str) -> bool {
    let input: Vec<char> = input.to_lowercase().chars().collect();
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let (mut i, mut p) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while i < input.len() {
        if p < pattern.len() && pattern[p] != '*' && pattern[p] == input[i] {
            i += 1;
            p += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = i;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            i = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(windows)]
mod windows {
    //! `HttpWindowsProxy` and `WinInetProxyHelper`.

    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{GetLastError, GlobalFree};
    use windows_sys::Win32::Networking::WinHttp::*;

    use super::{ManualSettings, Target};

    const ERROR_WINHTTP_LOGIN_FAILURE: u32 = 12015;
    const ERROR_WINHTTP_AUTODETECTION_FAILED: u32 = 12180;
    /// `WinInetProxyHelper.RecentAutoDetectionInterval`.
    const RECENT_AUTO_DETECTION_INTERVAL: Duration = Duration::from_secs(120);

    static AUTO_DETECTION_FAILED: Mutex<Option<Instant>> = Mutex::new(None);

    struct Session(*mut core::ffi::c_void);
    // SAFETY: a WinHTTP session handle may be used from any thread.
    unsafe impl Send for Session {}
    unsafe impl Sync for Session {}

    /// The session `WinHttpGetProxyForUrl` runs in (`HttpWindowsProxy.TryCreate`).
    fn session() -> Option<*mut core::ffi::c_void> {
        static SESSION: OnceLock<Session> = OnceLock::new();
        let s = SESSION.get_or_init(|| {
            // SAFETY: null strings are allowed for the agent, proxy and bypass.
            Session(unsafe { WinHttpOpen(std::ptr::null(), WINHTTP_ACCESS_TYPE_NO_PROXY, std::ptr::null(), std::ptr::null(), 0) })
        });
        (!s.0.is_null()).then_some(s.0)
    }

    /// Takes a string WinHTTP allocated, and frees it.
    fn take(p: *mut u16) -> String {
        if p.is_null() {
            return String::new();
        }
        // SAFETY: WinHTTP returns null-terminated strings allocated with GlobalAlloc.
        unsafe {
            let len = (0..).take_while(|&i| *p.add(i) != 0).count();
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            GlobalFree(p as _);
            s
        }
    }

    /// `WinInetProxyHelper`: the user's Internet settings.
    struct Helper {
        auto_detect: bool,
        auto_config_url: String,
        proxy: String,
        proxy_bypass: String,
    }

    impl Helper {
        fn read() -> Self {
            let mut c = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
            // SAFETY: `c` is a valid out pointer; its strings are freed by `take`.
            if unsafe { WinHttpGetIEProxyConfigForCurrentUser(&mut c) } == 0 {
                return Self { auto_detect: false, auto_config_url: String::new(), proxy: String::new(), proxy_bypass: String::new() };
            }
            Self {
                auto_detect: c.fAutoDetect != 0,
                auto_config_url: take(c.lpszAutoConfigUrl),
                proxy: take(c.lpszProxy),
                proxy_bypass: take(c.lpszProxyBypass),
            }
        }

        fn auto_settings_used(&self) -> bool {
            self.auto_detect || !self.auto_config_url.is_empty()
        }

        fn manual_settings_used(&self) -> bool {
            !self.proxy.is_empty()
        }

        fn recent_auto_detection_failure() -> bool {
            AUTO_DETECTION_FAILED.lock().expect("auto detection").is_some_and(|t| t.elapsed() <= RECENT_AUTO_DETECTION_INTERVAL)
        }

        /// `GetProxyForUrl`: WinHTTP's answer (the proxy list and bypass
        /// list), the manual settings if it has none, or `None`.
        fn get_proxy_for_url(&self, session: *mut core::ffi::c_void, url: &str) -> Option<(String, String)> {
            let config_url: Vec<u16> = self.auto_config_url.encode_utf16().chain([0]).collect();
            let wide_url: Vec<u16> = url.encode_utf16().chain([0]).collect();
            let mut options = WINHTTP_AUTOPROXY_OPTIONS {
                dwFlags: if self.auto_detect { WINHTTP_AUTOPROXY_AUTO_DETECT } else { 0 }
                    | if self.auto_config_url.is_empty() { 0 } else { WINHTTP_AUTOPROXY_CONFIG_URL },
                dwAutoDetectFlags: if self.auto_detect { WINHTTP_AUTO_DETECT_TYPE_DHCP | WINHTTP_AUTO_DETECT_TYPE_DNS_A } else { 0 },
                lpszAutoConfigUrl: if self.auto_config_url.is_empty() { std::ptr::null() } else { config_url.as_ptr() },
                lpvReserved: std::ptr::null_mut(),
                dwReserved: 0,
                fAutoLogonIfChallenged: 0,
            };
            let mut repeat = false;
            loop {
                let mut info = WINHTTP_PROXY_INFO::default();
                // SAFETY: valid session, null-terminated URL, options and out pointer.
                if unsafe { WinHttpGetProxyForUrl(session, wide_url.as_ptr(), &mut options, &mut info) } != 0 {
                    return Some((take(info.lpszProxy), take(info.lpszProxyBypass)));
                }
                // SAFETY: no other call in between.
                let error = unsafe { GetLastError() };
                if error == ERROR_WINHTTP_LOGIN_FAILURE && !repeat {
                    // Cached results only without auto logon: try that first.
                    repeat = true;
                    options.fAutoLogonIfChallenged = 1;
                    continue;
                }
                if error == ERROR_WINHTTP_AUTODETECTION_FAILED {
                    *AUTO_DETECTION_FAILED.lock().expect("auto detection") = Some(Instant::now());
                }
                break;
            }
            self.manual_settings_used().then(|| (self.proxy.clone(), self.proxy_bypass.clone()))
        }
    }

    /// `HttpWindowsProxy.GetMultiProxy`'s first proxy.
    pub(super) fn system_proxy(target: &Target, url: &str) -> Option<String> {
        let helper = Helper::read();
        if !helper.auto_settings_used() && !helper.manual_settings_used() {
            return None;
        }
        if helper.auto_settings_used() && !Helper::recent_auto_detection_failure() {
            let session = session()?;
            let (proxy, bypass) = helper.get_proxy_for_url(session, url)?;
            if bypass.is_empty() {
                // A PAC answer: a list of proxies, or none (direct).
                return super::parse_proxy_part(&proxy, target.is_secure());
            }
            // WinHTTP fell back to the manual settings: they decide below.
        }
        if helper.manual_settings_used() {
            return ManualSettings::parse(&helper.proxy, &helper.proxy_bypass).route(target);
        }
        None
    }
}

#[cfg(target_os = "macos")]
mod macos {
    //! `MacProxy`: CFNetwork's proxies for the URL.

    use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
    use core_foundation_sys::base::{CFEqual, CFGetTypeID, CFRelease, CFTypeRef};
    use core_foundation_sys::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
    use core_foundation_sys::number::{CFNumberGetTypeID, CFNumberGetValue, CFNumberRef, kCFNumberSInt32Type};
    use core_foundation_sys::string::{CFStringGetCString, CFStringGetTypeID, CFStringRef, kCFStringEncodingUTF8};
    use core_foundation_sys::url::{CFURLCreateWithBytes, CFURLRef};

    #[link(name = "CFNetwork", kind = "framework")]
    unsafe extern "C" {
        fn CFNetworkCopySystemProxySettings() -> CFDictionaryRef;
        fn CFNetworkCopyProxiesForURL(url: CFURLRef, proxy_settings: CFDictionaryRef) -> CFArrayRef;
        static kCFProxyTypeKey: CFStringRef;
        static kCFProxyHostNameKey: CFStringRef;
        static kCFProxyPortNumberKey: CFStringRef;
        static kCFProxyUsernameKey: CFStringRef;
        static kCFProxyPasswordKey: CFStringRef;
        static kCFProxyTypeNone: CFStringRef;
        static kCFProxyTypeHTTP: CFStringRef;
        static kCFProxyTypeHTTPS: CFStringRef;
        static kCFProxyTypeSOCKS: CFStringRef;
    }

    fn string(value: CFTypeRef) -> Option<String> {
        // SAFETY: `value` is a CF object or null; the buffer is big enough for
        // host names, user names and passwords.
        unsafe {
            if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
                return None;
            }
            let mut buf = vec![0u8; 1024];
            if CFStringGetCString(value as CFStringRef, buf.as_mut_ptr() as _, buf.len() as _, kCFStringEncodingUTF8) == 0 {
                return None;
            }
            let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            Some(String::from_utf8_lossy(&buf[..len]).into_owned())
        }
    }

    fn number(value: CFTypeRef) -> Option<i32> {
        // SAFETY: as above.
        unsafe {
            if value.is_null() || CFGetTypeID(value) != CFNumberGetTypeID() {
                return None;
            }
            let mut n: i32 = 0;
            CFNumberGetValue(value as CFNumberRef, kCFNumberSInt32Type, &mut n as *mut i32 as _).then_some(n)
        }
    }

    pub(super) fn system_proxy(url: &str) -> Option<String> {
        // SAFETY: every CF object created here is released; values got from
        // the dictionaries and array are borrowed.
        unsafe {
            let settings = CFNetworkCopySystemProxySettings();
            if settings.is_null() {
                return None;
            }
            let cf_url = CFURLCreateWithBytes(std::ptr::null(), url.as_ptr(), url.len() as _, kCFStringEncodingUTF8, std::ptr::null());
            if cf_url.is_null() {
                CFRelease(settings as _);
                return None;
            }
            let proxies = CFNetworkCopyProxiesForURL(cf_url, settings);
            let mut result = None;
            if !proxies.is_null() {
                for i in 0..CFArrayGetCount(proxies) {
                    let proxy = CFArrayGetValueAtIndex(proxies, i) as CFDictionaryRef;
                    let kind = CFDictionaryGetValue(proxy, kCFProxyTypeKey as _);
                    if kind.is_null() {
                        continue;
                    }
                    if CFEqual(kind, kCFProxyTypeNone as _) != 0 {
                        break;
                    }
                    let scheme = if CFEqual(kind, kCFProxyTypeHTTP as _) != 0 || CFEqual(kind, kCFProxyTypeHTTPS as _) != 0 {
                        "http"
                    } else if CFEqual(kind, kCFProxyTypeSOCKS as _) != 0 {
                        "socks5"
                    } else {
                        // PAC entries: not run here.
                        continue;
                    };
                    let Some(host) = string(CFDictionaryGetValue(proxy, kCFProxyHostNameKey as _)) else { continue };
                    let port = number(CFDictionaryGetValue(proxy, kCFProxyPortNumberKey as _));
                    let user = string(CFDictionaryGetValue(proxy, kCFProxyUsernameKey as _));
                    let password = string(CFDictionaryGetValue(proxy, kCFProxyPasswordKey as _));
                    let mut uri = format!("{scheme}://{host}");
                    if let Some(p) = port.filter(|&p| p > 0) {
                        uri.push_str(&format!(":{p}"));
                    }
                    if let Some(u) = user.filter(|u| !u.is_empty()) {
                        let c = super::Credentials { username: u, password: password.unwrap_or_default() };
                        uri = super::with_credentials(&uri, Some(&c));
                    }
                    result = Some(uri);
                    break;
                }
                CFRelease(proxies as _);
            }
            CFRelease(cf_url as _);
            CFRelease(settings as _);
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(method: ProxyMethod, host: &str, port: i64, user: &str, password: &str) -> ProxyInfo {
        ProxyInfo { proxy_method: method, host: host.into(), port, username: user.into(), password: password.into() }
    }

    fn creds(u: &str, p: &str) -> Option<Credentials> {
        Some(Credentials { username: u.into(), password: p.into() })
    }

    #[test]
    fn only_none_connects_directly() {
        assert_eq!(info(ProxyMethod::None, "proxy", 8080, "u", "p").plan(), ProxyPlan::Direct);
        assert_eq!(ProxyPlan::Direct.route("https://example.com/"), None);
        // The default `ProxyInfo` (Manual without a host) is the system proxy.
        assert_eq!(ProxyInfo::default().plan(), ProxyPlan::System { credentials: None });
        assert_eq!(info(ProxyMethod::Manual, "proxy", 0, "u", "p").plan(), ProxyPlan::System { credentials: None });
        assert_eq!(info(ProxyMethod::Manual, "", 8080, "", "").plan(), ProxyPlan::System { credentials: None });
    }

    #[test]
    fn automatic_passes_credentials_only_with_a_user_name() {
        assert_eq!(info(ProxyMethod::Automatic, "", 0, "user", "pa ss").plan(), ProxyPlan::System { credentials: creds("user", "pa ss") });
        assert_eq!(info(ProxyMethod::Automatic, "", 0, "", "secret").plan(), ProxyPlan::System { credentials: None });
        // Automatic ignores a host and port left from Manual.
        assert_eq!(info(ProxyMethod::Automatic, "proxy", 8080, "", "").plan(), ProxyPlan::System { credentials: None });
    }

    #[test]
    fn is_valid_proxy_is_only_manual_with_host_and_port() {
        assert!(info(ProxyMethod::Manual, "proxy", 8080, "", "").is_valid_proxy());
        assert!(!info(ProxyMethod::Automatic, "proxy", 8080, "", "").is_valid_proxy());
        assert!(!info(ProxyMethod::None, "proxy", 8080, "", "").is_valid_proxy());
        assert!(!info(ProxyMethod::Manual, "proxy", -1, "", "").is_valid_proxy());
    }

    #[test]
    fn manual_proxy_bypasses_local_hosts() {
        let plan = info(ProxyMethod::Manual, "proxy.lan", 3128, "me", "p@ss").plan();
        assert_eq!(plan, ProxyPlan::Manual { uri: "http://proxy.lan:3128".into(), credentials: creds("me", "p@ss") });
        assert_eq!(plan.route("https://example.com/upload").as_deref(), Some("http://me:p%40ss@proxy.lan:3128"));
        assert_eq!(plan.route("http://localhost:8080/"), None);
        assert_eq!(plan.route("http://127.0.0.1:8080/"), None);
        assert_eq!(plan.route("http://[::1]:8080/"), None);
        assert_eq!(plan.route("http://intranet/x"), None);
        assert!(plan.route("http://10.0.0.5/").is_some());
        let socks = info(ProxyMethod::Manual, "socks5://proxy", 1080, "", "").plan();
        assert_eq!(socks.route("https://example.com/").as_deref(), Some("socks5://proxy:1080"));
    }

    #[test]
    fn reads_proxy_settings_json() {
        let v = serde_json::json!({"ProxyMethod": "Automatic", "Host": null, "Port": 0, "Username": "u", "Password": null});
        assert_eq!(ProxyInfo::from_json(&v), info(ProxyMethod::Automatic, "", 0, "u", ""));
        assert_eq!(ProxyInfo::from_json(&serde_json::Value::Null), ProxyInfo::default());
    }

    #[test]
    fn target_parts() {
        assert_eq!(Target::parse("HTTPS://User:pw@Example.COM:443/a?b").unwrap(), Target { scheme: "https".into(), host: "example.com".into() });
        assert_eq!(Target::parse("http://[::1]:80/").unwrap().host, "[::1]");
        assert!(Target::parse("wss://x/").unwrap().is_secure());
    }

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: Vec<(String, String)> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }

    #[test]
    fn environment_proxy() {
        assert_eq!(EnvProxy::from_env(&env(&[]), false), None);
        let e = EnvProxy::from_env(&env(&[("http_proxy", "proxy:3128"), ("HTTPS_PROXY", "https://u%40x:p@secure"), ("no_proxy", " .corp.example, host.example ,")]), false).unwrap();
        let t = |u: &str| Target::parse(u).unwrap();
        assert_eq!(e.route(&t("http://example.com/")).as_deref(), Some("http://proxy:3128"));
        assert_eq!(e.route(&t("https://example.com/")).as_deref(), Some("https://u%40x:p@secure:443"));
        assert_eq!(e.route(&t("http://corp.example/")), None);
        assert_eq!(e.route(&t("http://a.corp.example/")), None);
        assert_eq!(e.route(&t("http://HOST.example/")), None);
        assert!(e.route(&t("http://xhost.example/")).is_some());
        // all_proxy fills in the missing one; unknown schemes are ignored.
        let e = EnvProxy::from_env(&env(&[("all_proxy", "socks5://s:1080"), ("http_proxy", "ftp://x")]), false).unwrap();
        assert_eq!(e.route(&t("http://a/")).as_deref(), Some("socks5://s:1080"));
        // Windows reads only the upper-case names; CGI ignores the http one.
        assert_eq!(EnvProxy::from_env(&env(&[("http_proxy", "p:1")]), true), None);
        assert_eq!(EnvProxy::from_env(&env(&[("HTTP_PROXY", "p:1"), ("GATEWAY_INTERFACE", "CGI/1.1")]), false), None);
    }

    #[test]
    fn windows_manual_settings() {
        let m = ManualSettings::parse("http=web:80;https=secure:443", "<local>;*.example.com; [::2] ;http://skip.me");
        let t = |u: &str| Target::parse(u).unwrap();
        assert_eq!(m.route(&t("http://a.org/")).as_deref(), Some("http://web:80"));
        assert_eq!(m.route(&t("https://a.org/")).as_deref(), Some("http://secure:443"));
        assert_eq!(m.route(&t("https://www.example.com/")), None);
        assert_eq!(m.route(&t("http://intranet/")), None);
        assert_eq!(m.route(&t("http://127.0.0.1/")), None);
        assert_eq!(m.route(&t("http://[::2]/")), None);
        assert_eq!(m.route(&t("http://skip.me/")), None);
        let m = ManualSettings::parse("proxy:8080", "");
        assert_eq!(m.route(&t("http://intranet/")).as_deref(), Some("http://proxy:8080"));
        assert_eq!(m.route(&t("https://a.org/")).as_deref(), Some("http://proxy:8080"));
        assert_eq!(ManualSettings::parse("socks=s:1080", "").route(&t("http://a.org/")), None);
    }

    #[test]
    fn star_wildcards() {
        assert!(star_match("www.example.com", "*.example.com"));
        assert!(!star_match("example.com", "*.example.com"));
        assert!(star_match("10.1.2.3", "10.*"));
        assert!(star_match("Host", "host"));
        assert!(!star_match("hostx", "host"));
    }
}
