//! A mock FTP / FTPS server on 127.0.0.1 with a virtual file system: it
//! answers as vsftpd does, records every command and reply in order, and
//! keeps what was stored. Explicit (`AUTH TLS`) and implicit TLS, passive
//! (`PASV`, `EPSV`) and active (`PORT`, `EPRT`) data connections.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use native_tls::{TlsAcceptor, TlsStream};

/// What a session did, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FtpEvent {
    /// A control connection was accepted.
    Connected,
    /// A command line as the client sent it.
    Command(String),
    /// A reply line as the server sent it.
    Reply(String),
    /// TLS started on the control connection.
    TlsControl,
    /// A data transfer: the command, the bytes, whether it was over TLS.
    Data { command: String, bytes: usize, tls: bool },
    /// The control connection closed.
    Closed,
}

impl FtpEvent {
    /// One line, with what changes between runs (ports) normalised.
    pub fn to_line(&self) -> String {
        fn norm(s: &str) -> String {
            normalize_ports(s)
        }
        match self {
            FtpEvent::Connected => "connect".into(),
            FtpEvent::Command(c) => format!("> {}", norm(c)),
            FtpEvent::Reply(r) => format!("< {}", norm(r)),
            FtpEvent::TlsControl => "tls".into(),
            FtpEvent::Data { command, bytes, tls } => format!("data {} {bytes} bytes{}", norm(command), if *tls { " tls" } else { "" }),
            FtpEvent::Closed => "close".into(),
        }
    }
}

/// Replaces the port numbers of PASV/EPSV replies and PORT/EPRT commands.
fn normalize_ports(s: &str) -> String {
    if let Some(i) = s.find("(|||")
        && let Some(j) = s[i + 4..].find('|')
    {
        return format!("{}(|||PORT{}", &s[..i], &s[i + 4 + j..]);
    }
    if s.starts_with("227 ")
        && let (Some(i), Some(j)) = (s.find('('), s.find(')'))
    {
        return format!("{}(ADDR){}", &s[..i], &s[j + 1..]);
    }
    let upper = s.to_ascii_uppercase();
    if upper.starts_with("PORT ") {
        return "PORT ADDR".into();
    }
    if upper.starts_with("EPRT ") {
        return "EPRT ADDR".into();
    }
    s.to_owned()
}

/// The server's settings.
#[derive(Clone)]
pub struct FtpMockConfig {
    pub user: String,
    pub password: String,
    /// Directories that exist (`/` always does).
    pub dirs: Vec<String>,
    /// TLS identity for `AUTH TLS` and implicit TLS.
    pub tls: Option<TlsAcceptor>,
    /// Implicit TLS: the control connection is TLS from the start.
    pub implicit_tls: bool,
    /// Replies that replace the normal one the first time a command
    /// starting with the key is received (`("STOR", "451 Disk full")`).
    pub fail_once: Vec<(String, String)>,
}

impl Default for FtpMockConfig {
    fn default() -> Self {
        Self { user: "user".into(), password: "pass".into(), dirs: Vec::new(), tls: None, implicit_tls: false, fail_once: Vec::new() }
    }
}

#[derive(Default)]
struct State {
    events: Vec<FtpEvent>,
    dirs: BTreeSet<String>,
    files: BTreeMap<String, Vec<u8>>,
    fail_once: Vec<(String, String)>,
}

/// A running mock server (it stops with the process).
pub struct FtpMockServer {
    pub addr: SocketAddr,
    state: Arc<Mutex<State>>,
    config: Arc<Mutex<FtpMockConfig>>,
}

/// The TLS identity in `tests/fixtures/mock_tls.p12` (self-signed for
/// 127.0.0.1, made for these tests only). `None` where the platform's TLS
/// can't load it.
pub fn test_tls_acceptor() -> Option<TlsAcceptor> {
    let p12 = include_bytes!("../../tests/fixtures/mock_tls.p12");
    let identity = native_tls::Identity::from_pkcs12(p12, "sharexrs").ok()?;
    TlsAcceptor::new(identity).ok()
}

