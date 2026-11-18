//! `FileUploaders/FTP.cs` and `FTPAccount.cs`: FTP and FTPS uploads.
//!
//! ShareX uses FluentFTP; [`FtpClient`] sends what FluentFTP sends for
//! ShareX's calls, command for command (checked against ShareX's own code
//! on a mock server, `tools/uploaders-harness`):
//!
//! - connect: `[TLS] 220`, `[AUTH TLS]`, `USER`, `PASS`, `[PBSZ 0, PROT P]`,
//!   `FEAT`, `OPTS UTF8 ON` (if listed), `SYST`, `PWD`;
//! - `OpenWrite`: `SIZE`, `TYPE I` (once), `EPSV` (passive) or `EPRT`
//!   falling back to `PORT` (active), `STOR`; the data, then the reply;
//! - `CreateDirectory(path)` (forced): the parent made first if
//!   `DirectoryExists` says it's missing (`CWD`, `PWD` for a relative
//!   path, `CWD` back), then `MKD`;
//! - `Dispose`: `QUIT`.
//!
//! FTPS (`AUTH TLS` explicit or implicit) accepts the server's certificate
//! when it doesn't validate, as ShareX does without a certificate file.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

use native_tls::{TlsConnector, TlsStream};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sharex_helpers::NameParserType;

use super::cloud_storage::name_parse;
use super::shared_folder::{browser_protocol_prefix, file_name_without_extension};
use super::uploader_base;
use crate::custom::CustomUploaderParser;
use crate::dpapi::Secret;
use crate::enums::{BrowserProtocol, FTPProtocol, FTPSEncryption};
use crate::http::SharedStream;
use crate::result::UploadResult;
use crate::uploader::{CoreResult, UResult, UploadError, Uploader, WritesBackConfig};
use crate::url_helpers;

fn new_account() -> Option<String> {
    Some("New account".into())
}
fn empty() -> Option<String> {
    Some(String::new())
}
fn port_21() -> i32 {
    21
}
fn yes() -> bool {
    true
}
fn protocol_ftp() -> FTPProtocol {
    FTPProtocol::FTP
}
fn browser_http() -> BrowserProtocol {
    BrowserProtocol::http
}
fn explicit() -> FTPSEncryption {
    FTPSEncryption::Explicit
}

/// `FTPAccount` (an item of `UploadersConfig.FTPAccountList`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct FTPAccount {
    #[serde(default = "new_account")]
    pub Name: Option<String>,
    #[serde(default = "protocol_ftp")]
    pub Protocol: FTPProtocol,
    #[serde(default = "empty")]
    pub Host: Option<String>,
    #[serde(default = "port_21")]
    pub Port: i32,
    #[serde(default)]
    pub Username: Option<String>,
    #[serde(default)]
    pub Password: Secret,
    #[serde(default)]
    pub IsActive: bool,
    #[serde(default = "empty")]
    pub SubFolderPath: Option<String>,
    #[serde(default = "browser_http")]
    pub BrowserProtocol: BrowserProtocol,
    #[serde(default = "empty")]
    pub HttpHomePath: Option<String>,
    #[serde(default = "yes")]
    pub HttpHomePathAutoAddSubFolderPath: bool,
    #[serde(default)]
    pub HttpHomePathNoExtension: bool,
    #[serde(default = "explicit")]
    pub FTPSEncryption: FTPSEncryption,
    #[serde(default = "empty")]
    pub FTPSCertificateLocation: Option<String>,
    #[serde(default)]
    pub Keypath: Option<String>,
    #[serde(default)]
    pub Passphrase: Secret,
}

impl Default for FTPAccount {
    fn default() -> Self {
        serde_json::from_value(Value::Object(Default::default())).expect("defaults")
    }
}

/// `ToString()` of a URL part that may be null.
fn s(o: &Option<String>) -> &str {
    o.as_deref().unwrap_or("")
}

