//! Requests through `set_proxy` (ShareX #9319) against a mock proxy on
//! 127.0.0.1: a manual proxy, the system proxy from the environment with
//! Automatic's credentials, and None going direct. Nothing leaves the
//! machine (the targets are `.invalid` names only the mock answers for).
//! Its own test binary: it changes the process's proxy and environment.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;

use sharex_settings::ProxyMethod;
use sharex_uploaders::enums::HttpMethod;
use sharex_uploaders::http::{self, HttpRequest, ProxyInfo, RequestOptions};

/// A proxy that answers one connection: a `CONNECT` (then the tunnelled
/// request) or an absolute-form request. Sends back the first request line
/// and the `Proxy-Authorization` header.
fn mock_proxy() -> (u16, mpsc::Receiver<(String, Option<String>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        while let Ok((stream, _)) = listener.accept() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;
            let read_head = |reader: &mut BufReader<_>| {
                let mut lines = Vec::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    lines.push(line.trim_end().to_owned());
                }
                lines
            };
            let head = read_head(&mut reader);
            let Some(first) = head.first().cloned() else { continue };
            let auth = head
                .iter()
                .find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("Proxy-Authorization")).map(|(_, v)| v.trim().to_owned()));
            if first.starts_with("CONNECT ") {
                writer.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").unwrap();
                read_head(&mut reader);
            }
            writer.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").unwrap();
            let _ = tx.send((first, auth));
        }
    });
    (port, rx)
}

fn get(url: &str) -> Result<String, String> {
    let request = HttpRequest { method: HttpMethod::GET, url: url.into(), headers: Vec::new(), content: None };
    let options = RequestOptions { read_body: true, ..Default::default() };
    http::send(request, options).map(|r| r.response_text.unwrap_or_default()).map_err(|e| e.to_string())
}

fn proxy(method: ProxyMethod, host: &str, port: i64, user: &str, password: &str) -> ProxyInfo {
    ProxyInfo { proxy_method: method, host: host.into(), port, username: user.into(), password: password.into() }
}

#[test]
fn requests_go_through_the_chosen_proxy() {
    for name in ["http_proxy", "https_proxy", "all_proxy", "no_proxy", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY", "GATEWAY_INTERFACE"] {
        // SAFETY: this test binary's only test; nothing else reads the environment meanwhile.
        unsafe { std::env::remove_var(name) };
    }
    let (port, seen) = mock_proxy();

    // Manual: the proxy as given, with its credentials.
    http::set_proxy(&proxy(ProxyMethod::Manual, "127.0.0.1", port as i64, "me", "pw"));
    assert_eq!(get("http://upload.example.invalid/a").as_deref(), Ok("ok"));
    let (line, auth) = seen.recv().unwrap();
    assert!(line.contains("upload.example.invalid"), "{line}");
    assert_eq!(auth.as_deref(), Some("Basic bWU6cHc="));

    // Automatic: the system proxy (here the environment's), with the
    // credentials from the settings. The host and port left from Manual
    // aren't used.
    let var = if cfg!(windows) { "HTTP_PROXY" } else { "http_proxy" };
    // SAFETY: as above.
    unsafe { std::env::set_var(var, format!("127.0.0.1:{port}")) };
    http::set_proxy(&proxy(ProxyMethod::Automatic, "unused.invalid", 1, "auto", "secret"));
    assert_eq!(get("http://other.example.invalid/b").as_deref(), Ok("ok"));
    let (line, auth) = seen.recv().unwrap();
    assert!(line.contains("other.example.invalid"), "{line}");
    assert_eq!(auth.as_deref(), Some("Basic YXV0bzpzZWNyZXQ="));

    // Manual without a host (the default): the system proxy, no credentials.
    http::set_proxy(&ProxyInfo::default());
    assert_eq!(get("http://third.example.invalid/c").as_deref(), Ok("ok"));
    let (line, auth) = seen.recv().unwrap();
    assert!(line.contains("third.example.invalid"), "{line}");
    assert_eq!(auth, None);

    // None: directly, so the .invalid name doesn't resolve and the proxy
    // sees nothing.
    http::set_proxy(&proxy(ProxyMethod::None, "127.0.0.1", port as i64, "", ""));
    assert!(get("http://direct.example.invalid/d").is_err());
    assert!(seen.recv_timeout(std::time::Duration::from_millis(300)).is_err());
}
