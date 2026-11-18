//! In-process mock servers for tests and the uploaders harness: FTP/FTPS
//! ([`ftp`]), SFTP ([`sftp`]) and SMTP ([`smtp`]). They bind 127.0.0.1 only.

pub mod ftp;
pub mod sftp;
pub mod smtp;