/// `UriBuilder.Path = value`: escapes what a path can't hold, keeping
/// `%XX` escapes.
fn uri_builder_path(path: &str) -> String {
    let bytes = path.replace('\\', "/").into_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let escaped = b == b'%' && bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit) && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit);
        if escaped || b.is_ascii_alphanumeric() || b"-._~:/[]@!$&'()*+,;=".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
        i += 1;
    }
    out
}

impl FTPAccount {
    /// Reads `UploadersConfig.FTPAccountList`.
    pub fn list_from_value(v: &Value) -> Option<Vec<FTPAccount>> {
        match v {
            Value::Array(items) => Some(items.iter().map(|i| serde_json::from_value(i.clone()).unwrap_or_default()).collect()),
            _ => None,
        }
    }

    /// `FTPAddress`: `protocol://host:port`.
    pub fn ftp_address(&self) -> String {
        if s(&self.Host).is_empty() {
            return String::new();
        }
        let protocol = match self.Protocol {
            FTPProtocol::FTPS => "ftps://",
            FTPProtocol::SFTP => "sftp://",
            _ => "ftp://",
        };
        format!("{protocol}{}:{}", s(&self.Host), self.Port)
    }

    /// `GetSubFolderPath(fileName, nameParserType)`.
    pub fn get_sub_folder_path(&self, file_name: Option<&str>, kind: NameParserType) -> String {
        let path = name_parse(kind, &s(&self.SubFolderPath).replace("%host", s(&self.Host)));
        url_helpers::combine_url(&path, file_name.unwrap_or(""))
    }

    /// `GetHttpHomePath`: `%host`, then the name parser and the custom
    /// uploader syntax.
    pub fn get_http_home_path(&self) -> String {
        let home = s(&self.HttpHomePath).replace("%host", s(&self.Host));
        let parser = CustomUploaderParser { use_name_parser: true, name_parser_type: Some(NameParserType::Url), ..Default::default() };
        parser.parse(&home).unwrap_or_default()
    }

    /// `GetUriPath(fileName, subFolderPath)`: the URL the file gets.
    pub fn get_uri_path(&self, file_name: &str, sub_folder_path: Option<&str>) -> String {
        if s(&self.Host).is_empty() {
            return String::new();
        }
        let mut file_name = file_name.to_owned();
        if self.HttpHomePathNoExtension {
            file_name = file_name_without_extension(&file_name);
        }
        let file_name = url_helpers::url_encode(&file_name, false);
        let sub_folder_path = sub_folder_path.map(str::to_owned).unwrap_or_else(|| self.get_sub_folder_path(None, NameParserType::Url));
        let scheme = browser_protocol_prefix(&self.BrowserProtocol).trim_end_matches("://");
        let http_home_path = self.get_http_home_path();
        if http_home_path.is_empty() {
            let mut url = s(&self.Host).to_owned();
            if let Some(rest) = url.strip_prefix("ftp.") {
                url = rest.to_owned();
            }
            if self.HttpHomePathAutoAddSubFolderPath {
                url = url_helpers::combine_url(&url, &sub_folder_path);
            }
            url = url_helpers::combine_url(&url, &file_name);
            // new UriBuilder(url): parsed as an http URL, the port dropped.
            let absolute = crate::http::dotnet_absolute_uri(&format!("http://{url}")).unwrap_or_else(|_| format!("http://{url}"));
            let rest = absolute.split_once("://").map(|(_, r)| r).unwrap_or("");
            let (authority, path_query) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
            let host = authority.rsplit('@').next().unwrap_or("");
            let host = match host.rfind(':') {
                Some(i) if !host.ends_with(']') => &host[..i],
                _ => host,
            };
            let path_query = if path_query.is_empty() { "/" } else { path_query };
            return format!("{scheme}://{}{path_query}", host.to_ascii_lowercase());
        }
        // HttpHomePath as host[:port]/path[?query].
        let first_slash = http_home_path.find('/');
        let http_home = first_slash.map(|i| &http_home_path[..i]).unwrap_or(&http_home_path);
        let port_at = http_home.rfind(':');
        let host = port_at.map(|i| &http_home[..i]).unwrap_or(http_home);
        let path_and_query = first_slash.map(|i| &http_home_path[i + 1..]).unwrap_or("");
        let query_at = path_and_query.rfind('?');
        let dir = query_at.map(|i| &path_and_query[..i]).unwrap_or(path_and_query);
        let query = query_at.map(|i| &path_and_query[i + 1..]).unwrap_or("");
        // int.TryParse: 0 when it fails.
        let port: Option<i32> = port_at.map(|i| http_home[i + 1..].parse().unwrap_or(0));
        let mut path = uri_builder_path(dir);
        let mut query = if query.is_empty() { String::new() } else { format!("?{query}") };
        if query.ends_with('=') {
            query = if self.HttpHomePathAutoAddSubFolderPath {
                format!("?{}", url_helpers::combine_urls(&[&query[1..], &sub_folder_path, &file_name]))
            } else {
                format!("{query}{file_name}")
            };
        } else {
            if self.HttpHomePathAutoAddSubFolderPath {
                path = uri_builder_path(&url_helpers::combine_url(&path, &sub_folder_path));
            }
            path = uri_builder_path(&url_helpers::combine_url(&path, &file_name));
        }
        let port = match port {
            Some(p) if p >= 0 => format!(":{p}"),
            _ => String::new(),
        };
        let path = if path.starts_with('/') { path } else { format!("/{path}") };
        format!("{scheme}://{host}{port}{path}{query}")
    }

    /// `GetFtpPath(fileName)` (`PreviewFtpPath` with `example.png`).
    pub fn get_ftp_path(&self, file_name: &str) -> String {
        let address = self.ftp_address();
        if address.is_empty() {
            return String::new();
        }
        url_helpers::combine_url(&address, &self.get_sub_folder_path(Some(file_name), NameParserType::FilePath))
    }

    /// `PreviewHttpPath`.
    pub fn preview_http_path(&self) -> String {
        self.get_uri_path("example.png", None)
    }

    /// `ToString()`: `Name (Host:Port)`.
    pub fn display_name(&self) -> String {
        format!("{} ({}:{})", s(&self.Name), s(&self.Host), self.Port)
    }
}

// ---------------------------------------------------------------------------
// The FTP client.

/// A reply: code, message (the last line's text) and every line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtpReply {
    pub code: String,
    pub message: String,
    pub lines: Vec<String>,
}

impl FtpReply {
    /// `FtpReply.Success`: 1xx, 2xx, 3xx.
    pub fn success(&self) -> bool {
        matches!(self.code.as_bytes().first(), Some(b'1' | b'2' | b'3'))
    }
}

/// What FluentFTP throws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FtpError {
    /// `FtpCommandException` / `FtpAuthenticationException`: a failure reply.
    Command(FtpReply),
    /// Anything else (connection, TLS): the message.
    Other(String),
}

impl std::fmt::Display for FtpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FtpError::Command(r) => write!(f, "Code: {} Message: {}", r.code, r.message),
            FtpError::Other(m) => f.write_str(m),
        }
    }
}

impl From<std::io::Error> for FtpError {
    fn from(e: std::io::Error) -> Self {
        FtpError::Other(e.to_string())
    }
}

type FtpResult<T> = Result<T, FtpError>;

enum Conn {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
}

impl Conn {
    fn tcp(&self) -> &TcpStream {
        match self {
            Conn::Plain(s) => s,
            Conn::Tls(s) => s.get_ref(),
        }
    }
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

/// How data connections are made (`FtpDataConnectionType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataConnection {
    /// `AutoPassive`: `EPSV`.
    AutoPassive,
    /// `AutoActive`: `EPRT`, else `PORT`.
    AutoActive,
}

/// FTPS settings (`FtpEncryptionMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encryption {
    None,
    Explicit,
    Implicit,
}

