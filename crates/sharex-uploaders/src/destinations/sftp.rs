//! `FileUploaders/SFTP.cs`: SFTP uploads to an [`FTPAccount`] whose
//! protocol is SFTP. ShareX uses SSH.NET; [`SftpSession`] makes the
//! requests SSH.NET's `SftpClient` makes for ShareX's calls (checked
//! against ShareX's own code on a mock server, `tools/uploaders-harness`):
//!
//! - log in: `none`, then the key (RSA: `ssh-rsa` then `rsa-sha2-512`,
//!   each asked first) or the password;
//! - `INIT 3`, `REALPATH .` (the working directory);
//! - `GetCanonicalPath`: `REALPATH` of the full path, else of its parent
//!   with the name appended, else the full path as is;
//! - `UploadFile`: `OPEN` (write, create, truncate), `WRITE`s of the
//!   buffer size less the request's other fields, `CLOSE`;
//! - `Exists`: `LSTAT` of the canonical path; `CreateDirectory`: `MKDIR`.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use sharex_helpers::NameParserType;
use sharex_strings::uploaders_lib as strings;

use super::ftp::FTPAccount;
use super::uploader_base;
use crate::http::{self, SharedStream};
use crate::result::UploadResult;
use crate::sftp::{Attrs, DirEntry, PF_CREAT, PF_READ, PF_TRUNC, PF_WRITE, SftpClient, SftpError};
use crate::ssh::{self, Channel, Reader, SshError, Transport, UserKey, Writer};
use crate::uploader::{CoreResult, UResult, UploadError, Uploader, WritesBackConfig};
use crate::url_helpers;

/// The identification ShareXrs sends (SSH.NET: `SSH-2.0-Renci.SshNet.SshClient.<version>`).
pub fn client_version() -> String {
    format!("SSH-2.0-ShareXrs_{}", env!("CARGO_PKG_VERSION"))
}

/// What a login uses.
pub enum Credentials {
    Password(String),
    Key(Box<UserKey>),
}

/// `new PrivateKeyFile(path[, passphrase])`: OpenSSH keys (encrypted or
/// not) and unencrypted PEM RSA keys (PKCS#1, PKCS#8).
pub fn load_private_key(text: &str, passphrase: Option<&str>) -> Result<UserKey, String> {
    if text.contains("BEGIN OPENSSH PRIVATE KEY") {
        let key = ssh_key::PrivateKey::from_openssh(text).map_err(|e| e.to_string())?;
        return if key.is_encrypted() {
            key.decrypt(passphrase.unwrap_or("").as_bytes()).map_err(|_| "The passphrase is wrong or the key is invalid.".to_owned())
        } else {
            Ok(key)
        };
    }
    use rsa::pkcs1::DecodeRsaPrivateKey;
    use rsa::pkcs8::DecodePrivateKey;
    let rsa = if text.contains("BEGIN RSA PRIVATE KEY") {
        if text.contains("ENCRYPTED") {
            return Err("Encrypted PEM keys aren't supported; convert the key to the OpenSSH format (ssh-keygen -p).".into());
        }
        rsa::RsaPrivateKey::from_pkcs1_pem(text).map_err(|e| e.to_string())?
    } else if text.contains("BEGIN PRIVATE KEY") {
        rsa::RsaPrivateKey::from_pkcs8_pem(text).map_err(|e| e.to_string())?
    } else {
        return Err("Invalid private key file.".into());
    };
    let keypair = ssh_key::private::RsaKeypair::try_from(rsa).map_err(|e| e.to_string())?;
    ssh_key::PrivateKey::new(ssh_key::private::KeypairData::Rsa(keypair), "").map_err(|e| e.to_string())
}

