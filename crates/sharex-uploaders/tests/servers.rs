//! The FTP/FTPS (and later SFTP, SMTP) cases of `tools/uploaders-harness`
//! run in-process against the mock servers, compared with what ShareX's
//! own code did on the same servers (`tests/fixtures/servers.sharex.json`,
//! written by the harness from ShareX's results): every command, reply and
//! stored byte, and the result. Runs on every platform; FTPS cases are
//! skipped where the platform's TLS can't load the test certificate.

use std::io::Cursor;

use serde_json::Value;
use sharex_uploaders::factory::{self, TaskReferenceHelper};
use sharex_uploaders::http;
use sharex_uploaders::testing::ftp::{FtpMockConfig, FtpMockServer, test_tls_acceptor};
use sharex_uploaders::testing::sftp::{SftpMockConfig, SftpMockServer};
use sharex_uploaders::testing::smtp::{SmtpMockConfig, SmtpMockServer};
use sharex_uploaders::uploader;
use sharex_uploaders::URLSharingServices;
use sharex_uploaders::{FileDestination, UploadersConfig};

/// How long a case waits for the mock servers' sessions to close before it
/// compares their logs. It returns as soon as they have closed; the margin
/// is for loaded CI runners and full workspace runs, where the SFTP cases'
/// key exchanges (8192-bit DH group 18) run in parallel and once kept an
/// FTPS session open past the old 5 s.
const CLOSE_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

const HARNESS_BASE: u64 = 18820;

fn load(path: &str) -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).expect("JSON")
}

fn ftp_config(spec: &Value, tls: bool, implicit: bool) -> FtpMockConfig {
    FtpMockConfig::from_spec(spec, tls, implicit)
}

fn sftp_config(v: &Value) -> SftpMockConfig {
    SftpMockConfig::from_spec(v)
}

fn lines(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().map(|l| l.as_str().unwrap().to_owned()).collect()).unwrap_or_default()
}

fn file_list(v: &Value) -> Vec<(String, Vec<u8>)> {
    v.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), base64_decode(v.as_str().unwrap()))).collect()).unwrap_or_default()
}

/// A port nothing listens on.
fn closed_port() -> u16 {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    l.local_addr().unwrap().port()
}

#[test]
fn ftp_and_sftp_cases_match_sharex() {
    let cases = load("../../tools/uploaders-harness/cases.json");
    let fixture = load("tests/fixtures/servers.sharex.json");
    let tls = test_tls_acceptor().is_some();
    if !tls {
        eprintln!("FTPS cases skipped: the platform's TLS can't load the test certificate");
    }
    let plain = FtpMockServer::start(FtpMockConfig::default()).unwrap();
    let explicit = FtpMockServer::start(FtpMockConfig::default()).unwrap();
    let implicit = FtpMockServer::start(FtpMockConfig::default()).unwrap();
    let sftp = SftpMockServer::start(SftpMockConfig::default()).unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ran = 0;
    for case in cases["cases"].as_array().unwrap() {
        if case["service"] != "file:FTP" || case["config"]["FTPAccountList"].is_null() {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let expected = &fixture[id];
        assert!(!expected.is_null(), "{id}: no ShareX fixture (run tools/uploaders-harness/run.sh)");
        let mut config_json = case["config"].clone();
        let mut needs_tls = false;
        for account in config_json["FTPAccountList"].as_array_mut().unwrap() {
            let port = account["Port"].as_u64().unwrap();
            // Key files are relative to the repository (the harness's
            // working directory).
            for field in ["Keypath", "FTPSCertificateLocation"] {
                if let Some(k) = account[field].as_str().filter(|k| !k.is_empty()) {
                    account[field] = repo.join(k).to_string_lossy().into_owned().into();
                }
            }
            let s = match port - HARNESS_BASE {
                1 => &plain,
                2 => &explicit,
                3 => &implicit,
                4 => {
                    account["Port"] = sftp.addr.port().into();
                    continue;
                }
                _ => {
                    account["Port"] = closed_port().into();
                    continue;
                }
            };
            needs_tls |= port - HARNESS_BASE > 1;
            account["Port"] = s.addr.port().into();
        }
        if needs_tls && !tls {
            eprintln!("{id}: skipped (no TLS)");
            continue;
        }
        let spec = &case["servers"];
        for (s, t, i) in [(&plain, false, false), (&explicit, true, false), (&implicit, true, true)] {
            s.reset(Some(ftp_config(spec, t, i)));
        }
        sftp.reset(Some(sftp_config(spec)));
        let mut config: UploadersConfig = serde_json::from_value(config_json).unwrap();
        let task = TaskReferenceHelper::default();
        let mut uploader = factory::create_file(&FileDestination::FTP, &mut config, &task).expect("FTP uploader");
        let bytes = base64_decode(case["input"]["content_base64"].as_str().unwrap());
        let r = uploader.upload(&http::shared_stream(Cursor::new(bytes)), case["input"]["file_name"].as_str().unwrap());
        for s in [&plain, &explicit, &implicit] {
            s.wait_closed(CLOSE_WAIT);
        }
        sftp.wait_closed(CLOSE_WAIT);
        let (url, exception) = match r {
            Ok(Some(res)) => (res.url, None),
            Ok(None) => (None, None),
            Err(e) => (None, Some(e)),
        };
        assert_eq!(exception.as_deref(), expected["exception"].as_str(), "{id}: exception");
        assert_eq!(url.as_deref(), expected["url"].as_str(), "{id}: URL");
        let log: Vec<String> = [&plain, &explicit, &implicit].iter().flat_map(|s| s.log_lines()).collect();
        let mut expected_log: Vec<String> = expected["servers"]["ftp"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect();
        // When the client rejects the mock's self-signed certificate (an
        // FTPS account with a client certificate stops accepting untrusted
        // servers, as ShareX does), Windows' SChannel and OpenSSL complete
        // the handshake before rejecting it, so the mock logs "tls" and
        // whatever it sends next (implicit TLS's greeting); macOS's Security
        // framework aborts inside the handshake, so the mock logs nothing
        // between the handshake and the close. The commands before it and
        // the exception are compared as they are.
        if cfg!(target_os = "macos")
            && id.starts_with("ftps-")
            && expected["exception"].is_string()
            && let Some(i) = expected_log.iter().position(|l| l == "tls")
        {
            let closed = expected_log.last().is_some_and(|l| l == "close");
            expected_log.truncate(i);
            if closed {
                expected_log.push("close".into());
            }
        }
        assert_eq!(log, expected_log, "{id}: commands and replies");
        let files: Vec<(String, Vec<u8>)> = [&plain, &explicit, &implicit].iter().flat_map(|s| s.files()).collect();
        let expected_files: Vec<(String, Vec<u8>)> = expected["servers"]["files"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), base64_decode(v.as_str().unwrap())))
            .collect();
        assert_eq!(files, expected_files, "{id}: stored files");
        assert_eq!(sftp.log_lines(), lines(&expected["servers"]["sftp"]), "{id}: SFTP requests and replies");
        assert_eq!(sftp.files().into_iter().collect::<Vec<_>>(), file_list(&expected["servers"]["sftp_files"]), "{id}: SFTP files");
        ran += 1;
    }
    assert!(ran >= 20, "only {ran} FTP/SFTP cases ran");
}

