//! A tiny HTTP/1.1 server on 127.0.0.1 standing in for GitHub's API and
//! its asset downloads: canned responses by path, every request recorded.
//! Nothing leaves the machine.

#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Default)]
pub struct Response {
    pub status: u16,
    pub reason: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Write the body this many bytes at a time, pausing between.
    pub chunk: usize,
    pub chunk_delay: Duration,
    /// Send a `Content-Length` (true unless a test leaves it out).
    pub no_length: bool,
}

impl Response {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self { status: 200, reason: "OK", body: body.into(), ..Default::default() }
    }

    pub fn json(body: &str) -> Self {
        let mut r = Self::ok(body.as_bytes().to_vec());
        r.headers.push(("Content-Type".into(), "application/json; charset=utf-8".into()));
        r
    }

    pub fn status(status: u16, reason: &'static str, body: &str) -> Self {
        Self { status, reason, body: body.as_bytes().to_vec(), ..Default::default() }
    }
}

#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

pub struct Mock {
    pub base: String,
    routes: Arc<Mutex<HashMap<String, Response>>>,
    log: Arc<Mutex<Vec<Recorded>>>,
}

impl Mock {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes: Arc<Mutex<HashMap<String, Response>>> = Arc::default();
        let log: Arc<Mutex<Vec<Recorded>>> = Arc::default();
        let (r, l) = (routes.clone(), log.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (r, l) = (r.clone(), l.clone());
                std::thread::spawn(move || serve(stream, r, l));
            }
        });
        Self { base, routes, log }
    }

    pub fn route(&self, path: &str, response: Response) {
        self.routes.lock().unwrap().insert(path.to_owned(), response);
    }

    pub fn log(&self) -> Vec<Recorded> {
        self.log.lock().unwrap().clone()
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }
}

fn serve(stream: TcpStream, routes: Arc<Mutex<HashMap<String, Response>>>, log: Arc<Mutex<Vec<Recorded>>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.is_empty() {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_owned();
    let path = parts.next().unwrap_or("").to_owned();
    let mut headers = Vec::new();
    let mut length = 0usize;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h.trim_end().is_empty() {
            break;
        }
        if let Some((k, v)) = h.trim_end().split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                length = v.trim().parse().unwrap_or(0);
            }
            headers.push((k.to_owned(), v.trim().to_owned()));
        }
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    log.lock().unwrap().push(Recorded { method, path: path.clone(), headers });
    let response = routes
        .lock()
        .unwrap()
        .get(&path)
        .cloned()
        .unwrap_or_else(|| Response::status(404, "Not Found", r#"{"message":"Not Found"}"#));
    let mut stream = stream;
    let mut head = format!("HTTP/1.1 {} {}\r\nConnection: close\r\n", response.status, response.reason);
    for (k, v) in &response.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    if !response.no_length {
        head.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
    }
    head.push_str("\r\n");
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    if response.chunk == 0 {
        let _ = stream.write_all(&response.body);
    } else {
        for part in response.body.chunks(response.chunk) {
            if stream.write_all(part).is_err() {
                return;
            }
            let _ = stream.flush();
            std::thread::sleep(response.chunk_delay);
        }
    }
    let _ = stream.flush();
}