/// The `rsa` crate's key of an RSA key, from its components.
fn rsa_private(key: &UserKey) -> Result<rsa::RsaPrivateKey, String> {
    let ssh_key::private::KeypairData::Rsa(kp) = key.key_data() else { return Err("not an RSA key".into()) };
    let n = |m: &ssh_key::Mpint| rsa::BigUint::from_bytes_be(m.as_positive_bytes().unwrap_or(&[]));
    let mut k = rsa::RsaPrivateKey::from_components(n(&kp.public.n), n(&kp.public.e), n(&kp.private.d), vec![n(&kp.private.p), n(&kp.private.q)])
        .map_err(|e| e.to_string())?;
    k.precompute().map_err(|e| e.to_string())?;
    Ok(k)
}

/// An SSH-1 `ssh-rsa` signature (PKCS#1 v1.5, SHA-1), which SSH.NET tries
/// first with RSA keys.
fn sign_rsa_sha1(key: &UserKey, data: &[u8]) -> Result<Vec<u8>, String> {
    use rsa::signature::{SignatureEncoding, Signer};
    let private = rsa_private(key)?;
    let sig = rsa::pkcs1v15::SigningKey::<sha1::Sha1>::new(private).sign(data);
    Ok(Writer::default().string("ssh-rsa").bytes(&sig.to_bytes()).0)
}

/// An `rsa-sha2-256` / `rsa-sha2-512` signature blob.
fn sign_rsa_sha2(key: &UserKey, algorithm: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    use rsa::signature::{SignatureEncoding, Signer};
    let private = rsa_private(key)?;
    let sig = if algorithm == "rsa-sha2-256" {
        rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private).sign(data).to_bytes()
    } else {
        rsa::pkcs1v15::SigningKey::<sha2::Sha512>::new(private).sign(data).to_bytes()
    };
    Ok(Writer::default().string(algorithm).bytes(&sig).0)
}

/// SSH.NET's login: `none`, then the key's algorithms or the password.
/// A refused login closes the connection without a goodbye.
fn login(t: &mut Transport, user: &str, credentials: &Credentials) -> Result<(), String> {
    let e = |e: SshError| e.0;
    t.send(&Writer::new(ssh::MSG_SERVICE_REQUEST).string("ssh-userauth").0).map_err(e)?;
    if t.recv().map_err(e)?.first() != Some(&ssh::MSG_SERVICE_ACCEPT) {
        return Err("The server refused the authentication service.".into());
    }
    let request = || Writer::new(ssh::MSG_USERAUTH_REQUEST).string(user).string("ssh-connection");
    t.send(&request().string("none").0).map_err(e)?;
    if auth_reply(t).map_err(e)?.is_none() {
        return Ok(());
    }
    match credentials {
        Credentials::Password(password) => {
            t.send(&request().string("password").bool(false).string(password).0).map_err(e)?;
            match auth_reply(t).map_err(e)? {
                None => Ok(()),
                Some(_) => Err("Permission denied (password).".into()),
            }
        }
        Credentials::Key(key) => {
            let blob = key.public_key().to_bytes().map_err(|e| e.to_string())?;
            let algorithms: Vec<&str> = match key.algorithm() {
                ssh_key::Algorithm::Rsa { .. } => vec!["ssh-rsa", "rsa-sha2-512", "rsa-sha2-256"],
                _ => vec![""],
            };
            let ask_first = algorithms.len() > 1;
            for algorithm in algorithms {
                let algorithm = if algorithm.is_empty() { key.algorithm().as_str().to_owned() } else { algorithm.to_owned() };
                if ask_first {
                    t.send(&request().string("publickey").bool(false).string(&algorithm).bytes(&blob).0).map_err(e)?;
                    let p = t.recv().map_err(e)?;
                    if p.first() != Some(&ssh::MSG_USERAUTH_PK_OK) {
                        continue;
                    }
                }
                let to_sign = Writer::default()
                    .bytes(&t.session_id)
                    .byte(ssh::MSG_USERAUTH_REQUEST)
                    .string(user)
                    .string("ssh-connection")
                    .string("publickey")
                    .bool(true)
                    .string(&algorithm)
                    .bytes(&blob)
                    .0;
                let signature = match algorithm.as_str() {
                    "ssh-rsa" => sign_rsa_sha1(key, &to_sign)?,
                    "rsa-sha2-256" | "rsa-sha2-512" => sign_rsa_sha2(key, &algorithm, &to_sign)?,
                    _ => ssh::sign_user_auth(key, &to_sign).map_err(e)?.1,
                };
                t.send(&request().string("publickey").bool(true).string(&algorithm).bytes(&blob).bytes(&signature).0).map_err(e)?;
                if auth_reply(t).map_err(e)?.is_none() {
                    return Ok(());
                }
            }
            Err("Permission denied (publickey).".into())
        }
    }
}

