//! In-process mock servers for tests and the uploaders harness: FTP/FTPS
//! ([`ftp`]) and SFTP ([`sftp`]). They bind 127.0.0.1 only.

pub mod ftp;
pub mod sftp;
