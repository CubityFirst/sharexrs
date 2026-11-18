//! The non-HTTP mock servers of `tools/uploaders-harness` (FTP, FTPS
//! explicit and implicit, SFTP, SMTP), on fixed ports of 127.0.0.1, with a
//! small HTTP control port both harnesses use like the HTTP mock's:
//!
//! - `POST /reset` `{"user", "password", "dirs", "fail_once": [[prefix, reply]]}`
//!   clears the logs and files and sets up the next case,
//! - `GET /log` gives `{"ftp": [lines], "files": {path: base64}}`.
//!
//! ```text
//! cargo run -p sharex-uploaders --example sharex_uploaders_mock_servers -- [base port]
//! ```
//! Ports: base (control), base+1 FTP, base+2 FTPS explicit, base+3 FTPS
//! implicit, base+4 SFTP, base+5 SMTP with STARTTLS (default base 18820).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

use base64::Engine;
use serde_json::{Value, json};
use sharex_uploaders::testing::ftp::{FtpMockConfig, FtpMockServer, test_tls_acceptor};
use sharex_uploaders::testing::sftp::{SftpMockConfig, SftpMockServer};
use sharex_uploaders::testing::smtp::{SmtpMockConfig, SmtpMockServer};

fn config(v: &Value, tls: bool, implicit: bool) -> FtpMockConfig {
    FtpMockConfig {
        user: v["user"].as_str().unwrap_or("user").to_owned(),
        password: v["password"].as_str().unwrap_or("pass").to_owned(),
        dirs: v["dirs"].as_array().map(|a| a.iter().filter_map(|d| d.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
        tls: if tls { test_tls_acceptor() } else { None },
        implicit_tls: implicit,
        fail_once: v["fail_once"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| Some((p[0].as_str()?.to_owned(), p[1].as_str()?.to_owned()))).collect())
            .unwrap_or_default(),
    }
}

fn sftp_config(v: &Value) -> SftpMockConfig {
    SftpMockConfig {
        user: v["user"].as_str().unwrap_or("user").to_owned(),
        password: v["password"].as_str().unwrap_or("pass").to_owned(),
        authorized_key: v["authorized_key"].as_str().map(str::to_owned),
        home: v["home"].as_str().unwrap_or("/home/user").to_owned(),
        dirs: v["dirs"].as_array().map(|a| a.iter().filter_map(|d| d.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
        fail_once: v["sftp_fail_once"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| Some((p[0].as_str()?.to_owned(), (p[1].as_u64()? as u32, p[2].as_str()?.to_owned())))).collect())
            .unwrap_or_default(),
    }
}

fn smtp_config(v: &Value) -> SmtpMockConfig {
    SmtpMockConfig {
        user: v["user"].as_str().unwrap_or("user").to_owned(),
        password: v["password"].as_str().unwrap_or("pass").to_owned(),
        tls: test_tls_acceptor(),
        fail_once: v["smtp_fail_once"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| Some((p[0].as_str()?.to_owned(), p[1].as_str()?.to_owned()))).collect())
            .unwrap_or_default(),
    }
}

fn main() {
    let base: u16 = std::env::args().nth(1).and_then(|p| p.parse().ok()).unwrap_or(18820);
    let ftp = FtpMockServer::start_on(base + 1, config(&json!({}), false, false)).expect("FTP port");
    let ftps = FtpMockServer::start_on(base + 2, config(&json!({}), true, false)).expect("FTPS port");
    let ftps_implicit = FtpMockServer::start_on(base + 3, config(&json!({}), true, true)).expect("FTPS implicit port");
    let sftp = SftpMockServer::start_on(base + 4, sftp_config(&json!({}))).expect("SFTP port");
    let smtp = SmtpMockServer::start_on(base + 5, smtp_config(&json!({}))).expect("SMTP port");
    let servers = [&ftp, &ftps, &ftps_implicit];
    let control = TcpListener::bind(("127.0.0.1", base)).expect("control port");
    println!("mock servers on 127.0.0.1:{base} (control), FTP {}, FTPS {}, FTPS implicit {}", base + 1, base + 2, base + 3);
    for stream in control.incoming().flatten() {
        let mut reader = BufReader::new(stream.try_clone().expect("stream"));
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            continue;
        }
        let mut length = 0usize;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                break;
            }
            if let Some((k, v)) = header.split_once(':')
                && k.trim().eq_ignore_ascii_case("content-length")
            {
                length = v.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0u8; length];
        let _ = reader.read_exact(&mut body);
        let response = if request_line.starts_with("POST /reset") {
            let v: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
            ftp.reset(Some(config(&v, false, false)));
            ftps.reset(Some(config(&v, true, false)));
            ftps_implicit.reset(Some(config(&v, true, true)));
            sftp.reset(Some(sftp_config(&v)));
            smtp.reset(Some(smtp_config(&v)));
            "{}".to_owned()
        } else if request_line.starts_with("GET /log") {
            let mut lines = Vec::new();
            let mut files = serde_json::Map::new();
            for s in servers {
                s.wait_closed(std::time::Duration::from_secs(5));
                lines.extend(s.log_lines());
                for (k, v) in s.files() {
                    files.insert(k, json!(base64::engine::general_purpose::STANDARD.encode(v)));
                }
            }
            sftp.wait_closed(std::time::Duration::from_secs(5));
            let sftp_files: serde_json::Map<String, Value> =
                sftp.files().into_iter().map(|(k, v)| (k, json!(base64::engine::general_purpose::STANDARD.encode(v)))).collect();
            smtp.wait_closed(std::time::Duration::from_secs(5));
            let mails: Vec<Value> = smtp.messages().iter().map(|m| json!(String::from_utf8_lossy(m))).collect();
            json!({"ftp": lines, "files": files, "sftp": sftp.log_lines(), "sftp_files": sftp_files, "smtp": smtp.log_lines(), "mails": mails})
                .to_string()
        } else {
            "{}".to_owned()
        };
        let mut stream = stream;
        let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len());
    }
}