/// A step's dates without their UTC offset (ShareX's run was in its
/// machine's time zone; FTP listings give local times); a listing whose
/// dates depend on the year it ran in loses them.
fn norm_dates(v: &Value, drop: bool) -> Value {
    match v {
        Value::Object(o) => Value::Object(
            o.iter()
                .filter_map(|(k, x)| match (k.as_str(), x) {
                    ("modified", _) if drop => None,
                    ("modified", Value::String(s)) => Some((k.clone(), Value::String(s.chars().take(27).collect()))),
                    _ => Some((k.clone(), norm_dates(x, drop))),
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(|x| norm_dates(x, drop)).collect()),
        other => other.clone(),
    }
}

#[test]
fn remote_storage_cases_match_sharex() {
    use sharex_uploaders::destinations::ftp::FTPAccount;
    let cases = load("../../tools/uploaders-harness/cases.json");
    let fixture = load("tests/fixtures/servers.sharex.json");
    let tls = test_tls_acceptor().is_some();
    let plain = FtpMockServer::start(FtpMockConfig::default()).unwrap();
    let explicit = FtpMockServer::start(FtpMockConfig::default()).unwrap();
    let implicit = FtpMockServer::start(FtpMockConfig::default()).unwrap();
    let sftp = SftpMockServer::start(SftpMockConfig::default()).unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ran = 0;
    for case in cases["cases"].as_array().unwrap() {
        if case["kind"] != "remote" || case["account"].is_null() {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let expected = &fixture[id];
        assert!(!expected.is_null(), "{id}: no ShareX fixture (run tools/uploaders-harness/run.sh)");
        let mut account_json = case["account"].clone();
        let port = account_json["Port"].as_u64().unwrap();
        let mapped = match port - HARNESS_BASE {
            1 => plain.addr.port(),
            2 | 3 if !tls => {
                eprintln!("{id}: skipped (no TLS)");
                continue;
            }
            2 => explicit.addr.port(),
            3 => implicit.addr.port(),
            4 => sftp.addr.port(),
            _ => closed_port(),
        };
        account_json["Port"] = mapped.into();
        if let Some(k) = account_json["Keypath"].as_str().filter(|k| !k.is_empty()) {
            account_json["Keypath"] = repo.join(k).to_string_lossy().into_owned().into();
        }
        let spec = &case["servers"];
        for (s, t, i) in [(&plain, false, false), (&explicit, true, false), (&implicit, true, true)] {
            s.reset(Some(ftp_config(spec, t, i)));
        }
        sftp.reset(Some(sftp_config(spec)));
        let account: FTPAccount = serde_json::from_value(account_json).unwrap();
        let got = sharex_uploaders::testing::remote::run_remote_case(case, Some(account));
        for s in [&plain, &explicit, &implicit] {
            s.wait_closed(CLOSE_WAIT);
        }
        sftp.wait_closed(CLOSE_WAIT);
        let drop = id == "remote-ftp-listing-formats";
        for k in ["display_name", "root_display_name", "root_path", "capabilities", "create"] {
            if expected.get(k).is_some() || !got[k].is_null() {
                // Display names hold the harness's port.
                let norm = |v: &Value| Value::String(v.to_string().replace(&mapped.to_string(), "PORT").replace(&port.to_string(), "PORT"));
                assert_eq!(norm(&got[k]), norm(&expected[k]), "{id}: {k}");
            }
        }
        let norm_steps = |v: &Value| Value::String(norm_dates(v, drop).to_string().replace(&mapped.to_string(), "PORT").replace(&port.to_string(), "PORT"));
        let (got_steps, expected_steps) = (got["steps"].as_array().cloned().unwrap_or_default(), expected["steps"].as_array().cloned().unwrap_or_default());
        assert_eq!(got_steps.len(), expected_steps.len(), "{id}: step count");
        for (n, (g, e)) in got_steps.iter().zip(&expected_steps).enumerate() {
            assert_eq!(norm_steps(g), norm_steps(e), "{id}: step {n} ({})", case["steps"][n]["op"]);
        }
        let log: Vec<String> = [&plain, &explicit, &implicit].iter().flat_map(|s| s.log_lines()).collect();
        assert_eq!(log, lines(&expected["servers"]["ftp"]), "{id}: FTP commands and replies");
        // The harness merges the three FTP servers' files (each starts with
        // the case's files), later servers over earlier ones.
        // (A JSON object in insertion order: a later value replaces in place.)
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for s in [&plain, &explicit, &implicit] {
            for (k, v) in s.files() {
                match files.iter_mut().find(|(f, _)| *f == k) {
                    Some(e) => e.1 = v,
                    None => files.push((k, v)),
                }
            }
        }
        assert_eq!(files, file_list(&expected["servers"]["files"]), "{id}: FTP files");
        assert_eq!(sftp.log_lines(), lines(&expected["servers"]["sftp"]), "{id}: SFTP requests and replies");
        assert_eq!(sftp.files().into_iter().collect::<Vec<_>>(), file_list(&expected["servers"]["sftp_files"]), "{id}: SFTP files");
        ran += 1;
    }
    assert!(ran >= 15, "only {ran} remote storage cases ran");
}

/// A mail with its date and boundary (counter and GUID) normalised.
fn norm_mail(m: &str) -> String {
    let mut out = String::new();
    for line in m.split("\r\n") {
        let line = if line.starts_with("Date: ") { "Date: DATE".to_owned() } else { line.to_owned() };
        let line = match line.find("--boundary_") {
            Some(i) => {
                let rest = &line[i + "--boundary_".len()..];
                let end = rest.find(|c: char| !(c.is_ascii_hexdigit() || c == '-' || c == '_' || c.is_ascii_digit())).unwrap_or(rest.len());
                format!("{}--boundary_N_GUID{}", &line[..i], &rest[end..])
            }
            None => line,
        };
        out.push_str(&line);
        out.push_str("\r\n");
    }
    out
}

#[test]
fn email_cases_match_sharex() {
    let cases = load("../../tools/uploaders-harness/cases.json");
    let fixture = load("tests/fixtures/servers.sharex.json");
    let Some(tls) = test_tls_acceptor() else {
        eprintln!("email cases skipped: the platform's TLS can't load the test certificate");
        return;
    };
    sharex_uploaders::destinations::email::set_accept_invalid_certificates_for_tests(true);
    let smtp = SmtpMockServer::start(SmtpMockConfig::default()).unwrap();
    let mut ran = 0;
    for case in cases["cases"].as_array().unwrap() {
        let service = case["service"].as_str().unwrap_or("");
        if service != "file:Email" && service != "sharing:Email" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let expected = &fixture[id];
        let spec = &case["servers"];
        smtp.reset(Some(SmtpMockConfig {
            user: spec["user"].as_str().unwrap_or("user").into(),
            password: spec["password"].as_str().unwrap_or("pass").into(),
            tls: Some(tls.clone()),
            fail_once: Vec::new(),
        }));
        let mut config_json = case["config"].clone();
        config_json["EmailSmtpPort"] = smtp.addr.port().into();
        let mut config: UploadersConfig = serde_json::from_value(config_json).unwrap();
        let task = TaskReferenceHelper::default();
        let r = if service == "file:Email" {
            let mut u = factory::create_file(&FileDestination::Email, &mut config, &task).expect("email uploader");
            let bytes = base64_decode(case["input"]["content_base64"].as_str().unwrap());
            u.upload(&http::shared_stream(Cursor::new(bytes)), case["input"]["file_name"].as_str().unwrap())
        } else {
            let mut u = factory::create_url_sharing(&URLSharingServices::Email, &mut config, &task).expect("email sharer");
            uploader::share_url(u.as_mut(), case["input"]["url"].as_str().unwrap())
        };
        smtp.wait_closed(CLOSE_WAIT);
        let (url, exception) = match r {
            Ok(Some(res)) => (res.url, None),
            Ok(None) => (None, None),
            Err(e) => (None, Some(e)),
        };
        assert_eq!(exception.as_deref(), expected["exception"].as_str(), "{id}: exception");
        assert_eq!(url.as_deref(), expected["url"].as_str(), "{id}: URL");
        assert_eq!(smtp.log_lines(), lines(&expected["servers"]["smtp"]), "{id}: SMTP commands and replies");
        let mails: Vec<String> = smtp.messages().iter().map(|m| norm_mail(&String::from_utf8_lossy(m))).collect();
        let expected_mails: Vec<String> = lines(&expected["servers"]["mails"]).iter().map(|m| norm_mail(m)).collect();
        assert_eq!(mails, expected_mails, "{id}: the mail");
        ran += 1;
    }
    assert!(ran >= 4, "only {ran} email cases ran");
}

fn base64_decode(s: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s).unwrap()
}
