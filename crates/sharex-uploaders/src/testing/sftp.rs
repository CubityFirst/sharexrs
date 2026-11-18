//! A mock SFTP server on 127.0.0.1: SSH-2 ([`crate::ssh`] in the server
//! role, an ed25519 host key), password and public key logins, and SFTP
//! version 3 over a virtual file system that answers as OpenSSH's
//! `sftp-server` does. Every login attempt and SFTP request and reply is
//! recorded.

use std::collections::{BTreeMap, BTreeSet};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::sftp::*;
use crate::ssh::*;

/// The server's settings.
#[derive(Clone, Debug)]
pub struct SftpMockConfig {
    pub user: String,
    pub password: String,
    /// An OpenSSH public key line allowed to log in.
    pub authorized_key: Option<String>,
    /// The home directory (`REALPATH .`).
    pub home: String,
    /// Directories that exist besides the home's parents.
    pub dirs: Vec<String>,
    /// Status replies that replace the normal reply the first time a
    /// request whose log line starts with the key arrives
    /// (`("MKDIR /home/user/a", (3, "Permission denied"))`).
    pub fail_once: Vec<(String, (u32, String))>,
    /// Files there at the start, by absolute path.
    pub files: Vec<(String, Vec<u8>)>,
    /// The key exchanges, ciphers and MACs offered (all when empty); the
    /// log then names what was chosen (`kex <kex> <cipher> <mac>`).
    pub kex: Vec<String>,
    pub ciphers: Vec<String>,
    pub macs: Vec<String>,
}

impl Default for SftpMockConfig {
    fn default() -> Self {
        Self {
            user: "user".into(),
            password: "pass".into(),
            authorized_key: None,
            home: "/home/user".into(),
            dirs: Vec::new(),
            fail_once: Vec::new(),
            files: Vec::new(),
            kex: Vec::new(),
            ciphers: Vec::new(),
            macs: Vec::new(),
        }
    }
}

impl SftpMockConfig {
    /// The settings from a harness case's `servers` object: `user`,
    /// `password`, `authorized_key`, `home`, `dirs`, `sftp_fail_once`
    /// (`[[prefix, code, message]]`), `sftp_files` (`{path: base64}`),
    /// `sftp_kex`, `sftp_ciphers`, `sftp_macs` (the algorithms offered).
    pub fn from_spec(v: &serde_json::Value) -> Self {
        use base64::Engine;
        Self {
            user: v["user"].as_str().unwrap_or("user").to_owned(),
            password: v["password"].as_str().unwrap_or("pass").to_owned(),
            authorized_key: v["authorized_key"].as_str().map(str::to_owned),
            home: v["home"].as_str().unwrap_or("/home/user").to_owned(),
            dirs: v["dirs"].as_array().map(|a| a.iter().filter_map(|d| d.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
            fail_once: v["sftp_fail_once"]
                .as_array()
                .map(|a| a.iter().filter_map(|p| Some((p[0].as_str()?.to_owned(), (p[1].as_u64()? as u32, p[2].as_str()?.to_owned())))).collect())
                .unwrap_or_default(),
            files: v["sftp_files"]
                .as_object()
                .map(|o| {
                    o.iter().map(|(k, b)| (k.clone(), base64::engine::general_purpose::STANDARD.decode(b.as_str().unwrap_or("")).unwrap_or_default())).collect()
                })
                .unwrap_or_default(),
            kex: list(&v["sftp_kex"]),
            ciphers: list(&v["sftp_ciphers"]),
            macs: list(&v["sftp_macs"]),
        }
    }
}

fn list(v: &serde_json::Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

#[derive(Default)]
struct State {
    log: Vec<String>,
    dirs: BTreeSet<String>,
    files: BTreeMap<String, Vec<u8>>,
    fail_once: Vec<(String, (u32, String))>,
    sessions_open: usize,
}

/// A running mock SFTP server.
pub struct SftpMockServer {
    pub addr: SocketAddr,
    state: Arc<Mutex<State>>,
    config: Arc<Mutex<SftpMockConfig>>,
}

/// The mock's host key (fixed, test only).
pub fn mock_host_key() -> HostKey {
    ssh_key::PrivateKey::from(ssh_key::private::Ed25519Keypair::from_seed(&[7u8; 32]))
}

fn normalize(cwd: &str, path: &str) -> String {
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

impl SftpMockServer {
    pub fn start(config: SftpMockConfig) -> std::io::Result<Self> {
        Self::start_on(0, config)
    }

    pub fn start_on(port: u16, config: SftpMockConfig) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let addr = listener.local_addr()?;
        let server = Self { addr, state: Arc::new(Mutex::new(State::default())), config: Arc::new(Mutex::new(config)) };
        server.reset(None);
        let state = server.state.clone();
        let config = server.config.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let state = state.clone();
                let config = config.lock().expect("sftp config").clone();
                thread::spawn(move || {
                    state.lock().expect("sftp state").sessions_open += 1;
                    let r = Session { state: state.clone(), config }.run(stream);
                    let mut s = state.lock().expect("sftp state");
                    if let Err(e) = r {
                        s.log.push(format!("error {e}"));
                    }
                    s.log.push("close".into());
                    s.sessions_open -= 1;
                });
            }
        });
        Ok(server)
    }