fn normalize_path(cwd: &str, path: &str) -> String {
    let full = if path.starts_with('/') { path.to_owned() } else { format!("{}/{path}", cwd.trim_end_matches('/')) };
    let mut parts: Vec<&str> = Vec::new();
    for p in full.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    format!("/{}", parts.join("/"))
}

fn parent(path: &str) -> String {
    match path.rfind('/') {
        Some(0) | None => "/".into(),
        Some(i) => path[..i].to_owned(),
    }
}

enum Stream {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
    /// While the control connection is being upgraded to TLS.
    Empty,
}

fn closed() -> std::io::Error {
    std::io::Error::from(std::io::ErrorKind::NotConnected)
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
            Stream::Empty => Err(closed()),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
            Stream::Empty => Err(closed()),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
            Stream::Empty => Err(closed()),
        }
    }
}

enum DataSetup {
    None,
    Passive(TcpListener),
    Active(SocketAddr),
}

struct Session {
    state: Arc<Mutex<State>>,
    config: FtpMockConfig,
    cwd: String,
    logged_in: bool,
    user: Option<String>,
    protect_data: bool,
    data: DataSetup,
}

impl FtpMockServer {
    /// Starts a server on a free port.
    pub fn start(config: FtpMockConfig) -> std::io::Result<Self> {
        Self::start_on(0, config)
    }

