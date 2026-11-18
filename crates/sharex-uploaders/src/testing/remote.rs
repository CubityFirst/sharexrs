//! Runs a harness "remote" case (the remote storage browser, step by step)
//! the way `tools/uploaders-harness` records ShareX's: the provider's
//! names and capabilities, then each step's value or exception text. Used
//! by the harness and by `tests/servers.rs`.

use base64::Engine;
use serde_json::{Value, json};

use crate::destinations::ftp::FTPAccount;
use crate::remote_storage::{
    AmazonS3RemoteStorageProvider, CancelToken, FtpRemoteStorageProvider, RemoteStorageError, RemoteStorageItem, RemoteStorageProvider,
    SftpRemoteStorageProvider,
};

/// `DateTimeOffset.ToString("o")`.
pub fn round_trip(m: &chrono::DateTime<chrono::FixedOffset>) -> String {
    format!("{}.{:07}{}", m.format("%Y-%m-%dT%H:%M:%S"), m.timestamp_subsec_nanos() / 100, m.format("%:z"))
}

fn item(v: &Value) -> RemoteStorageItem {
    RemoteStorageItem::new(v["name"].as_str().unwrap_or(""), v["path"].as_str().unwrap_or(""), v["is_folder"].as_bool().unwrap_or(false), v["size"].as_i64())
}

fn item_json(i: &RemoteStorageItem) -> Value {
    json!({"name": i.name, "path": i.path, "is_folder": i.is_folder, "size": i.size, "modified": i.modified.as_ref().map(round_trip)})
}

fn error(e: &dyn std::fmt::Display) -> Value {
    json!({"exception": e.to_string()})
}

/// Runs `case` (its `provider`, `account` or `s3`, `allow_upload`,
/// `steps`). `account` is the FTP account as the case gives it, after the
/// caller's changes (ports, key paths).
pub fn run_remote_case(case: &Value, account: Option<FTPAccount>) -> Value {
    let mut out = json!({});
    let allow_upload = case["allow_upload"].as_bool().unwrap_or(true);
    let mut provider: Box<dyn RemoteStorageProvider> = match case["provider"].as_str().unwrap_or("") {
        "ftp" => match FtpRemoteStorageProvider::new(account.expect("FTP account"), allow_upload) {
            Ok(p) => Box::new(p),
            Err(e) => {
                out["steps"] = json!([]);
                out["create"] = error(&e);
                return out;
            }
        },
        "sftp" => Box::new(SftpRemoteStorageProvider::new(account.expect("SFTP account"), allow_upload)),
        "s3" => Box::new(AmazonS3RemoteStorageProvider::new(serde_json::from_value(case["s3"].clone()).expect("s3 settings"), allow_upload)),
        other => panic!("unknown provider {other}"),
    };
    out["display_name"] = json!(provider.display_name());
    out["root_display_name"] = json!(provider.root_display_name());
    out["root_path"] = json!(provider.root_path());
    out["capabilities"] = json!(provider.capabilities().0);
    let cancel = CancelToken::new();
    let mut steps = Vec::new();
    for step in case["steps"].as_array().into_iter().flatten() {
        let s = |k: &str| step[k].as_str().unwrap_or("").to_owned();
        let r: Result<Value, RemoteStorageError> = match step["op"].as_str().unwrap_or("") {
            "display_path" => Ok(json!(provider.get_display_path(&s("path")))),
            "parent_path" => Ok(json!(provider.get_parent_path(&s("path")))),
            "url" => Ok(json!(provider.get_url(&item(&step["item"])))),
            "list" => provider.get_items(&s("path"), &cancel).map(|items| Value::Array(items.iter().map(item_json).collect())),
            "has_children" => provider.has_children(&item(&step["item"]), &cancel).map(|b| json!(b)),
            "download" => {
                let mut bytes = Vec::new();
                provider.download(&item(&step["item"]), &mut bytes, &cancel).map(|()| json!(base64::engine::general_purpose::STANDARD.encode(bytes)))
            }
            "upload" => {
                let bytes = base64::engine::general_purpose::STANDARD.decode(s("content_base64")).expect("base64");
                provider.upload(&s("dir"), &s("name"), &mut std::io::Cursor::new(bytes), &cancel).map(|()| json!("ok"))
            }
            "create_folder" => provider.create_folder(&s("dir"), &s("name"), &cancel).map(|()| json!("ok")),
            "rename" => provider.rename(&item(&step["item"]), &s("name"), &cancel).map(|()| json!("ok")),
            "delete" => provider.delete(&item(&step["item"]), &cancel).map(|()| json!("ok")),
            op => panic!("unknown op {op}"),
        };
        steps.push(r.unwrap_or_else(|e| error(&e)));
    }
    provider.dispose();
    out["steps"] = Value::Array(steps);
    out
}
