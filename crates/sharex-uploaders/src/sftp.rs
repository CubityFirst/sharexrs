//! SFTP version 3 (draft-ietf-secsh-filexfer-02) over [`crate::ssh`]: the
//! requests SSH.NET's `SftpClient` makes for ShareX.

use crate::ssh::{Channel, Reader, SshError, SshResult, Writer};

pub const FXP_INIT: u8 = 1;
pub const FXP_VERSION: u8 = 2;
pub const FXP_OPEN: u8 = 3;
pub const FXP_CLOSE: u8 = 4;
pub const FXP_READ: u8 = 5;
pub const FXP_WRITE: u8 = 6;
pub const FXP_LSTAT: u8 = 7;
pub const FXP_FSTAT: u8 = 8;
pub const FXP_SETSTAT: u8 = 9;
pub const FXP_FSETSTAT: u8 = 10;
pub const FXP_OPENDIR: u8 = 11;
pub const FXP_READDIR: u8 = 12;
pub const FXP_REMOVE: u8 = 13;
pub const FXP_MKDIR: u8 = 14;
pub const FXP_RMDIR: u8 = 15;
pub const FXP_REALPATH: u8 = 16;
pub const FXP_STAT: u8 = 17;
pub const FXP_RENAME: u8 = 18;
pub const FXP_STATUS: u8 = 101;
pub const FXP_HANDLE: u8 = 102;
pub const FXP_DATA: u8 = 103;
pub const FXP_NAME: u8 = 104;
pub const FXP_ATTRS: u8 = 105;

pub const FX_OK: u32 = 0;
pub const FX_EOF: u32 = 1;
pub const FX_NO_SUCH_FILE: u32 = 2;
pub const FX_PERMISSION_DENIED: u32 = 3;
pub const FX_FAILURE: u32 = 4;

pub const PF_READ: u32 = 0x01;
pub const PF_WRITE: u32 = 0x02;
pub const PF_APPEND: u32 = 0x04;
pub const PF_CREAT: u32 = 0x08;
pub const PF_TRUNC: u32 = 0x10;
pub const PF_EXCL: u32 = 0x20;

/// What an SFTP request can end with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SftpError {
    /// A status reply: `SftpPathNotFoundException` (2),
    /// `SftpPermissionDeniedException` (3), `SshException` (others).
    Status(u32, String),
    /// The connection.
    Ssh(String),
}

impl SftpError {
    pub fn is_not_found(&self) -> bool {
        matches!(self, SftpError::Status(FX_NO_SUCH_FILE, _))
    }
    pub fn is_permission_denied(&self) -> bool {
        matches!(self, SftpError::Status(FX_PERMISSION_DENIED, _))
    }
}

impl std::fmt::Display for SftpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SftpError::Status(_, m) | SftpError::Ssh(m) => f.write_str(m),
        }
    }
}

impl From<SshError> for SftpError {
    fn from(e: SshError) -> Self {
        SftpError::Ssh(e.0)
    }
}

pub type SftpResult<T> = Result<T, SftpError>;

/// File attributes (the fields SSH.NET reads).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attrs {
    pub size: Option<u64>,
    pub permissions: Option<u32>,
    pub mtime: Option<u32>,
}

impl Attrs {
    pub fn is_dir(&self) -> bool {
        self.permissions.is_some_and(|p| p & 0o170000 == 0o040000)
    }
    pub fn is_regular(&self) -> bool {
        self.permissions.is_some_and(|p| p & 0o170000 == 0o100000)
    }
    pub fn is_symlink(&self) -> bool {
        self.permissions.is_some_and(|p| p & 0o170000 == 0o120000)
    }

    pub fn read(r: &mut Reader) -> SshResult<Self> {
        let flags = r.u32()?;
        let mut a = Attrs::default();
        if flags & 0x1 != 0 {
            a.size = Some(r.u64()?);
        }
        if flags & 0x2 != 0 {
            r.u32()?;
            r.u32()?;
        }
        if flags & 0x4 != 0 {
            a.permissions = Some(r.u32()?);
        }
        if flags & 0x8 != 0 {
            r.u32()?;
            a.mtime = Some(r.u32()?);
        }
        if flags & 0x8000_0000 != 0 {
            let n = r.u32()?;
            for _ in 0..n {
                r.bytes()?;
                r.bytes()?;
            }
        }
        Ok(a)
    }

    pub fn write(&self, w: Writer) -> Writer {
        let mut flags = 0;
        if self.size.is_some() {
            flags |= 0x1;
        }
        if self.permissions.is_some() {
            flags |= 0x4;
        }
        if self.mtime.is_some() {
            flags |= 0x8;
        }
        let mut w = w.u32(flags);
        if let Some(s) = self.size {
            w = w.u64(s);
        }
        if let Some(p) = self.permissions {
            w = w.u32(p);
        }
        if let Some(m) = self.mtime {
            w = w.u32(m).u32(m);
        }
        w
    }
}