/// FluentFTP's `FtpClient`, as ShareX uses it.
pub struct FtpClient {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub data_connection: DataConnection,
    pub encryption: Encryption,
    /// `DataConnectionEncryption` (ShareX: on for FTPS).
    pub data_encryption: bool,
    /// The connect and read timeout.
    pub timeout: Duration,
    reader: Option<BufReader<Conn>>,
    features: Vec<String>,
    working_directory: String,
    binary: bool,
}

fn tls_connector() -> FtpResult<TlsConnector> {
    TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .map_err(|e| FtpError::Other(e.to_string()))
}

impl FtpClient {
    pub fn new(host: &str, port: u16, username: &str, password: &str) -> Self {
        Self {
            host: host.to_owned(),
            port,
            username: username.to_owned(),
            password: password.to_owned(),
            data_connection: DataConnection::AutoPassive,
            encryption: Encryption::None,
            data_encryption: false,
            timeout: Duration::from_secs(15),
            reader: None,
            features: Vec::new(),
            working_directory: "/".into(),
            binary: false,
        }
    }

    /// `IsConnected`.
    pub fn is_connected(&self) -> bool {
        self.reader.is_some()
    }

    fn conn(&mut self) -> FtpResult<&mut BufReader<Conn>> {
        self.reader.as_mut().ok_or_else(|| FtpError::Other("The FTP client is not connected.".into()))
    }

