//! A mock SMTP server on 127.0.0.1 (STARTTLS with the test certificate,
//! `AUTH LOGIN`/`PLAIN`): it records every command and reply and keeps the
//! messages it accepts.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use native_tls::{TlsAcceptor, TlsStream};

/// The server's settings.
#[derive(Clone)]
pub struct SmtpMockConfig {
    pub user: String,
    pub password: String,
    pub tls: Option<TlsAcceptor>,
    /// Replies that replace the normal one the first time a command starting
    /// with the key arrives (`("RCPT", "550 No such user")`).
    pub fail_once: Vec<(String, String)>,
}

impl Default for SmtpMockConfig {
    fn default() -> Self {
        Self { user: "user".into(), password: "pass".into(), tls: None, fail_once: Vec::new() }
    }
}

#[derive(Default)]
struct State {
    log: Vec<String>,
    messages: Vec<Vec<u8>>,
    fail_once: Vec<(String, String)>,
    open: usize,
}

/// A running mock SMTP server.
pub struct SmtpMockServer {
    pub addr: SocketAddr,
    state: Arc<Mutex<State>>,
    config: Arc<Mutex<SmtpMockConfig>>,
}

enum Conn {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
    Empty,
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.read(buf),
            Conn::Tls(s) => s.read(buf),
            Conn::Empty => Err(std::io::ErrorKind::NotConnected.into()),
        }
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.write(buf),
            Conn::Tls(s) => s.write(buf),
            Conn::Empty => Err(std::io::ErrorKind::NotConnected.into()),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Conn::Plain(s) => s.flush(),
            Conn::Tls(s) => s.flush(),
            Conn::Empty => Ok(()),
        }
    }
}