fn auth_reply(t: &mut Transport) -> ssh::SshResult<Option<Vec<String>>> {
    loop {
        let p = t.recv()?;
        match p.first().copied() {
            Some(ssh::MSG_USERAUTH_SUCCESS) => return Ok(None),
            Some(ssh::MSG_USERAUTH_FAILURE) => return Ok(Some(Reader::new(&p[1..]).name_list()?)),
            Some(ssh::MSG_USERAUTH_BANNER) => continue,
            _ => return Err(SshError("Unexpected message during authentication.".into())),
        }
    }
}

/// An SFTP connection as SSH.NET's `SftpClient` uses it.
pub struct SftpSession {
    client: SftpClient,
    /// `WorkingDirectory`.
    pub working_directory: String,
    max_packet: u32,
}

impl SftpSession {
    /// Connects, logs in and starts SFTP.
    pub fn connect(host: &str, port: u16, user: &str, credentials: &Credentials, timeout: Duration) -> Result<Self, String> {
        let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs().map_err(|e| e.to_string())?.collect();
        let mut last = "No such host is known.".to_owned();
        let mut stream = None;
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, timeout) {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(e) => last = socket_error_message(&e, &addr),
            }
        }
        let stream = stream.ok_or(last)?;
        stream.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
        let mut t = Transport::connect(stream, &client_version()).map_err(|e| e.0)?;
        login(&mut t, user, credentials)?;
        let channel = Channel::open_subsystem(t, "sftp").map_err(|e| e.0)?;
        let max_packet = channel_max_packet(&channel);
        let mut client = SftpClient::start(channel).map_err(|e| e.to_string())?;
        let working_directory = client.realpath(".").map_err(|e| e.to_string())?;
        Ok(Self { client, working_directory, max_packet })
    }

    /// `GetFullRemotePath`.
    fn full_path(&self, path: &str) -> String {
        if path.starts_with('/') {
            path.to_owned()
        } else if self.working_directory.ends_with('/') {
            format!("{}{path}", self.working_directory)
        } else {
            format!("{}/{path}", self.working_directory)
        }
    }

    /// `GetCanonicalPath`.
    pub fn canonical_path(&mut self, path: &str) -> Result<String, SftpError> {
        let full = self.full_path(path);
        if let Some(p) = ok_or_not_found(self.client.realpath(&full))? {
            return Ok(p);
        }
        if full.ends_with('/') || full.ends_with("/.") || full == "." {
            return Ok(full);
        }
        let slash = full.rfind('/').unwrap_or(0);
        let (parent, name) = (&full[..slash], &full[slash + 1..]);
        match ok_or_not_found(self.client.realpath(if parent.is_empty() { "/" } else { parent }))? {
            Some(c) if c.ends_with('/') => Ok(format!("{c}{name}")),
            Some(c) => Ok(format!("{c}/{name}")),
            None => Ok(full),
        }
    }

    /// `Exists(path)`.
    pub fn exists(&mut self, path: &str) -> Result<bool, SftpError> {
        let p = self.canonical_path(path)?;
        Ok(ok_or_not_found(self.client.lstat(&p))?.is_some())
    }

    /// `CreateDirectory(path)`.
    pub fn create_directory(&mut self, path: &str) -> Result<(), SftpError> {
        let p = self.canonical_path(path)?;
        self.client.mkdir(&p)
    }

    /// `UploadFile(stream, path, canOverride: true, progress)`; a missing
    /// directory is [`UploadFailure::PathNotFound`] (`SftpPathNotFoundException`).
    pub fn upload_file(&mut self, base: &mut Uploader, data: &SharedStream, path: &str, buffer_size: usize) -> Result<bool, UploadFailure> {
        let s = |e: SftpError| UploadFailure::Other(UploadError::Exception(e.to_string()));
        let p = self.canonical_path(path).map_err(s)?;
        let handle = match self.client.open(&p, PF_WRITE | PF_CREAT | PF_TRUNC) {
            Ok(h) => h,
            Err(SftpError::Status(2, m)) => return Err(UploadFailure::PathNotFound(format!("{m}. Path: '{p}'."))),
            Err(e) => return Err(s(e)),
        };
        let chunk = (buffer_size.min(self.max_packet as usize)).saturating_sub(25 + handle.len()).max(1);
        let io = |e: std::io::Error| UploadFailure::Other(UploadError::Exception(e.to_string()));
        let total = http::stream_length(data).map_err(io)?;
        let mut progress = http::ProgressManager::new(total, 0);
        let callback = base.progress_callback();
        let mut offset = 0u64;
        let mut buffer = vec![0u8; chunk];
        {
            use std::io::{Read, Seek, SeekFrom};
            let mut stream = data.lock().expect("upload stream");
            stream.seek(SeekFrom::Start(0)).map_err(io)?;
            loop {
                if base.stop_requested() {
                    return Ok(false);
                }
                let n = stream.read(&mut buffer).map_err(io)?;
                if n == 0 {
                    break;
                }
                let id = self.client.send_write(&handle, offset, &buffer[..n]).map_err(s)?;
                self.client.read_status(id).map_err(s)?;
                offset += n as u64;
                if progress.update_progress(n as u64)
                    && let Some(cb) = &callback
                {
                    cb(&progress);
                }
            }
        }
        self.client.close_handle(&handle).map_err(s)?;
        Ok(!base.stop_requested())
    }

    /// `DownloadFileAsync(path, stream)`: `OPEN` (read), `READ`s until
    /// the end, `CLOSE`.
    pub fn download_file(&mut self, path: &str, out: &mut dyn std::io::Write) -> Result<(), SftpError> {
        let p = self.canonical_path(path)?;
        let handle = self.client.open(&p, PF_READ)?;
        let chunk = self.max_packet.saturating_sub(13).max(1);
        let mut offset = 0u64;
        while let Some(data) = self.client.read(&handle, offset, chunk)? {
            out.write_all(&data).map_err(|e| SftpError::Ssh(e.to_string()))?;
            offset += data.len() as u64;
        }
        self.client.close_handle(&handle)
    }

    /// `UploadFileAsync(stream, path, canOverride: true)`.
    pub fn upload_stream(&mut self, source: &mut dyn std::io::Read, path: &str, buffer_size: usize) -> Result<(), SftpError> {
        let p = self.canonical_path(path)?;
        let handle = self.client.open(&p, PF_WRITE | PF_CREAT | PF_TRUNC)?;
        let chunk = (buffer_size.min(self.max_packet as usize)).saturating_sub(25 + handle.len()).max(1);
        let mut buffer = vec![0u8; chunk];
        let mut offset = 0u64;
        loop {
            let n = source.read(&mut buffer).map_err(|e| SftpError::Ssh(e.to_string()))?;
            if n == 0 {
                break;
            }
            let id = self.client.send_write(&handle, offset, &buffer[..n])?;
            self.client.read_status(id)?;
            offset += n as u64;
        }
        self.client.close_handle(&handle)
    }

    /// `ListDirectoryAsync` (remote storage browser).
    pub fn list_directory(&mut self, path: &str) -> Result<Vec<DirEntry>, SftpError> {
        let p = self.canonical_path(path)?;
        self.client.list_directory(&p)
    }

    /// `DeleteFileAsync`.
    pub fn delete_file(&mut self, path: &str) -> Result<(), SftpError> {
        let p = self.canonical_path(path)?;
        self.client.remove(&p)
    }

    /// `DeleteDirectoryAsync`.
    pub fn delete_directory(&mut self, path: &str) -> Result<(), SftpError> {
        let p = self.canonical_path(path)?;
        self.client.rmdir(&p)
    }

    /// `RenameAsync`.
    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), SftpError> {
        let (a, b) = (self.canonical_path(from)?, self.canonical_path(to)?);
        self.client.rename(&a, &b)
    }

    /// `STAT` of a path.
    pub fn stat(&mut self, path: &str) -> Result<Attrs, SftpError> {
        let p = self.canonical_path(path)?;
        self.client.stat(&p)
    }

    /// `Disconnect()`.
    pub fn disconnect(self) {
        self.client.close();
    }
}