    /// Starts a server on `port` (0: any).
    pub fn start_on(port: u16, config: FtpMockConfig) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let addr = listener.local_addr()?;
        let server = Self { addr, state: Arc::new(Mutex::new(State::default())), config: Arc::new(Mutex::new(config)) };
        server.reset(None);
        let state = server.state.clone();
        let config = server.config.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let state = state.clone();
                let config = config.lock().expect("ftp config").clone();
                thread::spawn(move || {
                    let _ = Session::run(stream, state, config);
                });
            }
        });
        Ok(server)
    }

    /// Clears the log and files and sets the directories (and, if given,
    /// the settings) for the next case.
    pub fn reset(&self, config: Option<FtpMockConfig>) {
        if let Some(c) = config {
            *self.config.lock().expect("ftp config") = c;
        }
        let c = self.config.lock().expect("ftp config").clone();
        let mut s = self.state.lock().expect("ftp state");
        s.events.clear();
        s.files.clear();
        s.dirs = BTreeSet::from(["/".to_owned()]);
        for d in &c.dirs {
            let mut p = String::new();
            for part in d.split('/').filter(|p| !p.is_empty()) {
                p.push('/');
                p.push_str(part);
                s.dirs.insert(p.clone());
            }
        }
        s.fail_once = c.fail_once.clone();
    }

    /// Everything that happened since the last reset.
    pub fn events(&self) -> Vec<FtpEvent> {
        self.state.lock().expect("ftp state").events.clone()
    }

    /// The log as lines ([`FtpEvent::to_line`]).
    pub fn log_lines(&self) -> Vec<String> {
        self.events().iter().map(FtpEvent::to_line).collect()
    }

    /// The files stored, by absolute path.
    pub fn files(&self) -> BTreeMap<String, Vec<u8>> {
        self.state.lock().expect("ftp state").files.clone()
    }

    /// The directories that exist.
    pub fn dirs(&self) -> Vec<String> {
        self.state.lock().expect("ftp state").dirs.iter().cloned().collect()
    }

    /// Waits until the client has closed its control connection (the log
    /// is complete).
    pub fn wait_closed(&self, timeout: Duration) {
        let end = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < end {
            {
                let s = self.state.lock().expect("ftp state");
                let opened = s.events.iter().filter(|e| **e == FtpEvent::Connected).count();
                let closed = s.events.iter().filter(|e| **e == FtpEvent::Closed).count();
                if opened == closed {
                    return;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Session {
    fn log(&self, e: FtpEvent) {
        self.state.lock().expect("ftp state").events.push(e);
    }

    fn run(stream: TcpStream, state: Arc<Mutex<State>>, config: FtpMockConfig) -> std::io::Result<()> {
        let mut session =
            Session { state, config, cwd: "/".into(), logged_in: false, user: None, protect_data: false, data: DataSetup::None };
        session.log(FtpEvent::Connected);
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        let mut control = if session.config.implicit_tls {
            let acceptor = session.config.tls.clone().ok_or_else(|| std::io::Error::other("no TLS identity"))?;
            let tls = acceptor.accept(stream).map_err(std::io::Error::other)?;
            session.log(FtpEvent::TlsControl);
            Stream::Tls(Box::new(tls))
        } else {
            Stream::Plain(stream)
        };
        let r = session.serve(&mut control);
        session.log(FtpEvent::Closed);
        r
    }

    fn reply(&self, control: &mut Stream, line: &str) -> std::io::Result<()> {
        for l in line.split("\r\n") {
            self.log(FtpEvent::Reply(l.to_owned()));
        }
        control.write_all(format!("{line}\r\n").as_bytes())?;
        control.flush()
    }

    fn read_line(control: &mut Stream) -> std::io::Result<Option<String>> {
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match control.read(&mut byte) {
                Ok(0) => return Ok(if line.is_empty() { None } else { Some(String::from_utf8_lossy(&line).into_owned()) }),
                Ok(_) => {
                    if byte[0] == b'\n' {
                        if line.last() == Some(&b'\r') {
                            line.pop();
                        }
                        return Ok(Some(String::from_utf8_lossy(&line).into_owned()));
                    }
                    line.push(byte[0]);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn serve(&mut self, control: &mut Stream) -> std::io::Result<()> {
        self.reply(control, "220 (ShareXrs mock FTP)")?;
        loop {
            let Some(line) = Self::read_line(control)? else { return Ok(()) };
            self.log(FtpEvent::Command(line.clone()));
            let (cmd, arg) = match line.split_once(' ') {
                Some((c, a)) => (c.to_ascii_uppercase(), a.to_owned()),
                None => (line.to_ascii_uppercase(), String::new()),
            };
            let failure = {
                let mut s = self.state.lock().expect("ftp state");
                let i = s.fail_once.iter().position(|(k, _)| line.to_ascii_uppercase().starts_with(&k.to_ascii_uppercase()));
                i.map(|i| s.fail_once.remove(i).1)
            };
            if let Some(f) = failure {
                self.reply(control, &f)?;
                continue;
            }
            match cmd.as_str() {
                "AUTH" => {
                    let Some(acceptor) = self.config.tls.clone() else {
                        self.reply(control, "530 Please login with USER and PASS.")?;
                        continue;
                    };
                    self.reply(control, "234 Proceed with negotiation.")?;
                    let Stream::Plain(tcp) = std::mem::replace(control, Stream::Empty) else {
                        return Ok(());
                    };
                    let tls = acceptor.accept(tcp).map_err(std::io::Error::other)?;
                    self.log(FtpEvent::TlsControl);
                    *control = Stream::Tls(Box::new(tls));
                }
                "USER" => {
                    self.user = Some(arg);
                    self.reply(control, "331 Please specify the password.")?;
                }
                "PASS" => {
                    if self.user.as_deref() == Some(self.config.user.as_str()) && arg == self.config.password {
                        self.logged_in = true;
                        self.reply(control, "230 Login successful.")?;
                    } else {
                        self.reply(control, "530 Login incorrect.")?;
                    }
                }
                "QUIT" => {
                    self.reply(control, "221 Goodbye.")?;
                    return Ok(());
                }
                "PBSZ" => self.reply(control, "200 PBSZ set to 0.")?,
                "PROT" => {
                    self.protect_data = arg.eq_ignore_ascii_case("P");
                    self.reply(control, if self.protect_data { "200 PROT now Private." } else { "200 PROT now Clear." })?;
                }
                "FEAT" => {
                    let tls = if self.config.tls.is_some() { "\r\n AUTH TLS\r\n PBSZ\r\n PROT" } else { "" };
                    self.reply(control, &format!("211-Features:\r\n EPRT\r\n EPSV\r\n MDTM\r\n PASV{tls}\r\n REST STREAM\r\n SIZE\r\n TVFS\r\n UTF8\r\n211 End"))?;
                }
                "SYST" => self.reply(control, "215 UNIX Type: L8")?,
                "NOOP" => self.reply(control, "200 NOOP ok.")?,
                "OPTS" if arg.to_ascii_uppercase().starts_with("UTF8") => self.reply(control, "200 Always in UTF8 mode.")?,
                _ if !self.logged_in => self.reply(control, "530 Please login with USER and PASS.")?,
                "PWD" | "XPWD" => self.reply(control, &format!("257 \"{}\" is the current directory", self.cwd))?,
                "TYPE" => self.reply(control, if arg.eq_ignore_ascii_case("A") { "200 Switching to ASCII mode." } else { "200 Switching to Binary mode." })?,
                "CWD" | "XCWD" => {
                    let p = normalize_path(&self.cwd, &arg);
                    if self.state.lock().expect("ftp state").dirs.contains(&p) {
                        self.cwd = p;
                        self.reply(control, "250 Directory successfully changed.")?;
                    } else {
                        self.reply(control, "550 Failed to change directory.")?;
                    }
                }
                "CDUP" => {
                    self.cwd = parent(&self.cwd);
                    self.reply(control, "250 Directory successfully changed.")?;
                }
                "MKD" | "XMKD" => {
                    let p = normalize_path(&self.cwd, &arg);
                    let mut s = self.state.lock().expect("ftp state");
                    if s.dirs.contains(&parent(&p)) && !s.dirs.contains(&p) && !s.files.contains_key(&p) {
                        s.dirs.insert(p.clone());
                        drop(s);
                        self.reply(control, &format!("257 \"{p}\" created"))?;
                    } else {
                        drop(s);
                        self.reply(control, "550 Create directory operation failed.")?;
                    }
                }
                "SIZE" => {
                    let p = normalize_path(&self.cwd, &arg);
                    let size = self.state.lock().expect("ftp state").files.get(&p).map(Vec::len);
                    match size {
                        Some(n) => self.reply(control, &format!("213 {n}"))?,
                        None => self.reply(control, "550 Could not get file size.")?,
                    }
                }
                "MDTM" => {
                    let p = normalize_path(&self.cwd, &arg);
                    let exists = self.state.lock().expect("ftp state").files.contains_key(&p);
                    self.reply(control, if exists { "213 20260101000000" } else { "550 Could not get file modification time." })?;
                }
                "DELE" => {
                    let p = normalize_path(&self.cwd, &arg);
                    let removed = self.state.lock().expect("ftp state").files.remove(&p).is_some();
                    self.reply(control, if removed { "250 Delete operation successful." } else { "550 Delete operation failed." })?;
                }
                "PASV" => {
                    let listener = TcpListener::bind(("127.0.0.1", 0))?;
                    let port = listener.local_addr()?.port();
                    self.data = DataSetup::Passive(listener);
                    self.reply(control, &format!("227 Entering Passive Mode (127,0,0,1,{},{}).", port >> 8, port & 0xff))?;
                }
                "EPSV" => {
                    let listener = TcpListener::bind(("127.0.0.1", 0))?;
                    let port = listener.local_addr()?.port();
                    self.data = DataSetup::Passive(listener);
                    self.reply(control, &format!("229 Entering Extended Passive Mode (|||{port}|)"))?;
                }
                "PORT" => {
                    let n: Vec<u16> = arg.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                    if n.len() == 6 {
                        let addr = SocketAddr::from(([n[0] as u8, n[1] as u8, n[2] as u8, n[3] as u8], n[4] * 256 + n[5]));
                        self.data = DataSetup::Active(addr);
                        self.reply(control, "200 PORT command successful. Consider using PASV.")?;
                    } else {
                        self.reply(control, "500 Illegal PORT command.")?;
                    }
                }
                "EPRT" => {
                    let d = arg.chars().next().unwrap_or('|');
                    let parts: Vec<&str> = arg.split(d).collect();
                    match (parts.get(2), parts.get(3).and_then(|p| p.parse::<u16>().ok())) {
                        (Some(ip), Some(port)) if ip.parse::<std::net::IpAddr>().is_ok() => {
                            self.data = DataSetup::Active(SocketAddr::new(ip.parse().expect("checked"), port));
                            self.reply(control, "200 EPRT command successful. Consider using EPSV.")?;
                        }
                        _ => self.reply(control, "500 Bad EPRT command.")?,
                    }
                }
                "STOR" | "APPE" => {
                    let p = normalize_path(&self.cwd, &arg);
                    if !self.state.lock().expect("ftp state").dirs.contains(&parent(&p)) {
                        self.data = DataSetup::None;
                        self.reply(control, "553 Could not create file.")?;
                        continue;
                    }
                    let setup = std::mem::replace(&mut self.data, DataSetup::None);
                    if matches!(setup, DataSetup::None) {
                        self.reply(control, "425 Use PORT or PASV first.")?;
                        continue;
                    }
                    self.reply(control, "150 Ok to send data.")?;
                    let (bytes, tls) = self.receive(setup)?;
                    let n = bytes.len();
                    {
                        let mut s = self.state.lock().expect("ftp state");
                        if cmd == "APPE" {
                            s.files.entry(p).or_default().extend(bytes);
                        } else {
                            s.files.insert(p, bytes);
                        }
                    }
                    self.log(FtpEvent::Data { command: line.clone(), bytes: n, tls });
                    self.reply(control, "226 Transfer complete.")?;
                }
                "LIST" | "NLST" | "MLSD" | "RETR" => {
                    self.data = DataSetup::None;
                    self.reply(control, "550 Permission denied.")?;
                }
                _ => self.reply(control, "502 Command not implemented.")?,
            }
        }
    }

    /// Accepts or makes the data connection and reads it to the end.
    fn receive(&self, setup: DataSetup) -> std::io::Result<(Vec<u8>, bool)> {
        let tcp = match setup {
            DataSetup::Passive(listener) => {
                listener.set_nonblocking(false)?;
                listener.accept()?.0
            }
            DataSetup::Active(addr) => TcpStream::connect_timeout(&addr, Duration::from_secs(10))?,
            DataSetup::None => unreachable!(),
        };
        tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
        let mut data = Vec::new();
        if self.protect_data {
            let acceptor = self.config.tls.clone().ok_or_else(|| std::io::Error::other("no TLS identity"))?;
            let mut tls = acceptor.accept(tcp).map_err(std::io::Error::other)?;
            let _ = read_to_end_lenient(&mut tls, &mut data);
            let _ = tls.shutdown();
            Ok((data, true))
        } else {
            let mut tcp = tcp;
            let _ = read_to_end_lenient(&mut tcp, &mut data);
            let _ = tcp.shutdown(Shutdown::Both);
            Ok((data, false))
        }
    }
}

/// Reads until the peer closes; a TLS close without `close_notify` or a
/// reset after the data still counts as the end.
fn read_to_end_lenient(r: &mut dyn Read, out: &mut Vec<u8>) -> std::io::Result<()> {
    let mut buf = [0u8; 16384];
    loop {
        match r.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted | std::io::ErrorKind::UnexpectedEof) => {
                return Ok(());
            }
            Err(e) => return Err(e),
        }
    }
}
