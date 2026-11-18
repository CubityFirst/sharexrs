//! `FileUploaders/SharedFolderUploader.cs` and `LocalhostAccount.cs`: the
//! file copied into a local or network folder, the URL built from the
//! account's HTTP home path.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sharex_helpers::NameParserType;
use sharex_settings::paths::expand_folder_variables;

use super::cloud_storage::name_parse;
use super::uploader_base;
use crate::dpapi::Secret;
use crate::enums::BrowserProtocol;
use crate::http::SharedStream;
use crate::result::UploadResult;
use crate::uploader::{CoreResult, UploadError, Uploader, WritesBackConfig};
use crate::url_helpers;

fn default_port() -> i32 {
    80
}

fn yes() -> bool {
    true
}

fn default_remote_protocol() -> BrowserProtocol {
    BrowserProtocol::file
}

fn new_account_name() -> Option<String> {
    Some("New account".into())
}

fn empty() -> Option<String> {
    Some(String::new())
}

/// `LocalhostAccount` (an item of `UploadersConfig.LocalhostAccountList`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct LocalhostAccount {
    #[serde(default = "new_account_name")]
    pub Name: Option<String>,
    #[serde(default = "empty")]
    pub LocalhostRoot: Option<String>,
    #[serde(default = "default_port")]
    pub Port: i32,
    #[serde(default)]
    pub UserName: Option<String>,
    #[serde(default)]
    pub Password: Secret,
    #[serde(default = "empty")]
    pub SubFolderPath: Option<String>,
    #[serde(default = "empty")]
    pub HttpHomePath: Option<String>,
    #[serde(default = "yes")]
    pub HttpHomePathAutoAddSubFolderPath: bool,
    #[serde(default)]
    pub HttpHomePathNoExtension: bool,
    #[serde(default = "default_remote_protocol")]
    pub RemoteProtocol: BrowserProtocol,
}

impl Default for LocalhostAccount {
    fn default() -> Self {
        serde_json::from_value(Value::Object(Default::default())).expect("defaults")
    }
}

/// `BrowserProtocol`'s `[Description]`.
pub fn browser_protocol_prefix(p: &BrowserProtocol) -> &'static str {
    match p {
        BrowserProtocol::http => "http://",
        BrowserProtocol::https => "https://",
        BrowserProtocol::ftp => "ftp://",
        BrowserProtocol::ftps => "ftps://",
        BrowserProtocol::sftp => "sftp://",
        _ => "file://",
    }
}

/// `new Uri(path).AbsoluteUri` of a local or UNC path.
pub fn local_file_uri(path: &str) -> String {
    let s = path.replace('\\', "/");
    let (prefix, rest) = if let Some(unc) = s.strip_prefix("//") {
        ("file://", unc.to_owned())
    } else if s.starts_with('/') {
        ("file://", s.clone())
    } else {
        ("file:///", s.clone())
    };
    let mut out = String::from(prefix);
    for c in rest.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '.' | '_' | '~' | '/' | ':' | '!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';' | '=' | '@' => {
                out.push(c)
            }
            c => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
        }
    }
    out
}

/// `Path.Combine(a, b)`: `b` if it's rooted.
fn path_combine(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_owned();
    }
    if b.is_empty() {
        return a.to_owned();
    }
    let rooted = b.starts_with('/') || b.starts_with('\\') || (b.len() >= 2 && b.as_bytes()[1] == b':');
    if rooted {
        return b.to_owned();
    }
    let sep = std::path::MAIN_SEPARATOR;
    if a.ends_with('/') || a.ends_with('\\') || a.ends_with(':') { format!("{a}{b}") } else { format!("{a}{sep}{b}") }
}

impl LocalhostAccount {
    /// Reads `UploadersConfig.LocalhostAccountList`.
    pub fn list_from_value(v: &Value) -> Option<Vec<LocalhostAccount>> {
        match v {
            Value::Array(items) => Some(items.iter().map(|i| serde_json::from_value(i.clone()).unwrap_or_default()).collect()),
            _ => None,
        }
    }

    fn root(&self) -> String {
        expand_folder_variables(self.LocalhostRoot.as_deref().unwrap_or(""))
    }

    /// `LocalUri`.
    pub fn local_uri(&self) -> String {
        if self.LocalhostRoot.as_deref().unwrap_or("").is_empty() { String::new() } else { local_file_uri(&self.root()) }
    }

    /// `GetSubFolderPath`.
    pub fn get_sub_folder_path(&self) -> String {
        name_parse(NameParserType::Url, &self.SubFolderPath.as_deref().unwrap_or("").replace("%host", &self.root()))
    }

    /// `GetHttpHomePath` (it rewrites `HttpHomePath` as ShareX's does: a
    /// leading `@` turns off adding the sub folder, the prefix goes).
    pub fn get_http_home_path(&mut self) -> String {
        let mut home = self.HttpHomePath.clone().unwrap_or_default();
        if let Some(rest) = home.strip_prefix('@') {
            home = rest.to_owned();
            self.HttpHomePathAutoAddSubFolderPath = false;
        }
        home = url_helpers::remove_prefixes(&home);
        self.HttpHomePath = Some(home.clone());
        name_parse(NameParserType::Url, &home.replace("%host", &self.root()))
    }