/// A directory entry (`SFTPFileInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub attrs: Attrs,
}

/// An SFTP session.
pub struct SftpClient {
    channel: Channel,
    next_id: u32,
    pub version: u32,
}

impl SftpClient {
    /// `SSH_FXP_INIT` (version 3).
    pub fn start(channel: Channel) -> SftpResult<Self> {
        let mut c = Self { channel, next_id: 0, version: 0 };
        c.send_packet(&Writer::new(FXP_INIT).u32(3).0)?;
        let p = c.read_packet()?;
        if p.first() != Some(&FXP_VERSION) {
            return Err(SftpError::Ssh("Expected the SFTP version.".into()));
        }
        c.version = Reader::new(&p[1..]).u32()?;
        Ok(c)
    }

    fn send_packet(&mut self, payload: &[u8]) -> SftpResult<()> {
        let mut data = (payload.len() as u32).to_be_bytes().to_vec();
        data.extend_from_slice(payload);
        self.channel.write(&data)?;
        Ok(())
    }

    fn read_packet(&mut self) -> SftpResult<Vec<u8>> {
        let len = u32::from_be_bytes(self.channel.read_exact(4)?.try_into().expect("4 bytes")) as usize;
        Ok(self.channel.read_exact(len)?)
    }

    fn id(&mut self) -> u32 {
        self.next_id = self.next_id.wrapping_add(1);
        self.next_id
    }

    /// Sends a request; its id.
    pub fn send_request(&mut self, kind: u8, body: Writer) -> SftpResult<u32> {
        let id = self.id();
        let mut payload = Writer::new(kind).u32(id).0;
        payload.extend_from_slice(&body.0);
        self.send_packet(&payload)?;
        Ok(id)
    }

    /// Reads the reply to `id` (replies come in order).
    pub fn read_reply(&mut self, id: u32) -> SftpResult<Vec<u8>> {
        let p = self.read_packet()?;
        let reply_id = Reader::new(&p[1..]).u32()?;
        if reply_id != id {
            return Err(SftpError::Ssh(format!("Unexpected SFTP reply {reply_id} (expected {id}).")));
        }
        Ok(p)
    }

    fn status(p: &[u8]) -> SftpResult<()> {
        let mut r = Reader::new(&p[5..]);
        let code = r.u32()?;
        let message = r.string().unwrap_or_default();
        if code == FX_OK { Ok(()) } else { Err(SftpError::Status(code, if message.is_empty() { status_text(code).into() } else { message })) }
    }