    /// Clears the log and files and sets up the next case.
    pub fn reset(&self, config: Option<SftpMockConfig>) {
        if let Some(c) = config {
            *self.config.lock().expect("sftp config") = c;
        }
        let c = self.config.lock().expect("sftp config").clone();
        let mut s = self.state.lock().expect("sftp state");
        s.log.clear();
        s.files.clear();
        s.dirs = BTreeSet::from(["/".to_owned()]);
        for d in std::iter::once(&c.home).chain(c.dirs.iter()) {
            let mut p = String::new();
            for part in d.split('/').filter(|p| !p.is_empty()) {
                p.push('/');
                p.push_str(part);
                s.dirs.insert(p.clone());
            }
        }
        s.fail_once = c.fail_once.clone();
        for (path, bytes) in &c.files {
            let path = normalize("/", path);
            let mut p = String::new();
            let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
            for part in &parts[..parts.len().saturating_sub(1)] {
                p.push('/');
                p.push_str(part);
                s.dirs.insert(p.clone());
            }
            s.files.insert(path, bytes.clone());
        }
    }

    pub fn log_lines(&self) -> Vec<String> {
        self.state.lock().expect("sftp state").log.clone()
    }

    pub fn files(&self) -> BTreeMap<String, Vec<u8>> {
        self.state.lock().expect("sftp state").files.clone()
    }