    /// `GetReply()`: one reply, multi-line ones whole.
    pub fn get_reply(&mut self) -> FtpResult<FtpReply> {
        let reader = self.conn()?;
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                return Err(FtpError::Other("The connection was closed by the server.".into()));
            }
            let line = line.trim_end_matches(['\r', '\n']).to_owned();
            lines.push(line.clone());
            let b = line.as_bytes();
            if b.len() >= 4 && b[..3].iter().all(u8::is_ascii_digit) && b[3] == b' ' {
                return Ok(FtpReply { code: line[..3].to_owned(), message: line[4..].to_owned(), lines });
            }
            if b.len() == 3 && b.iter().all(u8::is_ascii_digit) && lines.len() == 1 {
                return Ok(FtpReply { code: line.clone(), message: String::new(), lines });
            }
        }
    }

    /// `Execute(command)`.
    pub fn execute(&mut self, command: &str) -> FtpResult<FtpReply> {
        let conn = self.conn()?.get_mut();
        conn.write_all(format!("{command}\r\n").as_bytes())?;
        conn.flush()?;
        self.get_reply()
    }

    fn execute_ok(&mut self, command: &str) -> FtpResult<FtpReply> {
        let reply = self.execute(command)?;
        if reply.success() { Ok(reply) } else { Err(FtpError::Command(reply)) }
    }

    fn connect_tcp(&self) -> FtpResult<TcpStream> {
        let addrs: Vec<SocketAddr> = (self.host.as_str(), self.port).to_socket_addrs().map_err(|_| FtpError::Other("Failed to connect to host.".into()))?.collect();
        for addr in addrs {
            if let Ok(s) = TcpStream::connect_timeout(&addr, self.timeout) {
                s.set_read_timeout(Some(self.timeout))?;
                s.set_write_timeout(Some(self.timeout))?;
                return Ok(s);
            }
        }
        Err(FtpError::Other("Failed to connect to host.".into()))
    }

    fn upgrade_tls(&mut self) -> FtpResult<()> {
        let Some(reader) = self.reader.take() else { return Err(FtpError::Other("The FTP client is not connected.".into())) };
        let Conn::Plain(tcp) = reader.into_inner() else { return Err(FtpError::Other("TLS is already on.".into())) };
        let tls = tls_connector()?.connect(&self.host, tcp).map_err(|e| FtpError::Other(e.to_string()))?;
        self.reader = Some(BufReader::new(Conn::Tls(Box::new(tls))));
        Ok(())
    }

    /// `Connect()`.
    pub fn connect(&mut self) -> FtpResult<()> {
        let tcp = self.connect_tcp()?;
        let conn = if self.encryption == Encryption::Implicit {
            Conn::Tls(Box::new(tls_connector()?.connect(&self.host, tcp).map_err(|e| FtpError::Other(e.to_string()))?))
        } else {
            Conn::Plain(tcp)
        };
        self.reader = Some(BufReader::new(conn));
        let banner = self.get_reply()?;
        if !banner.success() {
            return Err(FtpError::Command(banner));
        }
        if self.encryption == Encryption::Explicit {
            self.execute_ok("AUTH TLS")?;
            self.upgrade_tls()?;
        }
        let user = self.execute(&format!("USER {}", self.username))?;
        if !user.success() {
            return Err(FtpError::Command(user));
        }
        if user.code.starts_with('3') {
            let pass = self.execute(&format!("PASS {}", self.password))?;
            if !pass.success() {
                return Err(FtpError::Command(pass));
            }
        }
        if self.encryption != Encryption::None {
            self.execute_ok("PBSZ 0")?;
            self.execute_ok(if self.data_encryption { "PROT P" } else { "PROT C" })?;
        }
        let feat = self.execute("FEAT")?;
        self.features = if feat.success() { feat.lines.iter().skip(1).filter(|l| l.starts_with(' ')).map(|l| l.trim().to_ascii_uppercase()).collect() } else { Vec::new() };
        if self.has_feature("UTF8") {
            self.execute("OPTS UTF8 ON")?;
        }
        self.execute("SYST")?;
        let pwd = self.execute("PWD")?;
        if pwd.success()
            && let Some(dir) = pwd.message.split('"').nth(1)
        {
            self.working_directory = dir.to_owned();
        }
        Ok(())
    }

    fn has_feature(&self, name: &str) -> bool {
        self.features.iter().any(|f| f == name || f.starts_with(&format!("{name} ")))
    }

    /// `Disconnect()` / `Dispose()`: `QUIT`, then the connection closes.
    pub fn disconnect(&mut self) {
        if self.reader.is_some() {
            let _ = self.execute("QUIT");
        }
        if let Some(r) = self.reader.take() {
            match r.into_inner() {
                Conn::Plain(s) => {
                    let _ = s.shutdown(Shutdown::Both);
                }
                Conn::Tls(mut s) => {
                    let _ = s.shutdown();
                    let _ = s.get_ref().shutdown(Shutdown::Both);
                }
            }
        }
    }

    /// `DirectoryExists(path)`.
    pub fn directory_exists(&mut self, path: &str) -> FtpResult<bool> {
        let path = path.trim_end_matches('/');
        if path.is_empty() || path == "." || path == "/" {
            return Ok(true);
        }
        let pwd = self.working_directory.clone();
        if self.execute(&format!("CWD {path}"))?.success() {
            if !path.starts_with('/') {
                self.execute("PWD")?;
            }
            self.execute_ok(&format!("CWD {pwd}"))?;
            return Ok(true);
        }
        Ok(false)
    }

    /// `CreateDirectory(path)` (forced: the missing parents too).
    pub fn create_directory(&mut self, path: &str) -> FtpResult<()> {
        let path = path.trim_end_matches('/');
        if path.is_empty() || path == "/" {
            return Ok(());
        }
        let parent = url_helpers::get_directory_path(path);
        let parent = if parent == path { String::new() } else { parent };
        if !parent.is_empty() && parent != "/" && !self.directory_exists(&parent)? {
            self.create_directory(&parent)?;
        }
        self.execute_ok(&format!("MKD {path}"))?;
        Ok(())
    }

    /// Opens a data connection for `command` (`TYPE I`, `EPSV` or
    /// `EPRT`/`PORT`, then the command).
    fn open_data(&mut self, command: &str) -> FtpResult<DataStream> {
        if !self.binary {
            self.execute_ok("TYPE I")?;
            self.binary = true;
        }
        let stream = match self.data_connection {
            DataConnection::AutoPassive => {
                let reply = self.execute_ok("EPSV")?;
                let port: u16 = reply
                    .message
                    .split('|')
                    .nth(3)
                    .and_then(|p| p.parse().ok())
                    .ok_or_else(|| FtpError::Other("Failed to get the EPSV port from: ".to_owned() + &reply.message))?;
                let ip = self.conn()?.get_ref().tcp().peer_addr()?.ip();
                let tcp = TcpStream::connect_timeout(&SocketAddr::new(ip, port), self.timeout)?;
                self.execute_ok(command)?;
                tcp
            }
            DataConnection::AutoActive => {
                let local: IpAddr = self.conn()?.get_ref().tcp().local_addr()?.ip();
                let listener = TcpListener::bind((local, 0))?;
                let port = listener.local_addr()?.port();
                let family = if local.is_ipv6() { 2 } else { 1 };
                let eprt = self.execute(&format!("EPRT |{family}|{local}|{port}|"))?;
                if !eprt.success() {
                    let IpAddr::V4(v4) = local else { return Err(FtpError::Command(eprt)) };
                    let o = v4.octets();
                    self.execute_ok(&format!("PORT {},{},{},{},{},{}", o[0], o[1], o[2], o[3], port >> 8, port & 0xff))?;
                }
                self.execute_ok(command)?;
                listener.set_nonblocking(false)?;
                let (tcp, _) = listener.accept()?;
                tcp
            }
        };
        stream.set_write_timeout(Some(self.timeout))?;
        if self.encryption != Encryption::None && self.data_encryption {
            let tls = tls_connector()?.connect(&self.host, stream).map_err(|e| FtpError::Other(e.to_string()))?;
            Ok(DataStream::Tls(Box::new(tls)))
        } else {
            Ok(DataStream::Plain(stream))
        }
    }

    /// `OpenWrite(path)`: `SIZE` (whether it exists), then `STOR`.
    pub fn open_write(&mut self, path: &str) -> FtpResult<DataStream> {
        self.execute(&format!("SIZE {path}"))?;
        self.open_data(&format!("STOR {path}"))
    }
}

