//! Progress reports and stopping an upload, against a local socket that
//! reads slowly (nothing leaves the machine).

use std::io::{Cursor, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sharex_uploaders::config::PomfUploader;
use sharex_uploaders::destinations::files::Pomf;
use sharex_uploaders::http;
use sharex_uploaders::uploader::{FileUploader, GenericUploader};

/// A server that reads the request `chunk` bytes every `delay`, then answers
/// with a Pomf-style JSON response.
fn slow_server(chunk: usize, delay: Duration) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            std::thread::spawn(move || {
                let mut received = Vec::new();
                let mut buf = vec![0u8; chunk];
                let mut expected: Option<usize> = None;
                loop {
                    std::thread::sleep(delay);
                    let n = match stream.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => n,
                    };
                    received.extend_from_slice(&buf[..n]);
                    if expected.is_none()
                        && let Some(end) = received.windows(4).position(|w| w == b"\r\n\r\n")
                    {
                        let head = String::from_utf8_lossy(&received[..end]).to_lowercase();
                        let len = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                            .unwrap_or(0);
                        expected = Some(end + 4 + len);
                    }
                    if expected.is_some_and(|e| received.len() >= e) {
                        break;
                    }
                }
                let body = r#"{"success":true,"files":[{"url":"https://example.com/f.bin"}]}"#;
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            });
        }
    });
    port
}

fn pomf(port: u16) -> Pomf {
    Pomf::new(PomfUploader { UploadURL: Some(format!("http://127.0.0.1:{port}/upload")), ..Default::default() })
}

#[test]
fn progress_is_reported_and_ends_at_the_length() {
    // ~1.5 s of upload: ProgressManager reports at most every 250 ms (and at the end).
    let port = slow_server(128 * 1024, Duration::from_millis(20));
    let len = 8 * 1024 * 1024;
    let data = vec![7u8; len];
    let mut uploader = pomf(port);
    uploader.base.buffer_size = 16 * 1024;
    let seen: Arc<Mutex<Vec<(u64, u64)>>> = Arc::default();
    let s = seen.clone();
    uploader.base.on_progress(Arc::new(move |p| s.lock().unwrap().push((p.position, p.length))));
    let mut g = GenericUploader::File(Box::new(uploader));
    let result = g.upload(&http::shared_stream(Cursor::new(data)), "a.bin").unwrap().unwrap();
    assert_eq!(result.url.as_deref(), Some("https://example.com/f.bin"));
    let seen = seen.lock().unwrap();
    assert!(seen.len() >= 2, "progress reported {} times", seen.len());
    assert!(seen.windows(2).all(|w| w[0].0 <= w[1].0), "progress goes forward");
    assert_eq!(*seen.last().unwrap(), (len as u64, len as u64));
}

#[test]
fn stopping_returns_at_once_with_no_result() {
    let port = slow_server(1024, Duration::from_millis(50));
    let data = vec![1u8; 4 * 1024 * 1024];
    let uploader = pomf(port);
    let stop = uploader.base.stop_handle();
    let mut g = GenericUploader::File(Box::new(uploader));
    let started = Instant::now();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        stop.cancel();
    });
    let result = g.upload(&http::shared_stream(Cursor::new(data)), "a.bin");
    assert_eq!(result, Ok(None));
    assert!(started.elapsed() < Duration::from_secs(3), "stopped after {:?}", started.elapsed());
    assert!(g.uploader().errors.is_empty(), "a stop isn't an error");
}

#[test]
fn a_refused_connection_is_an_error_not_a_panic() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let mut uploader = pomf(port);
    let result = FileUploader::upload_core(&mut uploader, &http::shared_stream(Cursor::new(vec![1u8; 10])), "a.bin").unwrap().unwrap();
    assert!(!result.is_success());
    let text = &uploader.base.errors.errors[0].text;
    assert!(text.starts_with("Error message:"), "{text}");
    assert!(text.contains(&format!("http://127.0.0.1:{port}/upload")), "{text}");
}
