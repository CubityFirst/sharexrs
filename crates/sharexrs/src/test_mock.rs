//! The 127.0.0.1 mock server the app's tests talk to instead of real
//! services: `tools/uploaders-harness/mock_server.py` on a free port, and
//! [`rewrite_to`], which sends every built-in destination's requests to it
//! through `http::set_url_rewriter`. Nothing leaves the machine.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use sharex_uploaders::http;

pub(crate) struct Mock {
    child: Child,
    pub(crate) port: u16,
    _stdout: BufReader<ChildStdout>,
}

impl Mock {
    /// `python mock_server.py 0`: its first line says the port it got.
    pub(crate) fn start() -> Mock {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/uploaders-harness/mock_server.py");
        let candidates: &[&str] = if cfg!(windows) { &["python", "python3", "py"] } else { &["python3", "python"] };
        for exe in candidates {
            let Ok(mut child) = Command::new(exe).arg(&script).arg("0").stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
                continue;
            };
            let mut reader = BufReader::new(child.stdout.take().expect("stdout"));
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok()
                && let Some(port) = line.trim().rsplit(':').next().and_then(|p| p.parse().ok())
            {
                return Mock { child, port, _stdout: reader };
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        panic!("couldn't start tools/uploaders-harness/mock_server.py: Python 3 is needed (python3 or python on PATH)");
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub(crate) fn control(&self, path: &str, body: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).expect("mock server");
        let method = if body.is_empty() { "GET" } else { "POST" };
        write!(
            s,
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("write");
        let mut out = String::new();
        s.read_to_string(&mut out).expect("read");
        out.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default()
    }

    /// Queues the responses (and the server's options).
    pub(crate) fn reset(&self, spec: Value) {
        self.control("/__mock/reset", &spec.to_string());
    }

    /// The requests since the last reset.
    pub(crate) fn log(&self) -> Vec<Request> {
        let log: Vec<Value> = serde_json::from_str(&self.control("/__mock/log", "")).expect("mock log");
        log.iter().map(Request::from_json).collect()
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A request as the mock received it.
#[derive(Debug)]
pub(crate) struct Request {
    pub(crate) method: String,
    pub(crate) target: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl Request {
    fn from_json(v: &Value) -> Self {
        use base64::Engine;
        Request {
            method: v["method"].as_str().unwrap_or_default().to_owned(),
            target: v["target"].as_str().unwrap_or_default().to_owned(),
            headers: v["headers"]
                .as_array()
                .map(|a| a.iter().map(|h| (h[0].as_str().unwrap_or_default().to_owned(), h[1].as_str().unwrap_or_default().to_owned())).collect())
                .unwrap_or_default(),
            body: base64::engine::general_purpose::STANDARD.decode(v["body_base64"].as_str().unwrap_or_default()).unwrap_or_default(),
        }
    }

    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub(crate) fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub(crate) fn ok(body: &str) -> Value {
    json!({ "status": 200, "reason": "OK", "headers": [["Content-Type", "application/json"]], "body": body })
}

pub(crate) fn text_ok(body: &str) -> Value {
    json!({ "status": 200, "reason": "OK", "headers": [["Content-Type", "text/plain"]], "body": body })
}

pub(crate) fn server_error() -> Value {
    json!({ "status": 500, "reason": "Internal Server Error", "headers": [["Content-Type", "text/plain"]], "body": "made-up failure" })
}

// ---------------------------------------------------------------------------
// Built-in destinations, through the URL rewriter (one test at a time).

static REWRITER: Mutex<()> = Mutex::new(());

/// Sends every URL but 127.0.0.1's to the mock as
/// `/__/<scheme>/<host><path>`, as the harness does.
pub(crate) fn rewrite_to(mock: &Mock) -> std::sync::MutexGuard<'static, ()> {
    let guard = REWRITER.lock().unwrap_or_else(|e| e.into_inner());
    let base = mock.url("");
    let back_base = format!("{base}/__/");
    http::set_url_rewriter(Some(http::UrlRewriter {
        forward: Arc::new(move |u: &str| {
            if u.starts_with("http://127.0.0.1") {
                return u.to_owned();
            }
            let (scheme, rest) = u.split_once("://").unwrap_or(("https", u));
            format!("{base}/__/{scheme}/{rest}")
        }),
        back: Arc::new(move |u: &str| match u.strip_prefix(&back_base).and_then(|r| r.split_once('/')) {
            Some((scheme, rest)) => format!("{scheme}://{rest}"),
            None => u.to_owned(),
        }),
    }));
    guard
}

/// [`rewrite_to`] for as long as it's held: the rewriter is taken away
/// when it's dropped, also when a test fails.
pub(crate) struct Rewritten(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

impl Drop for Rewritten {
    fn drop(&mut self) {
        http::set_url_rewriter(None);
    }
}

pub(crate) fn rewritten(mock: &Mock) -> Rewritten {
    Rewritten(rewrite_to(mock))
}