/// How an upload failed.
#[derive(Debug)]
pub enum UploadFailure {
    /// `SftpPathNotFoundException`: the directory doesn't exist.
    PathNotFound(String),
    Other(UploadError),
}

fn channel_max_packet(channel: &Channel) -> u32 {
    channel.remote_max_packet()
}

/// `nullOnError` for "no such file".
fn ok_or_not_found<T>(r: Result<T, SftpError>) -> Result<Option<T>, SftpError> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.is_not_found() => Ok(None),
        Err(e) => Err(e),
    }
}

/// A connection failure as .NET's `SocketException` words it.
fn socket_error_message(e: &std::io::Error, addr: &SocketAddr) -> String {
    match e.kind() {
        std::io::ErrorKind::ConnectionRefused if cfg!(windows) => {
            format!("No connection could be made because the target machine actively refused it. {addr}")
        }
        std::io::ErrorKind::ConnectionRefused => format!("Connection refused {addr}"),
        std::io::ErrorKind::TimedOut => "Connection failed to establish within 30000 milliseconds.".into(),
        _ => e.to_string(),
    }
}

/// `SFTP`.
pub struct Sftp {
    pub base: Uploader,
    pub account: FTPAccount,
    session: Option<SftpSession>,
    /// The connect timeout (SSH.NET's 30 seconds).
    pub timeout: Duration,
}

