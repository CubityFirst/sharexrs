//! The ShareXrs side of `tools/uploaders-harness`: runs every case in
//! `cases.json` through ShareXrs's uploaders against the local mock server
//! and writes the same results file the .NET harness writes for ShareX.
//!
//! ```text
//! cargo run -p sharex-uploaders --example uploaders_harness -- <cases.json> <results.json> [port]
//! ```
//!
//! Every request is rewritten to `http://127.0.0.1:<port>/__/<scheme>/<host><path>`
//! just before it's sent: nothing leaves the machine.

use std::io::Cursor;
use std::sync::Arc;

use base64::Engine;
use serde_json::{Value, json};
use sharex_uploaders::custom::{CustomFileUploader, CustomTextUploader, CustomUploaderInput, CustomUploaderItem, CustomUrlSharer, CustomUrlShortener};
use sharex_uploaders::factory::{self, Ported, TaskReferenceHelper};
use sharex_uploaders::http::{self, UrlRewriter};
use sharex_uploaders::uploader::{self, GenericUploader, UrlSharer, UrlShortener};
use sharex_uploaders::{UploadResult, UploadersConfig, UploaderErrorManager};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cases_path = &args[1];
    let results_path = &args[2];
    let port: u16 = args.get(3).and_then(|p| p.parse().ok()).unwrap_or(18765);

    // The mock SMTP server's certificate is self-signed (the .NET harness
    // accepts it too).
    sharex_uploaders::destinations::email::set_accept_invalid_certificates_for_tests(true);
    let prefix = format!("http://127.0.0.1:{port}/__/");
    let back_prefix = prefix.clone();
    http::set_url_rewriter(Some(UrlRewriter {
        forward: Arc::new(move |url: &str| {
            let (scheme, rest) = url.split_once("://").expect("absolute URL");
            format!("http://127.0.0.1:{port}/__/{scheme}/{rest}")
        }),
        back: Arc::new(move |url: &str| match url.strip_prefix(&back_prefix) {
            Some(rest) => {
                let (scheme, rest) = rest.split_once('/').expect("scheme");
                format!("{scheme}://{rest}")
            }
            None => url.to_owned(),
        }),
    }));

    let cases: Value = serde_json::from_str(&std::fs::read_to_string(cases_path).expect("cases")).expect("cases JSON");
    let only = std::env::var("HARNESS_ONLY").ok().filter(|s| !s.is_empty());
    let mut results = Vec::new();
    for case in cases["cases"].as_array().expect("cases array") {
        let id = case["id"].as_str().unwrap_or("").to_owned();
        if only.as_deref().is_some_and(|o| o != id) {
            continue;
        }
        let mut out = run_case(case, port);
        out["id"] = json!(id);
        println!("{id}: {}", if out.get("exception").is_some_and(|e| !e.is_null()) { "exception" } else { "ok" });
        results.push(out);
    }
    std::fs::write(results_path, serde_json::to_string_pretty(&results).expect("results")).expect("write results");
}

fn control(port: u16) -> ureq::Agent {
    let _ = port;
    ureq::Agent::config_builder().http_status_as_error(false).build().into()
}