    /// `GetUriPath(fileName)`.
    pub fn get_uri_path(&mut self, file_name: &str) -> String {
        if self.LocalhostRoot.as_deref().unwrap_or("").is_empty() {
            return String::new();
        }
        let mut file_name = file_name.to_owned();
        if self.HttpHomePathNoExtension {
            file_name = file_name_without_extension(&file_name);
        }
        let file_name = url_helpers::url_encode(&file_name, false);
        let sub_folder_path = url_helpers::url_encode(&self.get_sub_folder_path(), true);
        let http_home_path = self.get_http_home_path();
        let mut path = if http_home_path.is_empty() {
            self.RemoteProtocol = BrowserProtocol::file;
            self.local_uri().replace("file://", "")
        } else {
            url_helpers::url_encode(&http_home_path, true)
        };
        if self.Port != 80 {
            path = format!("{path}:{}", self.Port);
        }
        if self.HttpHomePathAutoAddSubFolderPath {
            path = url_helpers::combine_url(&path, &sub_folder_path);
        }
        path = url_helpers::combine_url(&path, &file_name);
        let protocol = browser_protocol_prefix(&self.RemoteProtocol);
        if !path.starts_with(protocol) {
            path = format!("{protocol}{path}");
        }
        path
    }

    /// `GetLocalhostPath(fileName)`.
    pub fn get_localhost_path(&self, file_name: &str) -> String {
        if self.LocalhostRoot.as_deref().unwrap_or("").is_empty() {
            return String::new();
        }
        path_combine(&path_combine(&self.root(), &self.get_sub_folder_path()), file_name)
    }

    /// `GetLocalhostUri(fileName)` (`PreviewLocalPath`).
    pub fn get_localhost_uri(&self, file_name: &str) -> String {
        let address = self.local_uri();
        if address.is_empty() {
            return String::new();
        }
        url_helpers::combine_urls(&[&address, &self.get_sub_folder_path(), file_name])
    }
}

/// `Path.GetFileNameWithoutExtension`.
pub(crate) fn file_name_without_extension(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match base.rfind('.') {
        Some(i) => base[..i].to_owned(),
        None => base.to_owned(),
    }
}

/// `SharedFolderUploader`.
pub struct SharedFolderUploader {
    pub base: Uploader,
    pub account: LocalhostAccount,
    /// The account's index in `LocalhostAccountList`, to write back the
    /// changes `GetHttpHomePath` makes.
    pub index: usize,
}

uploader_base!(SharedFolderUploader, FileUploader);

impl WritesBackConfig for SharedFolderUploader {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        if let Some(item) = config.LocalhostAccountList.as_array_mut().and_then(|a| a.get_mut(self.index))
            && let Some(o) = item.as_object_mut()
        {
            o.insert("HttpHomePath".into(), crate::json::str_or_null(self.account.HttpHomePath.as_deref()));
            o.insert("HttpHomePathAutoAddSubFolderPath".into(), Value::Bool(self.account.HttpHomePathAutoAddSubFolderPath));
            if let Ok(p) = serde_json::to_value(&self.account.RemoteProtocol) {
                o.insert("RemoteProtocol".into(), p);
            }
        }
    }
}

impl SharedFolderUploader {
    pub fn new(account: LocalhostAccount, index: usize) -> Self {
        Self { base: Uploader::new(), account, index }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if self.base.stop_requested() {
            return Err(UploadError::Cancelled);
        }
        let mut result = UploadResult::default();
        let file_path = self.account.get_localhost_path(file_name);
        let io = |e: std::io::Error| UploadError::Exception(e.to_string());
        if let Some(parent) = Path::new(&file_path).parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        let mut file = std::fs::File::create(PathBuf::from(&file_path)).map_err(io)?;
        if self.base.transfer_data(data, &mut file, 0, None)? {
            let name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
            result.url = Some(self.account.get_uri_path(name));
        }
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_paths() {
        let mut a = LocalhostAccount {
            LocalhostRoot: Some(if cfg!(windows) { r"C:\www".into() } else { "/srv/www".into() }),
            SubFolderPath: Some("shots".into()),
            HttpHomePath: Some("@https://example.com/img".into()),
            ..Default::default()
        };
        assert_eq!(a.get_uri_path("a b.png"), "file://example.com/img/a%20b.png");
        assert!(!a.HttpHomePathAutoAddSubFolderPath);
        assert_eq!(a.HttpHomePath.as_deref(), Some("example.com/img"));
        let mut b = LocalhostAccount { LocalhostRoot: a.LocalhostRoot.clone(), SubFolderPath: Some("shots".into()), ..Default::default() };
        let expected = if cfg!(windows) { "file:///C:/www/shots/x.png" } else { "file:///srv/www/shots/x.png" };
        assert_eq!(b.get_uri_path("x.png"), expected);
        assert_eq!(url_helpers::get_paths("a/b/c"), vec!["a", "a/b", "a/b/c"]);
        assert_eq!(url_helpers::get_paths("/a/b/"), vec!["/a", "/a/b"]);
    }
}