uploader_base!(Sftp, FileUploader);
impl WritesBackConfig for Sftp {}

impl Sftp {
    pub fn new(account: FTPAccount) -> Self {
        Self { base: Uploader::new(), account, session: None, timeout: Duration::from_secs(30) }
    }

    /// `IsValidAccount`.
    pub fn is_valid_account(&self) -> bool {
        self.account.Keypath.as_deref().is_some_and(|k| !k.is_empty() && std::path::Path::new(k).is_file()) || !self.account.Password.is_empty()
    }

    /// `EnsureClient` + `Connect()`: `false` without credentials.
    fn connect(&mut self) -> UResult<bool> {
        if self.session.is_some() {
            return Ok(true);
        }
        let credentials = match self.account.Keypath.as_deref().filter(|k| !k.is_empty()) {
            Some(path) => {
                let Ok(text) = std::fs::read_to_string(path) else {
                    return Err(UploadError::Exception(strings::SFTP_Key_file_not_found().into()));
                };
                let passphrase = self.account.Passphrase.get().filter(|p| !p.is_empty());
                Credentials::Key(Box::new(load_private_key(&text, passphrase).map_err(UploadError::Exception)?))
            }
            None if !self.account.Password.is_empty() => Credentials::Password(self.account.Password.as_str().to_owned()),
            None => return Ok(false),
        };
        let host = self.account.Host.clone().unwrap_or_default();
        let port = u16::try_from(self.account.Port).unwrap_or(22);
        let user = self.account.Username.clone().unwrap_or_default();
        self.session = Some(SftpSession::connect(&host, port, &user, &credentials, self.timeout).map_err(UploadError::Exception)?);
        Ok(true)
    }

    /// `Disconnect()`.
    pub fn disconnect(&mut self) {
        if let Some(s) = self.session.take() {
            s.disconnect();
        }
    }

