//! `ShareX.UploadersLib`: where ShareX sends what it captures.
//!
//! - [`uploader`] — the `Uploader` base (request helpers, errors, progress,
//!   stopping) and the uploader kinds (`FileUploader`, `TextUploader`,
//!   `URLShortener`, `URLSharer`),
//! - [`http`] — the HTTP layer (requests as .NET's `HttpClient` sends them),
//! - [`custom`] — custom uploaders (`.sxcu`), ShareX's most used destination,
//! - [`destinations`] — the built-in services ported so far,
//! - [`factory`] — `UploaderFactory`, which destination enum value makes
//!   which uploader, and whether its settings are complete,
//! - [`config`] — `UploadersConfig.json`.
//!
//! See `docs/UPLOADERS.md` for the map of ShareX's uploaders and what's
//! ported.

pub mod api_keys;
pub mod config;
pub mod custom;
pub mod destinations;
pub mod dpapi;
pub mod enums;
pub mod factory;
pub mod http;
pub mod json;
pub mod oauth;
pub mod oauth1;
pub mod oauth2;
pub mod remote_storage;
pub mod result;
pub mod sftp;
pub mod signin;
pub mod ssh;
#[doc(hidden)]
pub mod testing;
pub mod uploader;
pub mod url_helpers;
pub mod validator;

pub use config::UploadersConfig;
pub use enums::*;
pub use result::{UploadResult, UploaderErrorManager};
pub use uploader::{FileUploader, GenericUploader, TextUploader, UploadError, Uploader, UrlSharer, UrlShortener};

/// `Environment.NewLine`.
pub const NEW_LINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };
