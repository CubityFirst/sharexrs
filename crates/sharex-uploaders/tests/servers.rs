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
use sharex_uploaders::{FileDestination, UploadersConfig};

const HARNESS_BASE: u64 = 18820;

fn load(path: &str) -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).expect("JSON")
}

fn ftp_config(spec: &Value, tls: bool, implicit: bool) -> FtpMockConfig {
    FtpMockConfig {
        user: spec["user"].as_str().unwrap_or("user").to_owned(),
        password: spec["password"].as_str().unwrap_or("pass").to_owned(),
        dirs: spec["dirs"].as_array().map(|a| a.iter().filter_map(|d| d.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
        tls: if tls { test_tls_acceptor() } else { None },
        implicit_tls: implicit,
        fail_once: spec["fail_once"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| Some((p[0].as_str()?.to_owned(), p[1].as_str()?.to_owned()))).collect())
            .unwrap_or_default(),
    }
}

/// A port nothing listens on.
fn closed_port() -> u16 {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    l.local_addr().unwrap().port()
}

#[test]
fn ftp_cases_match_sharex() {
    let cases = load("../../tools/uploaders-harness/cases.json");
    let fixture = load("tests/fixtures/servers.sharex.json");
    let tls = test_tls_acceptor().is_some();
    if !tls {
        eprintln!("FTPS cases skipped: the platform's TLS can't load the test certificate");
    }
    let plain = FtpMockServer::start(FtpMockConfig::default()).unwrap();
    let explicit = FtpMockServer::start(FtpMockConfig::default()).unwrap();
    let implicit = FtpMockServer::start(FtpMockConfig::default()).unwrap();
    let mut ran = 0;
    for case in cases["cases"].as_array().unwrap() {
        if case["service"] != "file:FTP" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let expected = &fixture[id];
        assert!(!expected.is_null(), "{id}: no ShareX fixture (run tools/uploaders-harness/run.sh)");
        let mut config_json = case["config"].clone();
        let mut needs_tls = false;
        for account in config_json["FTPAccountList"].as_array_mut().unwrap() {
            let port = account["Port"].as_u64().unwrap();
            let s = match port - HARNESS_BASE {
                1 => &plain,
                2 => &explicit,
                3 => &implicit,
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
        let mut config: UploadersConfig = serde_json::from_value(config_json).unwrap();
        let task = TaskReferenceHelper::default();
        let mut uploader = factory::create_file(&FileDestination::FTP, &mut config, &task).expect("FTP uploader");
        let bytes = base64_decode(case["input"]["content_base64"].as_str().unwrap());
        let r = uploader.upload(&http::shared_stream(Cursor::new(bytes)), case["input"]["file_name"].as_str().unwrap());
        for s in [&plain, &explicit, &implicit] {
            s.wait_closed(std::time::Duration::from_secs(5));
        }
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
        ran += 1;
    }
    assert!(ran >= 10, "only {ran} FTP cases ran");
}

fn base64_decode(s: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s).unwrap()
}