    /// The session, connecting first (remote storage browser).
    pub fn session(&mut self) -> UResult<Option<&mut SftpSession>> {
        if !self.connect()? {
            return Ok(None);
        }
        Ok(self.session.as_mut())
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if self.base.stop_requested() {
            return Err(UploadError::Cancelled);
        }
        let mut result = UploadResult::default();
        let sub_folder_path = self.account.get_sub_folder_path(None, NameParserType::Url);
        let path = url_helpers::combine_url(&sub_folder_path, file_name);
        let url = self.account.get_uri_path(file_name, Some(&sub_folder_path));
        self.base.early_url_copy_requested(&url);
        let uploaded = self.upload_stream(data, &path, true);
        if let Some(s) = self.session.take() {
            s.disconnect();
        }
        if uploaded? && !self.base.stop_requested() && !self.base.is_error() {
            result.url = Some(url);
        }
        Ok(Some(result))
    }

    /// `UploadStream(stream, remotePath, autoCreateDirectory)`.
    fn upload_stream(&mut self, data: &SharedStream, remote_path: &str, auto_create_directory: bool) -> UResult<bool> {
        if !self.connect()? {
            return Ok(false);
        }
        let buffer_size = self.base.buffer_size;
        let session = self.session.as_mut().expect("connected");
        match session.upload_file(&mut self.base, data, remote_path, buffer_size) {
            Ok(b) => Ok(b),
            Err(UploadFailure::PathNotFound(_)) if auto_create_directory => {
                self.create_directory(&url_helpers::get_directory_path(remote_path), true)?;
                self.upload_stream(data, remote_path, false)
            }
            Err(UploadFailure::PathNotFound(m)) => Err(UploadError::Exception(m)),
            Err(UploadFailure::Other(e)) => Err(e),
        }
    }

    /// `CreateDirectory(path, createMultiDirectory)`.
    pub fn create_directory(&mut self, path: &str, create_multi_directory: bool) -> UResult<()> {
        if !self.connect()? {
            return Ok(());
        }
        let r = self.session.as_mut().expect("connected").create_directory(path);
        match r {
            Ok(()) => Ok(()),
            Err(e) if e.is_not_found() && create_multi_directory => {
                self.create_multi_directory(path)?;
                Ok(())
            }
            Err(e) if e.is_permission_denied() => Ok(()),
            Err(e) => Err(UploadError::Exception(e.to_string())),
        }
    }

    /// `CreateMultiDirectory`: the levels that don't exist.
    pub fn create_multi_directory(&mut self, path: &str) -> UResult<Vec<String>> {
        let mut created = Vec::new();
        for directory in url_helpers::get_paths(path) {
            if !self.directory_exists(&directory)? {
                self.create_directory(&directory, false)?;
                created.push(directory);
            }
        }
        Ok(created)
    }

    /// `DirectoryExists`.
    pub fn directory_exists(&mut self, path: &str) -> UResult<bool> {
        if !self.connect()? {
            return Ok(false);
        }
        self.session.as_mut().expect("connected").exists(path).map_err(|e| UploadError::Exception(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsa_keys_sign() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        for (name, pass) in [("sftp_test_rsa_pem", None), ("sftp_test_rsa", Some("made-up-passphrase")), ("sftp_test_ed25519", None)] {
            let key = load_private_key(&std::fs::read_to_string(dir.join(name)).unwrap(), pass).unwrap_or_else(|e| panic!("{name}: {e}"));
            if matches!(key.algorithm(), ssh_key::Algorithm::Rsa { .. }) {
                sign_rsa_sha1(&key, b"data").unwrap_or_else(|e| panic!("{name} sha1: {e}"));
            }
            if matches!(key.algorithm(), ssh_key::Algorithm::Rsa { .. }) {
                sign_rsa_sha2(&key, "rsa-sha2-512", b"data").unwrap_or_else(|e| panic!("{name} sha512: {e}"));
            } else {
                ssh::sign_user_auth(&key, b"data").unwrap_or_else(|e| panic!("{name} default: {e}"));
            }
        }
    }
}