/// An open data connection.
pub enum DataStream {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
}

impl Write for DataStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            DataStream::Plain(s) => s.write(buf),
            DataStream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            DataStream::Plain(s) => s.flush(),
            DataStream::Tls(s) => s.flush(),
        }
    }
}

impl DataStream {
    /// Closes the data connection (the end of the file for the server).
    pub fn close(self) {
        match self {
            DataStream::Plain(s) => {
                let _ = s.shutdown(Shutdown::Both);
            }
            DataStream::Tls(mut s) => {
                let _ = s.shutdown();
                let _ = s.get_ref().shutdown(Shutdown::Both);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The uploader.

/// `FTP`: FTP and FTPS uploads to an [`FTPAccount`].
pub struct Ftp {
    pub base: Uploader,
    pub account: FTPAccount,
    client: FtpClient,
}

uploader_base!(Ftp, FileUploader);
impl WritesBackConfig for Ftp {}

impl Ftp {
    pub fn new(account: FTPAccount) -> Self {
        let port = u16::try_from(account.Port).unwrap_or(21);
        let mut client = FtpClient::new(s(&account.Host), port, s(&account.Username), account.Password.as_str());
        client.data_connection = if account.IsActive { DataConnection::AutoActive } else { DataConnection::AutoPassive };
        if account.Protocol == FTPProtocol::FTPS {
            client.encryption = if account.FTPSEncryption == FTPSEncryption::Implicit { Encryption::Implicit } else { Encryption::Explicit };
            client.data_encryption = true;
            // A client certificate (FTPSCertificateLocation) isn't sent: the
            // OS TLS libraries need the private key with it.
        }
        Self { base: Uploader::new(), account, client }
    }

    /// The client, for the remote storage browser (listing aside).
    pub fn client(&mut self) -> &mut FtpClient {
        &mut self.client
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if self.base.stop_requested() {
            return Err(UploadError::Cancelled);
        }
        let mut result = UploadResult::default();
        let sub_folder_path = self.account.get_sub_folder_path(None, NameParserType::FilePath);
        let path = url_helpers::combine_url(&sub_folder_path, file_name);
        let url = self.account.get_uri_path(file_name, Some(&sub_folder_path));
        self.base.early_url_copy_requested(&url);
        let uploaded = self.upload_data(data, &path);
        self.client.disconnect();
        let uploaded = uploaded?;
        if uploaded && !self.base.stop_requested() && !self.base.is_error() {
            result.url = Some(url);
        }
        Ok(Some(result))
    }

    fn connect(&mut self) -> UResult<bool> {
        if !self.client.is_connected() {
            self.client.connect().map_err(|e| UploadError::Exception(e.to_string()))?;
        }
        Ok(self.client.is_connected())
    }

    /// `UploadData(stream, remotePath)`: a missing directory (550, 553) is
    /// created and the upload tried again.
    pub fn upload_data(&mut self, data: &SharedStream, remote_path: &str) -> UResult<bool> {
        if !self.connect()? {
            return Ok(false);
        }
        match self.upload_data_internal(data, remote_path) {
            Err(FtpError::Command(reply)) if reply.code == "550" || reply.code == "553" => {
                self.create_multi_directory(&url_helpers::get_directory_path(remote_path));
                self.upload_data_internal(data, remote_path).map_err(|e| UploadError::Exception(e.to_string()))
            }
            r => r.map_err(|e| UploadError::Exception(e.to_string())),
        }
    }

    fn upload_data_internal(&mut self, data: &SharedStream, remote_path: &str) -> FtpResult<bool> {
        let mut remote = self.client.open_write(remote_path)?;
        let transferred = self.base.transfer_data(data, &mut remote, 0, None);
        remote.close();
        let transferred = match transferred {
            Ok(t) => t,
            Err(UploadError::Cancelled) => false,
            Err(UploadError::Exception(m)) => return Err(FtpError::Other(m)),
        };
        let reply = self.client.get_reply()?;
        Ok(transferred && reply.success())
    }

    /// `CreateDirectory(remotePath)`: `false` (and logged) when it fails.
    pub fn create_directory(&mut self, remote_path: &str) -> bool {
        if self.connect().unwrap_or(false) {
            return self.client.create_directory(remote_path).is_ok();
        }
        false
    }

    /// `CreateMultiDirectory`: every level of the path.
    pub fn create_multi_directory(&mut self, remote_path: &str) -> Vec<String> {
        let paths = url_helpers::get_paths(remote_path);
        for p in &paths {
            self.create_directory(p);
        }
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> FTPAccount {
        FTPAccount {
            Host: Some("ftp.example.com".into()),
            SubFolderPath: Some("shots".into()),
            HttpHomePath: Some(String::new()),
            BrowserProtocol: BrowserProtocol::https,
            ..Default::default()
        }
    }

    #[test]
    fn uri_paths_as_sharex() {
        let a = account();
        assert_eq!(a.get_uri_path("my shot.png", None), "https://example.com/shots/my%20shot.png");
        let b = FTPAccount { HttpHomePath: Some("cdn.example.com:8080/x?f=".into()), SubFolderPath: Some(String::new()), ..account() };
        assert_eq!(b.get_uri_path("my shot.png", None), "https://cdn.example.com:8080/x?f=/my%20shot.png");
        let c = FTPAccount { HttpHomePath: Some("%host/~me".into()), HttpHomePathNoExtension: true, ..account() };
        assert_eq!(c.get_uri_path("a.png", None), "https://ftp.example.com/~me/shots/a");
        assert_eq!(a.get_ftp_path("example.png"), "ftp://ftp.example.com:21/shots/example.png");
        assert_eq!(uri_builder_path("a b/%20c#"), "a%20b/%20c%23");
    }
}