/// The FTP/SFTP/SMTP mocks' control port (`sharex_uploaders_mock_servers`).
fn servers_port() -> u16 {
    std::env::var("MOCK_SERVERS_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(18820)
}

fn reset_servers(spec: &Value) {
    control(0).post(&format!("http://127.0.0.1:{}/reset", servers_port())).send(spec.to_string()).expect("mock servers reset");
}

fn servers_log() -> Value {
    let mut resp = control(0).get(&format!("http://127.0.0.1:{}/log", servers_port())).call().expect("mock servers log");
    serde_json::from_str(&resp.body_mut().read_to_string().expect("log body")).expect("log JSON")
}

fn reset(port: u16, responses: &Value) {
    let body = json!({ "responses": responses.clone() }).to_string();
    control(port).post(&format!("http://127.0.0.1:{port}/__mock/reset")).send(body).expect("mock reset");
}

fn log(port: u16) -> Value {
    let mut resp = control(port).get(&format!("http://127.0.0.1:{port}/__mock/log")).call().expect("mock log");
    serde_json::from_str(&resp.body_mut().read_to_string().expect("log body")).expect("log JSON")
}

fn errors_json(errors: &UploaderErrorManager) -> Value {
    Value::Array(errors.errors.iter().map(|e| json!(e.text)).collect())
}

fn result_json(r: &Option<UploadResult>) -> Value {
    let Some(r) = r else { return Value::Null };
    json!({
        "url": r.url,
        "thumbnail_url": r.thumbnail_url,
        "deletion_url": r.deletion_url,
        "shortened_url": r.shortened_url,
        "is_success": r.is_success(),
        "is_error": r.is_error(),
        "is_url_expected": r.is_url_expected,
        "response": r.response,
        "errors": errors_json(&r.errors),
        "response_url": r.response_info.as_ref().and_then(|i| i.response_url.clone()),
        "status_code": r.response_info.as_ref().map(|i| i.status_code),
    })
}

enum Runner {
    Generic(GenericUploader),
    Shortener(Box<dyn UrlShortener>),
    Sharer(Box<dyn UrlSharer>),
}

/// An "oauth" case: the sign-in steps through the service's client.
fn run_oauth(case: &Value, out: &mut Value) {
    use sharex_uploaders::destinations::oauth_services::{Flickr, Photobucket, PhotobucketAccountInfo};
    use sharex_uploaders::oauth::{OAuth2Info, OAuthInfo};
    use sharex_uploaders::oauth2::{OAuth1Client, OAuth2ChallengeMethod, OAuth2Client, OAuth2Loopback, OAuth2ProofKey};
    use sharex_uploaders::signin::SignInService;

    let service = case["service"].as_str().unwrap_or("");
    let mut steps = Vec::new();
    let token_json = |info: &OAuth2Info| match &info.Token {
        None => json!({"access_token": null, "refresh_token": null, "expires_in": null, "token_type": null, "scope": null,
                       "has_expire_date": null, "expired": null}),
        Some(t) => json!({
            "access_token": t.access_token.get(),
            "refresh_token": t.refresh_token.get(),
            "expires_in": t.expires_in,
            "token_type": t.token_type,
            "scope": t.scope,
            "has_expire_date": t.ExpireDate.as_str().is_some_and(|d| !d.starts_with("0001-01-01T00:00:00")),
            "expired": t.is_expired(),
        }),
    };
    let oauth1_json = |info: &OAuthInfo| {
        json!({"AuthToken": info.AuthToken, "AuthSecret": info.AuthSecret.get(), "AuthVerifier": info.AuthVerifier.get(),
               "UserToken": info.UserToken, "UserSecret": info.UserSecret.get()})
    };
    let errors;
    let mut exception: Option<String> = None;
    if service == "mega" {
        use sharex_uploaders::destinations::mega::Mega;
        let mut m = None;
        for step in case["steps"].as_array().into_iter().flatten() {
            let mut mega = Mega::new(step["email"].as_str().unwrap_or(""), step["password"].as_str().unwrap_or(""));
            match mega.login(None) {
                Ok(s) => steps.push(json!(format!("{}|{}", s.session_id, s.master_key))),
                Err(e) => {
                    exception = Some(e.to_string());
                    m = Some(mega);
                    break;
                }
            }
            m = Some(mega);
        }
        out["steps"] = Value::Array(steps);
        out["uploader_errors"] = m.map(|m| errors_json(&m.base.errors)).unwrap_or(json!([]));
        if let Some(e) = exception {
            out["exception"] = json!(e);
        }
        return;
    }
    if service == "flickr" || service == "photobucket" {
        let info: OAuthInfo = serde_json::from_value(case["oauth1"].clone()).unwrap_or_default();
        let mut flickr = (service == "flickr").then(|| Flickr::new(info.clone(), Default::default()));
        let mut pb = (service == "photobucket").then(|| Photobucket::new(info, PhotobucketAccountInfo::default()));
        for step in case["steps"].as_array().into_iter().flatten() {
            let client: &mut dyn OAuth1Client = match (flickr.as_mut(), pb.as_mut()) {
                (Some(f), _) => f,
                (_, Some(p)) => p,
                _ => unreachable!(),
            };
            let r = match step["op"].as_str().unwrap_or("") {
                "authorization_url" => client.authorization_url().map(|u| json!(u)),
                "access_token" => client.get_access_token(step["code"].as_str().unwrap_or("")).map(|b| json!(b)),
                op => panic!("unknown op {op}"),
            };
            match r {
                Ok(v) => steps.push(v),
                Err(e) => {
                    exception = Some(e.to_string());
                    break;
                }
            }
        }
        let client: &mut dyn OAuth1Client = match (flickr.as_mut(), pb.as_mut()) {
            (Some(f), _) => f,
            (_, Some(p)) => p,
            _ => unreachable!(),
        };
        let mut auth = oauth1_json(client.auth_info());
        errors = errors_json(&client.uploader().errors);
        if let Some(p) = &pb {
            auth["Subdomain"] = json!(p.account_info.subdomain);
            auth["AlbumID"] = json!(p.account_info.album_id);
        }
        out["auth"] = auth;
    } else {
        let s = SignInService::from_page_id(service).unwrap_or_else(|| panic!("unknown service {service}"));
        let mut info: OAuth2Info = serde_json::from_value(case["oauth2"].clone()).unwrap_or_default();
        if let Some(p) = case.get("proof") {
            let proof = OAuth2ProofKey {
                code_verifier: p["verifier"].as_str().unwrap_or("").into(),
                code_challenge: p["challenge"].as_str().unwrap_or("").into(),
                method: OAuth2ChallengeMethod::Sha256,
            };
            info.Proof = proof.to_value();
        }
        let mut loopback: Option<Box<dyn OAuth2Loopback>> = s.loopback_client(info.clone());
        let mut code_paste: Option<Box<dyn OAuth2Client>> = if loopback.is_none() { s.oauth2_client(info) } else { None };
        if let (Some(l), Some(r)) = (loopback.as_mut(), case.get("redirect")) {
            l.set_redirect_uri(r["uri"].as_str().unwrap_or(""));
            l.set_state(r["state"].as_str().unwrap_or(""));
        }
        for step in case["steps"].as_array().into_iter().flatten() {
            let client: &mut dyn OAuth2Client = match (loopback.as_mut(), code_paste.as_mut()) {
                (Some(l), _) => l.as_mut(),
                (None, Some(c)) => c.as_mut(),
                _ => unreachable!(),
            };
            let r = match step["op"].as_str().unwrap_or("") {
                "authorization_url" => client.authorization_url().map(|u| json!(u)),
                "access_token" => client.get_access_token(step["code"].as_str().unwrap_or("")).map(|b| json!(b)),
                "refresh" => client.refresh_access_token().map(|b| json!(b)),
                "check" => client.check_authorization().map(|b| json!(b)),
                "user_info" => loopback.as_mut().expect("loopback").get_user_info().map(|u| match u {
                    Some(u) => json!(format!(
                        "{}|{}|{}|{}|{}",
                        u.sub.unwrap_or_default(),
                        u.name.unwrap_or_default(),
                        u.given_name.unwrap_or_default(),
                        u.picture.unwrap_or_default(),
                        u.locale.unwrap_or_default()
                    )),
                    None => Value::Null,
                }),
                op => panic!("unknown op {op}"),
            };
            match r {
                Ok(v) => steps.push(v),
                Err(e) => {
                    exception = Some(e.to_string());
                    break;
                }
            }
        }
        let client: &mut dyn OAuth2Client = match (loopback.as_mut(), code_paste.as_mut()) {
            (Some(l), _) => l.as_mut(),
            (None, Some(c)) => c.as_mut(),
            _ => unreachable!(),
        };
        out["auth"] = token_json(client.auth_info());
        errors = errors_json(&client.uploader().errors);
    }
    out["steps"] = Value::Array(steps);
    out["uploader_errors"] = errors;
    if let Some(e) = exception {
        out["exception"] = json!(e);
    }
}

/// A "remote" case: the remote storage browser's provider, step by step.
fn run_remote(case: &Value, out: &mut Value) {
    use sharex_uploaders::destinations::ftp::FTPAccount;
    use sharex_uploaders::remote_storage::{
        AmazonS3RemoteStorageProvider, CancelToken, FtpRemoteStorageProvider, RemoteStorageItem, RemoteStorageProvider, SftpRemoteStorageProvider,
    };
    let allow_upload = case["allow_upload"].as_bool().unwrap_or(true);
    let error = |e: &dyn std::fmt::Display| json!({"exception": e.to_string()});
    let mut provider: Box<dyn RemoteStorageProvider> = match case["provider"].as_str().unwrap_or("") {
        "ftp" => {
            let account: FTPAccount = serde_json::from_value(case["account"].clone()).expect("account");
            match FtpRemoteStorageProvider::new(account, allow_upload) {
                Ok(p) => Box::new(p),
                Err(e) => {
                    out["steps"] = json!([]);
                    out["create"] = error(&e);
                    return;
                }
            }
        }
        "sftp" => {
            let mut account: FTPAccount = serde_json::from_value(case["account"].clone()).expect("account");
            // Key files are relative to the repository.
            if let Some(k) = account.Keypath.clone().filter(|k| !k.is_empty()) {
                let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
                account.Keypath = Some(repo.join(k).to_string_lossy().into_owned());
            }
            Box::new(SftpRemoteStorageProvider::new(account, allow_upload))
        }
        "s3" => Box::new(AmazonS3RemoteStorageProvider::new(serde_json::from_value(case["s3"].clone()).expect("s3 settings"), allow_upload)),
        other => panic!("unknown provider {other}"),
    };
    out["display_name"] = json!(provider.display_name());
    out["root_display_name"] = json!(provider.root_display_name());
    out["root_path"] = json!(provider.root_path());
    out["capabilities"] = json!(provider.capabilities().0);
    let item = |v: &Value| RemoteStorageItem::new(v["name"].as_str().unwrap_or(""), v["path"].as_str().unwrap_or(""), v["is_folder"].as_bool().unwrap_or(false), v["size"].as_i64());
    let item_json = |i: &RemoteStorageItem| {
        json!({"name": i.name, "path": i.path, "is_folder": i.is_folder, "size": i.size,
               // DateTimeOffset.ToString("o").
               "modified": i.modified.map(|m| format!("{}.{:07}{}", m.format("%Y-%m-%dT%H:%M:%S"), m.timestamp_subsec_nanos() / 100, m.format("%:z")))})
    };
    let cancel = CancelToken::new();
    let mut steps = Vec::new();
    for step in case["steps"].as_array().into_iter().flatten() {
        let s = |k: &str| step[k].as_str().unwrap_or("").to_owned();
        let r: Result<Value, sharex_uploaders::remote_storage::RemoteStorageError> = match step["op"].as_str().unwrap_or("") {
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
                provider.upload(&s("dir"), &s("name"), &mut Cursor::new(bytes), &cancel).map(|()| json!("ok"))
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
}

/// A "listing" case: a destination picker's list (Imgur albums, MEGA folders).
fn run_listing(case: &Value, out: &mut Value) {
    use sharex_uploaders::destinations::images::Imgur;
    use sharex_uploaders::destinations::mega::{Mega, MegaFolderInfo, from_base64_url};
    use sharex_uploaders::json as sx_json;
    use sharex_uploaders::oauth::OAuth2Info;
    let error = |e: &dyn std::fmt::Display| json!({"exception": e.to_string()});
    let mut steps = Vec::new();
    match case["service"].as_str().unwrap_or("") {
        "imgur" => {
            let info: OAuth2Info = serde_json::from_value(case["oauth2"].clone()).unwrap_or_default();
            let mut imgur = Imgur::new(info);
            for step in case["steps"].as_array().into_iter().flatten() {
                let r = match step["op"].as_str().unwrap_or("") {
                    "albums" => imgur
                        .get_albums(step["max_page"].as_i64().unwrap_or(10) as i32, step["per_page"].as_i64().unwrap_or(100) as i32)
                        .map(|a| serde_json::to_value(a).expect("albums")),
                    "album_images" => imgur.get_album_images(step["album_id"].as_str().unwrap_or("")).map(|a| serde_json::to_value(a).expect("images")),
                    op => panic!("unknown op {op}"),
                };
                steps.push(match r {
                    Ok(v) => json!(sx_json::to_string(&v)),
                    Err(e) => error(&e),
                });
            }
            out["token"] = token_json(&imgur.auth_info);
            out["uploader_errors"] = errors_json(&imgur.base.errors);
        }
        "mega" => {
            let key = from_base64_url(case["master_key"].as_str().unwrap_or("")).unwrap_or_default();
            let mut mega = Mega::with_session(case["session_id"].as_str().unwrap_or(""), key);
            for step in case["steps"].as_array().into_iter().flatten() {
                let parent = (!step["parent"].is_null())
                    .then(|| MegaFolderInfo { id: step["parent"]["id"].as_str().map(str::to_owned), name: step["parent"]["name"].as_str().map(str::to_owned) });
                let root = MegaFolderInfo::root();
                steps.push(match mega.get_folders(Some(parent.as_ref().unwrap_or(&root))) {
                    Ok(folders) => Value::Array(folders.iter().map(|f| json!({"id": f.id, "name": f.name})).collect()),
                    Err(e) => error(&e),
                });
            }
            out["uploader_errors"] = errors_json(&mega.base.errors);
        }
        other => panic!("unknown listing service {other}"),
    }
    out["steps"] = Value::Array(steps);
}

/// `OAuthCases.Token`: what the .NET harness reports of a token.
fn token_json(info: &sharex_uploaders::oauth::OAuth2Info) -> Value {
    match &info.Token {
        None => json!({"access_token": null, "refresh_token": null, "expires_in": null, "token_type": null, "scope": null,
                       "has_expire_date": null, "expired": null}),
        Some(t) => json!({
            "access_token": t.access_token.get(),
            "refresh_token": t.refresh_token.get(),
            "expires_in": t.expires_in,
            "token_type": t.token_type,
            "scope": t.scope,
            "has_expire_date": t.ExpireDate.as_str().is_some_and(|d| !d.starts_with("0001-01-01T00:00:00")),
            "expired": t.is_expired(),
        }),
    }
}

fn run_case(case: &Value, port: u16) -> Value {
    reset(port, &case["responses"]);
    if !case["servers"].is_null() {
        reset_servers(&case["servers"]);
    }
    let mut out = json!({});
    if case["kind"] == "oauth" {
        run_oauth(case, &mut out);
        out["requests"] = log(port);
        return out;
    }
    if case["kind"] == "remote" {
        run_remote(case, &mut out);
        out["requests"] = log(port);
        if !case["servers"].is_null() {
            out["servers"] = servers_log();
        }
        return out;
    }
    if case["kind"] == "listing" {
        run_listing(case, &mut out);
        out["requests"] = log(port);
        return out;
    }
    let input = &case["input"];
    let file_name = input["file_name"].as_str().unwrap_or("").to_owned();
    let text = input["text"].as_str().map(str::to_owned);
    let url = input["url"].as_str().map(str::to_owned);
    let data: Vec<u8> = match input["content_base64"].as_str() {
        Some(b) => base64::engine::general_purpose::STANDARD.decode(b).expect("base64"),
        None => text.clone().unwrap_or_default().into_bytes(),
    };

    let runner: Result<Runner, String> = (|| {
        if case["kind"] == "custom" {
            let mut item = CustomUploaderItem::from_value(case["sxcu"].clone()).ok_or("bad sxcu")?;
            item.check_backward_compatibility()?;
            out["sxcu_export"] = json!(item.to_sxcu());
            out["request_url"] = json!(match item.get_request_url(&CustomUploaderInput::new(&file_name, text.as_deref().or(url.as_deref()).unwrap_or(""))) {
                Ok(u) => u,
                Err(e) => format!("EXCEPTION: {e}"),
            });
            return Ok(match case["dest"].as_str().unwrap_or("") {
                "image" | "file" => Runner::Generic(GenericUploader::File(Box::new(CustomFileUploader::new(item)))),
                "text" => Runner::Generic(GenericUploader::Text(Box::new(CustomTextUploader::new(item)))),
                "url_shortener" => Runner::Shortener(Box::new(CustomUrlShortener::new(item))),
                "url_sharing" => Runner::Sharer(Box::new(CustomUrlSharer::new(item))),
                other => return Err(format!("unknown dest {other}")),
            });
        }
        let mut config: UploadersConfig = serde_json::from_value(case["config"].clone()).map_err(|e| e.to_string())?;
        let task = TaskReferenceHelper { text_format: "text".into(), ..Default::default() };
        let service = case["service"].as_str().unwrap_or("");
        let (kind, name) = service.split_once(':').ok_or("bad service")?;
        let parse = |n: &str| serde_json::Value::String(n.to_owned());
        let ported = |p: Ported| match p {
            Ported::Yes(b) => Ok(b),
            Ported::No => Err(format!("{service} isn't ported")),
        };
        let runner = match kind {
            "image" => {
                let d = serde_json::from_value(parse(name)).map_err(|e| e.to_string())?;
                out["check_config"] = json!(ported(factory::check_image(&d, &config))?);
                Runner::Generic(factory::create_image(&d, &mut config, &task).ok_or("no uploader")?)
            }
            "text" => {
                let d = serde_json::from_value(parse(name)).map_err(|e| e.to_string())?;
                out["check_config"] = json!(ported(factory::check_text(&d, &config))?);
                Runner::Generic(factory::create_text(&d, &mut config, &task).ok_or("no uploader")?)
            }
            "file" => {
                let d = serde_json::from_value(parse(name)).map_err(|e| e.to_string())?;
                out["check_config"] = json!(ported(factory::check_file(&d, &config))?);
                Runner::Generic(factory::create_file(&d, &mut config, &task).ok_or("no uploader")?)
            }
            "shortener" => {
                let d = serde_json::from_value(parse(name)).map_err(|e| e.to_string())?;
                out["check_config"] = json!(ported(factory::check_url_shortener(&d, &config))?);
                Runner::Shortener(factory::create_url_shortener(&d, &mut config, &task).ok_or("no shortener")?)
            }
            "sharing" => {
                let d = serde_json::from_value(parse(name)).map_err(|e| e.to_string())?;
                out["check_config"] = json!(ported(factory::check_url_sharing(&d, &config))?);
                Runner::Sharer(factory::create_url_sharing(&d, &mut config, &task).ok_or("no sharer")?)
            }
            other => return Err(format!("unknown service kind {other}")),
        };
        Ok(runner)
    })();

    let (result, errors) = match runner {
        Err(e) => {
            out["exception"] = json!(e);
            (None, None)
        }
        Ok(Runner::Generic(mut g)) => {
            let r = match &mut g {
                GenericUploader::Text(t) => uploader::upload_text(t.as_mut(), text.as_deref().unwrap_or(""), &file_name),
                GenericUploader::File(_) => g.upload(&http::shared_stream(Cursor::new(data)), &file_name),
            };
            let errors = errors_json(&g.uploader().errors);
            (Some(r), Some(errors))
        }
        Ok(Runner::Shortener(mut s)) => {
            let r = uploader::shorten_url(s.as_mut(), url.as_deref().unwrap_or(""));
            (Some(r), Some(errors_json(&s.uploader().errors)))
        }
        Ok(Runner::Sharer(mut s)) => {
            let r = uploader::share_url(s.as_mut(), url.as_deref().unwrap_or(""));
            (Some(r), Some(errors_json(&s.uploader().errors)))
        }
    };
    match result {
        Some(Ok(r)) => out["result"] = result_json(&r),
        Some(Err(e)) => out["exception"] = json!(e),
        None => {}
    }
    if let Some(e) = errors {
        out["uploader_errors"] = e;
    }
    if let Some(path) = input["check_file"].as_str() {
        out["written_file_base64"] = match std::fs::read(path) {
            Ok(bytes) => json!(base64::engine::general_purpose::STANDARD.encode(bytes)),
            Err(_) => Value::Null,
        };
        let _ = std::fs::remove_file(path);
    }
    out["requests"] = log(port);
    if !case["servers"].is_null() {
        out["servers"] = servers_log();
    }
    out
}