    fn expect(p: &[u8], kind: u8) -> SftpResult<Reader<'_>> {
        match p.first().copied() {
            Some(k) if k == kind => Ok(Reader::new(&p[5..])),
            Some(FXP_STATUS) => {
                Self::status(p)?;
                Err(SftpError::Ssh("Unexpected SFTP status.".into()))
            }
            _ => Err(SftpError::Ssh("Unexpected SFTP reply.".into())),
        }
    }

    fn request(&mut self, kind: u8, body: Writer) -> SftpResult<Vec<u8>> {
        let id = self.send_request(kind, body)?;
        self.read_reply(id)
    }

    /// `REALPATH`.
    pub fn realpath(&mut self, path: &str) -> SftpResult<String> {
        let p = self.request(FXP_REALPATH, Writer::default().string(path))?;
        let mut r = Self::expect(&p, FXP_NAME)?;
        r.u32()?;
        Ok(r.string()?)
    }

    /// `STAT`.
    pub fn stat(&mut self, path: &str) -> SftpResult<Attrs> {
        let p = self.request(FXP_STAT, Writer::default().string(path))?;
        Ok(Attrs::read(&mut Self::expect(&p, FXP_ATTRS)?)?)
    }

    /// `LSTAT`.
    pub fn lstat(&mut self, path: &str) -> SftpResult<Attrs> {
        let p = self.request(FXP_LSTAT, Writer::default().string(path))?;
        Ok(Attrs::read(&mut Self::expect(&p, FXP_ATTRS)?)?)
    }

    /// `OPEN` (no attributes): the handle.
    pub fn open(&mut self, path: &str, flags: u32) -> SftpResult<Vec<u8>> {
        let p = self.request(FXP_OPEN, Writer::default().string(path).u32(flags).u32(0))?;
        Ok(Self::expect(&p, FXP_HANDLE)?.bytes()?.to_vec())
    }

    /// `WRITE` without waiting for its status (see [`Self::read_status`]).
    pub fn send_write(&mut self, handle: &[u8], offset: u64, data: &[u8]) -> SftpResult<u32> {
        self.send_request(FXP_WRITE, Writer::default().bytes(handle).u64(offset).bytes(data))
    }

    /// The status reply to `id`.
    pub fn read_status(&mut self, id: u32) -> SftpResult<()> {
        let p = self.read_reply(id)?;
        if p.first() != Some(&FXP_STATUS) {
            return Err(SftpError::Ssh("Expected an SFTP status.".into()));
        }
        Self::status(&p)
    }

    /// `READ`: the data, or `None` at the end of the file.
    pub fn read(&mut self, handle: &[u8], offset: u64, len: u32) -> SftpResult<Option<Vec<u8>>> {
        let p = self.request(FXP_READ, Writer::default().bytes(handle).u64(offset).u32(len))?;
        if p.first() == Some(&FXP_STATUS) {
            return match Self::status(&p) {
                Err(SftpError::Status(FX_EOF, _)) | Ok(()) => Ok(None),
                Err(e) => Err(e),
            };
        }
        Ok(Some(Self::expect(&p, FXP_DATA)?.bytes()?.to_vec()))
    }

    /// `CLOSE`.
    pub fn close_handle(&mut self, handle: &[u8]) -> SftpResult<()> {
        let id = self.send_request(FXP_CLOSE, Writer::default().bytes(handle))?;
        self.read_status(id)
    }

    /// `MKDIR` (no attributes).
    pub fn mkdir(&mut self, path: &str) -> SftpResult<()> {
        let id = self.send_request(FXP_MKDIR, Writer::default().string(path).u32(0))?;
        self.read_status(id)
    }

    /// `REMOVE`.
    pub fn remove(&mut self, path: &str) -> SftpResult<()> {
        let id = self.send_request(FXP_REMOVE, Writer::default().string(path))?;
        self.read_status(id)
    }

    /// `RMDIR`.
    pub fn rmdir(&mut self, path: &str) -> SftpResult<()> {
        let id = self.send_request(FXP_RMDIR, Writer::default().string(path))?;
        self.read_status(id)
    }

    /// `RENAME`.
    pub fn rename(&mut self, from: &str, to: &str) -> SftpResult<()> {
        let id = self.send_request(FXP_RENAME, Writer::default().string(from).string(to))?;
        self.read_status(id)
    }

    /// `OPENDIR`: the handle.
    pub fn opendir(&mut self, path: &str) -> SftpResult<Vec<u8>> {
        let p = self.request(FXP_OPENDIR, Writer::default().string(path))?;
        Ok(Self::expect(&p, FXP_HANDLE)?.bytes()?.to_vec())
    }

    /// `READDIR`: a batch of entries, `None` at the end.
    pub fn readdir(&mut self, handle: &[u8]) -> SftpResult<Option<Vec<DirEntry>>> {
        let p = self.request(FXP_READDIR, Writer::default().bytes(handle))?;
        if p.first() == Some(&FXP_STATUS) {
            return match Self::status(&p) {
                Err(SftpError::Status(FX_EOF, _)) | Ok(()) => Ok(None),
                Err(e) => Err(e),
            };
        }
        let mut r = Self::expect(&p, FXP_NAME)?;
        let n = r.u32()?;
        let mut entries = Vec::new();
        for _ in 0..n {
            let name = r.string()?;
            r.string()?;
            entries.push(DirEntry { name, attrs: Attrs::read(&mut r)? });
        }
        Ok(Some(entries))
    }

    /// `FSTAT`.
    pub fn fstat(&mut self, handle: &[u8]) -> SftpResult<Attrs> {
        let p = self.request(FXP_FSTAT, Writer::default().bytes(handle))?;
        Ok(Attrs::read(&mut Self::expect(&p, FXP_ATTRS)?)?)
    }

    /// `OPENDIR` + `READDIR` until EOF + `CLOSE`.
    pub fn list_directory(&mut self, path: &str) -> SftpResult<Vec<DirEntry>> {
        let p = self.request(FXP_OPENDIR, Writer::default().string(path))?;
        let handle = Self::expect(&p, FXP_HANDLE)?.bytes()?.to_vec();
        let mut entries = Vec::new();
        loop {
            let p = self.request(FXP_READDIR, Writer::default().bytes(&handle))?;
            if p.first() == Some(&FXP_STATUS) {
                match Self::status(&p) {
                    Err(SftpError::Status(FX_EOF, _)) | Ok(()) => break,
                    Err(e) => return Err(e),
                }
            }
            let mut r = Self::expect(&p, FXP_NAME)?;
            let n = r.u32()?;
            for _ in 0..n {
                let name = r.string()?;
                r.string()?;
                entries.push(DirEntry { name, attrs: Attrs::read(&mut r)? });
            }
        }
        self.close_handle(&handle)?;
        Ok(entries)
    }

    /// Ends the session and the connection.
    pub fn close(self) {
        self.channel.close();
    }
}

/// The default message of a status code (SSH.NET's when the server sends none).
pub fn status_text(code: u32) -> &'static str {
    match code {
        FX_OK => "Success",
        FX_EOF => "End of file",
        FX_NO_SUCH_FILE => "No such file",
        FX_PERMISSION_DENIED => "Permission denied",
        FX_FAILURE => "Failure",
        5 => "Bad message",
        6 => "No connection",
        7 => "Connection lost",
        8 => "Operation unsupported",
        _ => "Unknown error",
    }
}
