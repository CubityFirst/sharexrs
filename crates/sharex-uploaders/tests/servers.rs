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
            s.wait_closed(std::time::Duration::from_secs(5));
        }
        sftp.wait_closed(std::time::Duration::from_secs(5));
        let (url, exception) = match r {
            Ok(Some(res)) => (res.url, None),
            Ok(None) => (None, None),
            Err(e) => (None, Some(e)),
        };
        assert_eq!(exception.as_deref(), expected["exception"].as_str(), "{id}: exception");
        assert_eq!(url.as_deref(), expected["url"].as_str(), "{id}: URL");
        let log: Vec<String> = [&plain, &explicit, &implicit].iter().flat_map(|s| s.log_lines()).collect();
        let expected_log: Vec<String> = expected["servers"]["ftp"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect();
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
        smtp.wait_closed(std::time::Duration::from_secs(5));
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