    /// Waits until every session has ended.
    pub fn wait_closed(&self, timeout: Duration) {
        let end = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < end {
            if self.state.lock().expect("sftp state").sessions_open == 0 {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

struct Session {
    state: Arc<Mutex<State>>,
    config: SftpMockConfig,
}

impl Session {
    fn log(&self, line: impl Into<String>) {
        self.state.lock().expect("sftp state").log.push(line.into());
    }

    fn run(&self, stream: TcpStream) -> SshResult<()> {
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        self.log("connect");
        let mut algorithms = Algorithms::server();
        let configured = !self.config.kex.is_empty() || !self.config.ciphers.is_empty() || !self.config.macs.is_empty();
        if !self.config.kex.is_empty() {
            algorithms.kex = self.config.kex.clone();
        }
        if !self.config.ciphers.is_empty() {
            algorithms.ciphers = self.config.ciphers.clone();
        }
        if !self.config.macs.is_empty() {
            algorithms.macs = self.config.macs.clone();
        }
        let mut t = Transport::accept_with(stream, "SSH-2.0-OpenSSH_9.6 ShareXrs-mock", &mock_host_key(), &algorithms)?;
        if configured {
            let n = &t.negotiated;
            self.log(format!("kex {} {} {}", n[0], n[2], n[3]));
        }
        // Authentication.
        let p = t.recv()?;
        if p.first() != Some(&MSG_SERVICE_REQUEST) {
            return Err(SshError("Expected a service request.".into()));
        }
        t.send(&Writer::new(MSG_SERVICE_ACCEPT).string("ssh-userauth").0)?;
        loop {
            let p = t.recv()?;
            if p.first() != Some(&MSG_USERAUTH_REQUEST) {
                return Err(SshError("Expected an authentication request.".into()));
            }
            let mut r = Reader::new(&p[1..]);
            let user = r.string()?;
            let service = r.string()?;
            let method = r.string()?;
            let ok = match method.as_str() {
                "password" => {
                    r.bool()?;
                    let password = r.string()?;
                    let ok = user == self.config.user && password == self.config.password;
                    self.log(format!("auth password {user} {}", if ok { "ok" } else { "fail" }));
                    ok
                }
                "publickey" => {
                    let signed = r.bool()?;
                    let algorithm = r.string()?;
                    let blob = r.bytes()?.to_vec();
                    let allowed = self.config.authorized_key.as_deref().and_then(|k| ssh_key::PublicKey::from_openssh(k).ok()).and_then(|k| k.to_bytes().ok());
                    let known = user == self.config.user && allowed.as_deref() == Some(&blob[..]);
                    if !signed {
                        self.log(format!("auth publickey-query {user} {algorithm} {}", if known { "ok" } else { "fail" }));
                        if known {
                            t.send(&Writer::new(MSG_USERAUTH_PK_OK).string(&algorithm).bytes(&blob).0)?;
                            continue;
                        }
                        false
                    } else {
                        let signature = r.bytes()?;
                        let data = Writer::default()
                            .bytes(&t.session_id)
                            .byte(MSG_USERAUTH_REQUEST)
                            .string(&user)
                            .string(&service)
                            .string("publickey")
                            .bool(true)
                            .string(&algorithm)
                            .bytes(&blob)
                            .0;
                        let verified = known && verify(&blob, signature, &data);
                        self.log(format!("auth publickey {user} {algorithm} {}", if verified { "ok" } else { "fail" }));
                        verified
                    }
                }
                m => {
                    self.log(format!("auth {m} {user}"));
                    false
                }
            };
            if ok {
                t.send(&[MSG_USERAUTH_SUCCESS])?;
                break;
            }
            t.send(&Writer::new(MSG_USERAUTH_FAILURE).string("publickey,password").bool(false).0)?;
        }
        // The session channel and the subsystem.
        let mut client_channel = 0;
        let mut inbox: Vec<u8> = Vec::new();
        let mut cwd_files = SftpFs { state: self.state.clone(), home: self.config.home.clone(), handles: BTreeMap::new(), next_handle: 0 };
        let mut consumed = 0u64;
        loop {
            let p = match t.recv() {
                Ok(p) => p,
                Err(_) => return Ok(()),
            };
            let mut r = Reader::new(&p[1..]);
            match p[0] {
                MSG_CHANNEL_OPEN => {
                    r.string()?;
                    client_channel = r.u32()?;
                    t.send(&Writer::new(MSG_CHANNEL_OPEN_CONFIRMATION).u32(client_channel).u32(0).u32(2 * 1024 * 1024).u32(32768).0)?;
                }
                MSG_CHANNEL_REQUEST => {
                    r.u32()?;
                    let kind = r.string()?;
                    let want = r.bool()?;
                    let ok = kind == "subsystem" && r.string()? == "sftp";
                    if want {
                        t.send(&Writer::new(if ok { MSG_CHANNEL_SUCCESS } else { MSG_CHANNEL_FAILURE }).u32(client_channel).0)?;
                    }
                }
                MSG_CHANNEL_DATA => {
                    r.u32()?;
                    let data = r.bytes()?;
                    consumed += data.len() as u64;
                    inbox.extend_from_slice(data);
                    while inbox.len() >= 4 {
                        let len = u32::from_be_bytes(inbox[..4].try_into().expect("4")) as usize;
                        if inbox.len() < 4 + len {
                            break;
                        }
                        let packet: Vec<u8> = inbox.drain(..4 + len).skip(4).collect();
                        if let Some(reply) = cwd_files.handle(&packet, self)? {
                            let mut out = (reply.len() as u32).to_be_bytes().to_vec();
                            out.extend_from_slice(&reply);
                            t.send(&Writer::new(MSG_CHANNEL_DATA).u32(client_channel).bytes(&out).0)?;
                        }
                    }
                    if consumed > 1024 * 1024 {
                        t.send(&Writer::new(MSG_CHANNEL_WINDOW_ADJUST).u32(client_channel).u32(consumed as u32).0)?;
                        consumed = 0;
                    }
                }
                MSG_CHANNEL_EOF => {}
                MSG_CHANNEL_CLOSE => {
                    let _ = t.send(&Writer::new(MSG_CHANNEL_CLOSE).u32(client_channel).0);
                }
                MSG_CHANNEL_WINDOW_ADJUST => {}
                _ => {}
            }
        }
    }
}

/// The log line of a request (after its id), as the handler writes it.
fn describe(kind: u8, body: &[u8]) -> String {
    let mut r = Reader::new(body);
    let mut path = || r.string().unwrap_or_default();
    match kind {
        FXP_REALPATH => format!("REALPATH {}", path()),
        FXP_STAT => format!("STAT {}", path()),
        FXP_LSTAT => format!("LSTAT {}", path()),
        FXP_MKDIR => format!("MKDIR {}", path()),
        FXP_OPEN => {
            let p = path();
            let flags = r.u32().unwrap_or(0);
            format!("OPEN {p} flags=0x{flags:x}")
        }
        FXP_WRITE => {
            let _ = path();
            let offset = r.u64().unwrap_or(0);
            let len = r.bytes().map(<[u8]>::len).unwrap_or(0);
            format!("WRITE offset={offset} len={len}")
        }
        FXP_CLOSE => "CLOSE".into(),
        FXP_OPENDIR => format!("OPENDIR {}", path()),
        FXP_READDIR => "READDIR".into(),
        FXP_READ => {
            let _ = path();
            let offset = r.u64().unwrap_or(0);
            let len = r.u32().unwrap_or(0);
            format!("READ offset={offset} len={len}")
        }
        FXP_REMOVE => format!("REMOVE {}", path()),
        FXP_RMDIR => format!("RMDIR {}", path()),
        FXP_RENAME => {
            let from = path();
            format!("RENAME {from} {}", path())
        }
        _ => format!("OTHER {kind}"),
    }
}

fn verify(blob: &[u8], signature: &[u8], data: &[u8]) -> bool {
    use signature::Verifier;
    let (Ok(key), Ok(sig)) = (ssh_key::PublicKey::from_bytes(blob), ssh_key::Signature::try_from(signature)) else { return false };
    key.key_data().verify(data, &sig).is_ok()
}

struct SftpFs {
    state: Arc<Mutex<State>>,
    home: String,
    handles: BTreeMap<String, (String, bool)>,
    next_handle: u32,
}

impl SftpFs {
    fn status(id: u32, code: u32, message: &str) -> Vec<u8> {
        Writer::new(FXP_STATUS).u32(id).u32(code).string(message).string("").0
    }

    fn dir_attrs() -> Attrs {
        Attrs { size: Some(4096), permissions: Some(0o040755), mtime: Some(1_767_225_600) }
    }

    fn file_attrs(size: usize) -> Attrs {
        Attrs { size: Some(size as u64), permissions: Some(0o100644), mtime: Some(1_767_225_600) }
    }

    fn stat_of(&self, path: &str) -> Option<Attrs> {
        let s = self.state.lock().expect("sftp state");
        if s.dirs.contains(path) {
            Some(Self::dir_attrs())
        } else {
            s.files.get(path).map(|f| Self::file_attrs(f.len()))
        }
    }

    /// A directory's entries as `sftp-server` reads them: `.`, `..`, then
    /// the children (sorted here).
    fn entries(&self, dir: &str) -> Vec<(String, Attrs)> {
        let s = self.state.lock().expect("sftp state");
        let prefix = if dir == "/" { "/".to_owned() } else { format!("{dir}/") };
        let child = |p: &str| p.strip_prefix(prefix.as_str()).filter(|r| !r.is_empty() && !r.contains('/')).map(str::to_owned);
        let mut children: Vec<(String, Attrs)> = s.dirs.iter().filter_map(|d| child(d).map(|n| (n, Self::dir_attrs()))).collect();
        children.extend(s.files.iter().filter_map(|(f, b)| child(f).map(|n| (n, Self::file_attrs(b.len())))));
        children.sort_by(|a, b| a.0.cmp(&b.0));
        let mut entries = vec![(".".to_owned(), Self::dir_attrs()), ("..".to_owned(), Self::dir_attrs())];
        entries.extend(children);
        entries
    }

    fn handle(&mut self, packet: &[u8], session: &Session) -> SshResult<Option<Vec<u8>>> {
        let mut r = Reader::new(&packet[1..]);
        let kind = packet[0];
        if kind == FXP_INIT {
            let v = r.u32()?;
            session.log(format!("> INIT {v}"));
            session.log("< VERSION 3");
            return Ok(Some(Writer::new(FXP_VERSION).u32(3).0));
        }
        let id = r.u32()?;
        // A scripted failure replaces the reply, before anything is done.
        let described = describe(kind, &packet[5..]);
        let failure = {
            let mut s = self.state.lock().expect("sftp state");
            let i = s.fail_once.iter().position(|(k, _)| described.starts_with(k.as_str()));
            i.map(|i| s.fail_once.remove(i).1)
        };
        if let Some((code, message)) = failure {
            session.log(format!("> {described}"));
            session.log(format!("< STATUS {code} {message}"));
            return Ok(Some(Self::status(id, code, &message)));
        }
        let path_arg = |r: &mut Reader| -> SshResult<String> { r.string() };
        let (line, reply, reply_line): (String, Vec<u8>, String) = match kind {
            FXP_REALPATH => {
                let p = path_arg(&mut r)?;
                let full = normalize(&self.home, &p);
                let line = format!("REALPATH {p}");
                if self.stat_of(&full).is_some() {
                    let reply = Writer::new(FXP_NAME).u32(id).u32(1).string(&full).string(&full);
                    (line, Self::dir_attrs().write(reply).0, format!("NAME {full}"))
                } else {
                    (line, Self::status(id, FX_NO_SUCH_FILE, "No such file"), format!("STATUS {FX_NO_SUCH_FILE} No such file"))
                }
            }
            FXP_STAT | FXP_LSTAT => {
                let p = path_arg(&mut r)?;
                let full = normalize(&self.home, &p);
                let line = format!("{} {p}", if kind == FXP_STAT { "STAT" } else { "LSTAT" });
                match self.stat_of(&full) {
                    Some(a) => {
                        let what = if a.is_dir() { "dir" } else { "file" };
                        (line, a.write(Writer::new(FXP_ATTRS).u32(id)).0, format!("ATTRS {what}"))
                    }
                    None => (line, Self::status(id, FX_NO_SUCH_FILE, "No such file"), format!("STATUS {FX_NO_SUCH_FILE} No such file")),
                }
            }
            FXP_OPEN => {
                let p = path_arg(&mut r)?;
                let flags = r.u32()?;
                let full = normalize(&self.home, &p);
                let line = format!("OPEN {p} flags=0x{flags:x}");
                let mut s = self.state.lock().expect("sftp state");
                if !s.dirs.contains(&parent(&full)) {
                    (line, Self::status(id, FX_NO_SUCH_FILE, "No such file"), format!("STATUS {FX_NO_SUCH_FILE} No such file"))
                } else if s.dirs.contains(&full) {
                    (line, Self::status(id, FX_FAILURE, "Failure"), format!("STATUS {FX_FAILURE} Failure"))
                } else {
                    if flags & PF_CREAT != 0 && (flags & PF_TRUNC != 0 || !s.files.contains_key(&full)) {
                        s.files.insert(full.clone(), Vec::new());
                    }
                    drop(s);
                    self.next_handle += 1;
                    let h = format!("h{}", self.next_handle);
                    self.handles.insert(h.clone(), (full, false));
                    (line, Writer::new(FXP_HANDLE).u32(id).string(&h).0, "HANDLE".into())
                }
            }
            FXP_WRITE => {
                let h = r.string()?;
                let offset = r.u64()?;
                let data = r.bytes()?;
                let line = format!("WRITE offset={offset} len={}", data.len());
                if let Some((path, _)) = self.handles.get(&h) {
                    let mut s = self.state.lock().expect("sftp state");
                    let f = s.files.entry(path.clone()).or_default();
                    let end = offset as usize + data.len();
                    if f.len() < end {
                        f.resize(end, 0);
                    }
                    f[offset as usize..end].copy_from_slice(data);
                    (line, Self::status(id, FX_OK, "Success"), "STATUS 0".into())
                } else {
                    (line, Self::status(id, FX_FAILURE, "Bad handle"), "STATUS 4 Bad handle".into())
                }
            }
            FXP_CLOSE => {
                let h = r.string()?;
                self.handles.remove(&h);
                ("CLOSE".into(), Self::status(id, FX_OK, "Success"), "STATUS 0".into())
            }
            FXP_FSTAT => {
                let h = r.string()?;
                let size = self.handles.get(&h).and_then(|(p, _)| self.state.lock().expect("sftp state").files.get(p).map(Vec::len)).unwrap_or(0);
                ("FSTAT".into(), Self::file_attrs(size).write(Writer::new(FXP_ATTRS).u32(id)).0, "ATTRS file".into())
            }
            FXP_SETSTAT | FXP_FSETSTAT => {
                let what = if kind == FXP_SETSTAT { format!("SETSTAT {}", r.string()?) } else { "FSETSTAT".into() };
                (what, Self::status(id, FX_OK, "Success"), "STATUS 0".into())
            }
            FXP_MKDIR => {
                let p = path_arg(&mut r)?;
                let full = normalize(&self.home, &p);
                let line = format!("MKDIR {p}");
                let mut s = self.state.lock().expect("sftp state");
                if !s.dirs.contains(&parent(&full)) {
                    (line, Self::status(id, FX_NO_SUCH_FILE, "No such file"), format!("STATUS {FX_NO_SUCH_FILE} No such file"))
                } else if s.dirs.contains(&full) || s.files.contains_key(&full) {
                    (line, Self::status(id, FX_FAILURE, "Failure"), format!("STATUS {FX_FAILURE} Failure"))
                } else {
                    s.dirs.insert(full);
                    (line, Self::status(id, FX_OK, "Success"), "STATUS 0".into())
                }
            }
            FXP_OPENDIR => {
                let p = path_arg(&mut r)?;
                let full = normalize(&self.home, &p);
                let line = format!("OPENDIR {p}");
                let is_dir = self.state.lock().expect("sftp state").dirs.contains(&full);
                if is_dir {
                    self.next_handle += 1;
                    let h = format!("d{}", self.next_handle);
                    self.handles.insert(h.clone(), (full, false));
                    (line, Writer::new(FXP_HANDLE).u32(id).string(&h).0, "HANDLE".into())
                } else {
                    (line, Self::status(id, FX_NO_SUCH_FILE, "No such file"), format!("STATUS {FX_NO_SUCH_FILE} No such file"))
                }
            }
            FXP_READDIR => {
                let h = r.string()?;
                match self.handles.get(&h).cloned() {
                    Some((dir, false)) => {
                        self.handles.insert(h, (dir.clone(), true));
                        let entries = self.entries(&dir);
                        let mut w = Writer::new(FXP_NAME).u32(id).u32(entries.len() as u32);
                        for (name, attrs) in &entries {
                            let kind = if attrs.is_dir() { "d" } else { "-" };
                            let long = format!("{kind}rw-r--r--    1 user     user     {:>8} Jan  1 00:00 {name}", attrs.size.unwrap_or(0));
                            w = attrs.write(w.string(name).string(&long));
                        }
                        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
                        ("READDIR".into(), w.0, format!("NAME {}", names.join(" | ")))
                    }
                    Some((_, true)) => ("READDIR".into(), Self::status(id, FX_EOF, "End of file"), format!("STATUS {FX_EOF} End of file")),
                    None => ("READDIR".into(), Self::status(id, FX_FAILURE, "Bad handle"), "STATUS 4 Bad handle".into()),
                }
            }
            FXP_READ => {
                let h = r.string()?;
                let offset = r.u64()? as usize;
                let len = r.u32()? as usize;
                let line = format!("READ offset={offset} len={len}");
                let data = self.handles.get(&h).and_then(|(p, _)| self.state.lock().expect("sftp state").files.get(p).cloned());
                match data {
                    Some(d) if offset < d.len() => {
                        let end = (offset + len).min(d.len());
                        (line, Writer::new(FXP_DATA).u32(id).bytes(&d[offset..end]).0, format!("DATA {}", end - offset))
                    }
                    Some(_) => (line, Self::status(id, FX_EOF, "End of file"), format!("STATUS {FX_EOF} End of file")),
                    None => (line, Self::status(id, FX_FAILURE, "Bad handle"), "STATUS 4 Bad handle".into()),
                }
            }
            FXP_REMOVE => {
                let p = path_arg(&mut r)?;
                let full = normalize(&self.home, &p);
                let line = format!("REMOVE {p}");
                let mut s = self.state.lock().expect("sftp state");
                if s.files.remove(&full).is_some() {
                    (line, Self::status(id, FX_OK, "Success"), "STATUS 0".into())
                } else if s.dirs.contains(&full) {
                    (line, Self::status(id, FX_FAILURE, "Failure"), format!("STATUS {FX_FAILURE} Failure"))
                } else {
                    (line, Self::status(id, FX_NO_SUCH_FILE, "No such file"), format!("STATUS {FX_NO_SUCH_FILE} No such file"))
                }
            }
            FXP_RMDIR => {
                let p = path_arg(&mut r)?;
                let full = normalize(&self.home, &p);
                let line = format!("RMDIR {p}");
                let mut s = self.state.lock().expect("sftp state");
                let prefix = format!("{}/", full.trim_end_matches('/'));
                if !s.dirs.contains(&full) {
                    (line, Self::status(id, FX_NO_SUCH_FILE, "No such file"), format!("STATUS {FX_NO_SUCH_FILE} No such file"))
                } else if s.dirs.iter().any(|d| d.starts_with(&prefix)) || s.files.keys().any(|f| f.starts_with(&prefix)) {
                    (line, Self::status(id, FX_FAILURE, "Failure"), format!("STATUS {FX_FAILURE} Failure"))
                } else {
                    s.dirs.remove(&full);
                    (line, Self::status(id, FX_OK, "Success"), "STATUS 0".into())
                }
            }
            FXP_RENAME => {
                let from = path_arg(&mut r)?;
                let to = path_arg(&mut r)?;
                let (f, t) = (normalize(&self.home, &from), normalize(&self.home, &to));
                let line = format!("RENAME {from} {to}");
                let mut s = self.state.lock().expect("sftp state");
                let exists = s.files.contains_key(&f) || s.dirs.contains(&f);
                if !exists {
                    (line, Self::status(id, FX_NO_SUCH_FILE, "No such file"), format!("STATUS {FX_NO_SUCH_FILE} No such file"))
                } else if s.files.contains_key(&t) || s.dirs.contains(&t) || !s.dirs.contains(&parent(&t)) {
                    (line, Self::status(id, FX_FAILURE, "Failure"), format!("STATUS {FX_FAILURE} Failure"))
                } else {
                    if let Some(b) = s.files.remove(&f) {
                        s.files.insert(t, b);
                    } else {
                        let prefix = format!("{f}/");
                        let dirs: Vec<String> = s.dirs.iter().filter(|d| **d == f || d.starts_with(&prefix)).cloned().collect();
                        for d in dirs {
                            s.dirs.remove(&d);
                            s.dirs.insert(format!("{t}{}", &d[f.len()..]));
                        }
                        let files: Vec<String> = s.files.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
                        for k in files {
                            let b = s.files.remove(&k).expect("listed");
                            s.files.insert(format!("{t}{}", &k[f.len()..]), b);
                        }
                    }
                    (line, Self::status(id, FX_OK, "Success"), "STATUS 0".into())
                }
            }
            other => (format!("UNSUPPORTED {other}"), Self::status(id, 8, "Operation unsupported"), "STATUS 8 Operation unsupported".into()),
        };
        session.log(format!("> {line}"));
        session.log(format!("< {reply_line}"));
        Ok(Some(reply))
    }
}