fn b64d(s: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s.trim()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

impl SmtpMockServer {
    pub fn start(config: SmtpMockConfig) -> std::io::Result<Self> {
        Self::start_on(0, config)
    }

    pub fn start_on(port: u16, config: SmtpMockConfig) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let addr = listener.local_addr()?;
        let server = Self { addr, state: Arc::new(Mutex::new(State::default())), config: Arc::new(Mutex::new(config)) };
        server.reset(None);
        let (state, config) = (server.state.clone(), server.config.clone());
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let state = state.clone();
                let config = config.lock().expect("smtp config").clone();
                state.lock().expect("smtp state").open += 1;
                thread::spawn(move || {
                    let r = Session { state: state.clone(), config }.run(stream);
                    let mut s = state.lock().expect("smtp state");
                    if let Err(e) = r {
                        s.log.push(format!("error {e}"));
                    }
                    s.log.push("close".into());
                    s.open -= 1;
                });
            }
        });
        Ok(server)
    }

    pub fn reset(&self, config: Option<SmtpMockConfig>) {
        if let Some(c) = config {
            *self.config.lock().expect("smtp config") = c;
        }
        let c = self.config.lock().expect("smtp config").clone();
        let mut s = self.state.lock().expect("smtp state");
        s.log.clear();
        s.messages.clear();
        s.fail_once = c.fail_once;
    }

    pub fn log_lines(&self) -> Vec<String> {
        self.state.lock().expect("smtp state").log.clone()
    }

    pub fn messages(&self) -> Vec<Vec<u8>> {
        self.state.lock().expect("smtp state").messages.clone()
    }

    pub fn wait_closed(&self, timeout: Duration) {
        let end = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < end {
            if self.state.lock().expect("smtp state").open == 0 {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

struct Session {
    state: Arc<Mutex<State>>,
    config: SmtpMockConfig,
}

impl Session {
    fn log(&self, l: impl Into<String>) {
        self.state.lock().expect("smtp state").log.push(l.into());
    }

    fn reply(&self, c: &mut Conn, line: &str) -> std::io::Result<()> {
        for l in line.split("\r\n") {
            self.log(format!("< {l}"));
        }
        c.write_all(format!("{line}\r\n").as_bytes())?;
        c.flush()
    }

    fn read_line(c: &mut Conn) -> std::io::Result<Option<String>> {
        let mut line = Vec::new();
        let mut b = [0u8; 1];
        loop {
            if c.read(&mut b)? == 0 {
                return Ok(if line.is_empty() { None } else { Some(String::from_utf8_lossy(&line).into_owned()) });
            }
            if b[0] == b'\n' {
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return Ok(Some(String::from_utf8_lossy(&line).into_owned()));
            }
            line.push(b[0]);
        }
    }

    fn run(&self, stream: TcpStream) -> std::io::Result<()> {
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        self.log("connect");
        let mut c = Conn::Plain(stream);
        self.reply(&mut c, "220 mock.example.com ESMTP ShareXrs mock")?;
        let mut tls = false;
        let mut authed = false;
        let mut pending_auth: Option<(String, Option<String>)> = None;
        loop {
            let Some(line) = Self::read_line(&mut c)? else { return Ok(()) };
            if let Some((mech, user)) = pending_auth.take() {
                // A continuation of AUTH LOGIN.
                self.log(format!("> [{}]", if user.is_none() { "username" } else { "password" }));
                match (mech.as_str(), user) {
                    ("LOGIN", None) => {
                        pending_auth = Some(("LOGIN".into(), Some(b64d(&line))));
                        self.reply(&mut c, "334 UGFzc3dvcmQ6")?;
                    }
                    ("LOGIN", Some(user)) => {
                        authed = user == self.config.user && b64d(&line) == self.config.password;
                        self.log(format!("auth {} {}", user, if authed { "ok" } else { "fail" }));
                        self.reply(&mut c, if authed { "235 2.7.0 Authentication successful" } else { "535 5.7.8 Authentication credentials invalid" })?;
                    }
                    _ => {}
                }
                continue;
            }
            let upper = line.to_ascii_uppercase();
            // The client's host name differs between machines.
            self.log(if upper.starts_with("AUTH LOGIN ") || upper.starts_with("AUTH PLAIN ") {
                format!("> {} [credentials]", &line[..10])
            } else if upper.starts_with("EHLO ") || upper.starts_with("HELO ") {
                format!("> {} HOST", &line[..4])
            } else {
                format!("> {line}")
            });
            let failure = {
                let mut s = self.state.lock().expect("smtp state");
                let i = s.fail_once.iter().position(|(k, _)| upper.starts_with(&k.to_ascii_uppercase()));
                i.map(|i| s.fail_once.remove(i).1)
            };
            if let Some(f) = failure {
                self.reply(&mut c, &f)?;
                continue;
            }
            let (cmd, arg) = match line.split_once(' ') {
                Some((a, b)) => (a.to_ascii_uppercase(), b.to_owned()),
                None => (upper.clone(), String::new()),
            };
            match cmd.as_str() {
                "EHLO" => {
                    let mut lines = vec!["250-mock.example.com".to_owned(), "250-SIZE 35882577".into(), "250-8BITMIME".into()];
                    if !tls && self.config.tls.is_some() {
                        lines.push("250-STARTTLS".into());
                    }
                    if tls || self.config.tls.is_none() {
                        lines.push("250-AUTH LOGIN PLAIN".into());
                    }
                    lines.push("250 SMTPUTF8".into());
                    self.reply(&mut c, &lines.join("\r\n"))?;
                }
                "HELO" => self.reply(&mut c, "250 mock.example.com")?,
                "STARTTLS" => {
                    let Some(acceptor) = self.config.tls.clone() else {
                        self.reply(&mut c, "502 5.5.1 Unrecognized command")?;
                        continue;
                    };
                    self.reply(&mut c, "220 2.0.0 Ready to start TLS")?;
                    let Conn::Plain(tcp) = std::mem::replace(&mut c, Conn::Empty) else { return Ok(()) };
                    c = Conn::Tls(Box::new(acceptor.accept(tcp).map_err(std::io::Error::other)?));
                    tls = true;
                    self.log("tls");
                }
                "AUTH" => {
                    let mut parts = arg.splitn(2, ' ');
                    let mech = parts.next().unwrap_or("").to_ascii_uppercase();
                    let initial = parts.next();
                    match (mech.as_str(), initial) {
                        ("LOGIN", None) => {
                            pending_auth = Some(("LOGIN".into(), None));
                            self.reply(&mut c, "334 VXNlcm5hbWU6")?;
                        }
                        ("LOGIN", Some(u)) => {
                            pending_auth = Some(("LOGIN".into(), Some(b64d(u))));
                            self.reply(&mut c, "334 UGFzc3dvcmQ6")?;
                        }
                        ("PLAIN", Some(p)) => {
                            let decoded = b64d(p);
                            let mut f = decoded.split('\0').skip(1);
                            let (u, pw) = (f.next().unwrap_or(""), f.next().unwrap_or(""));
                            authed = u == self.config.user && pw == self.config.password;
                            self.log(format!("auth {u} {}", if authed { "ok" } else { "fail" }));
                            self.reply(&mut c, if authed { "235 2.7.0 Authentication successful" } else { "535 5.7.8 Authentication credentials invalid" })?;
                        }
                        _ => self.reply(&mut c, "504 5.5.4 Unrecognized authentication type")?,
                    }
                }
                "MAIL" => {
                    if !authed {
                        self.reply(&mut c, "530 5.7.0 Authentication required")?;
                    } else {
                        self.reply(&mut c, "250 2.1.0 OK")?;
                    }
                }
                "RCPT" => self.reply(&mut c, "250 2.1.5 OK")?,
                "DATA" => {
                    self.reply(&mut c, "354 Go ahead")?;
                    let mut data = Vec::new();
                    loop {
                        let Some(l) = Self::read_line(&mut c)? else { return Ok(()) };
                        if l == "." {
                            break;
                        }
                        data.extend_from_slice(l.as_bytes());
                        data.extend_from_slice(b"\r\n");
                    }
                    self.log(format!("data {} bytes", data.len()));
                    self.state.lock().expect("smtp state").messages.push(data);
                    self.reply(&mut c, "250 2.0.0 OK queued")?;
                }
                "RSET" | "NOOP" => self.reply(&mut c, "250 2.0.0 OK")?,
                "QUIT" => {
                    self.reply(&mut c, "221 2.0.0 Bye")?;
                    return Ok(());
                }
                _ => self.reply(&mut c, "502 5.5.2 Unrecognized command")?,
            }
        }
    }
}
