//! One running copy, as ShareX's `SingleInstanceManager`: later launches
//! pass their command line to the first copy and exit; the first runs it.
//!
//! Runs a portable scratch copy of `sharexrs` (its own folder, a `Portable`
//! file, a profile with no hotkeys, no Start screen, no update check, no
//! sounds, started with `-silent` so only its tray icon shows), so it never
//! meets another ShareXrs on the machine: a portable copy's mutex and pipe
//! are its folder's own. Only the processes this test starts are stopped,
//! by their handles. Windows only: elsewhere the app needs a desktop
//! session, and `sharex-helpers`' tests cover the pipe on every system.

#![cfg(windows)]

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// A started copy, stopped when the test ends however it ends.
struct Started(Child);

impl Drop for Started {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// The first copy's log, line by line.
struct Log {
    lines: Vec<String>,
    rx: Receiver<String>,
}

impl Log {
    fn new(child: &mut Child) -> Self {
        let stderr = child.stderr.take().expect("piped");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self { lines: Vec::new(), rx }
    }

    /// Waits for a line `want` accepts.
    fn wait_for(&mut self, what: &str, want: impl Fn(&str) -> bool) -> String {
        if let Some(l) = self.lines.iter().find(|l| want(l)) {
            return l.clone();
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match self.rx.recv_timeout(left) {
                Ok(line) => {
                    self.lines.push(line.clone());
                    if want(&line) {
                        return line;
                    }
                }
                Err(_) => break,
            }
        }
        panic!("no {what} in the first copy's log:\n{}", self.lines.join("\n"));
    }
}

fn scratch_copy() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("single-instance-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profile = dir.join("ShareXrs");
    std::fs::create_dir_all(&profile).unwrap();
    let exe = dir.join("sharexrs.exe");
    std::fs::copy(env!("CARGO_BIN_EXE_sharexrs"), &exe).unwrap();
    std::fs::write(dir.join("Portable"), b"").unwrap();
    let config = serde_json::json!({
        "ShowStartScreen": false,
        "ShowTray": true,
        "SilentRun": true,
        "AutoCheckUpdate": false,
        "DisableHotkeys": true,
        "DefaultTaskSettings": { "GeneralSettings": { "PlaySoundAfterAction": false, "PlaySoundAfterUpload": false } },
    });
    std::fs::write(profile.join("ApplicationConfig.json"), config.to_string()).unwrap();
    std::fs::write(profile.join("HotkeysConfig.json"), r#"{"Hotkeys":[]}"#).unwrap();
    exe
}

/// A later launch: it should pass its command line on and exit at once.
fn launch_again(exe: &Path, args: &[&str]) -> String {
    let mut child = Started(Command::new(exe).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(s) = child.0.try_wait().unwrap() {
            break s;
        }
        assert!(Instant::now() < deadline, "the second copy didn't exit");
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut err = String::new();
    child.0.stderr.take().unwrap().read_to_string(&mut err).unwrap();
    assert!(status.success(), "the second copy failed: {status}\n{err}");
    assert!(!err.contains("Startup time"), "the second copy started the app:\n{err}");
    err
}

#[test]
fn a_second_copy_passes_its_command_line_to_the_first() {
    let exe = scratch_copy();
    let dir = exe.parent().unwrap().to_path_buf();
    let mut first =
        Started(Command::new(&exe).args(["-silent"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap());
    let mut log = Log::new(&mut first.0);
    log.wait_for("startup", |l| l.starts_with("Startup time:"));
    // The first copy's own command line ran once startup was done.
    log.wait_for("its own command line", |l| l == "CommandLine: -silent");

    // ShareX's quirk shows: the path becomes -NoHotkeys' parameter, and the
    // folder after it is uploaded (it doesn't exist, so nothing happens).
    let missing = dir.join("no such folder");
    let missing = missing.to_str().unwrap();
    launch_again(&exe, &["-NoHotkeys", r"C:\nonexistent\a.png", missing, "-workflow", "No such workflow"]);
    log.wait_for("the arguments", |l| l == format!(r#"Arguments received: "-NoHotkeys C:\nonexistent\a.png {missing} -workflow No such workflow""#));
    log.wait_for("-NoHotkeys", |l| l == r#"CommandLine: -NoHotkeys "C:\nonexistent\a.png""#);
    log.wait_for("the folder", |l| l == format!("CommandLine: {missing}"));
    log.wait_for("-workflow", |l| l == r#"CommandLine: -workflow "No such workflow""#);

    // The browser extension's path: the host (its library, with its real
    // launcher) writes the message and starts this copy with
    // -NativeMessagingInput, which passes it to the first; that one reads
    // the file and deletes it.
    let message = r#"{"Action":"None"}"#;
    let mut stdin = Vec::new();
    sharexrs_native_messaging::write_message(&mut stdin, message).unwrap();
    let mut stdout = Vec::new();
    let outcome =
        sharexrs_native_messaging::run(&mut stdin.as_slice(), &mut stdout, &exe, &dir, sharexrs_native_messaging::launch).unwrap();
    let sharexrs_native_messaging::Outcome::Forwarded(file) = outcome else { panic!("not forwarded") };
    assert_eq!(sharexrs_native_messaging::read_message(&mut stdout.as_slice()).unwrap().as_deref(), Some(message));
    let line = format!("CommandLine: -NativeMessagingInput \"{}\"", file.display());
    log.wait_for("-NativeMessagingInput", |l| l == line);
    let deadline = Instant::now() + Duration::from_secs(30);
    while file.exists() {
        assert!(Instant::now() < deadline, "the first copy didn't take the message file");
        std::thread::sleep(Duration::from_millis(50));
    }

    // A job by its hotkey type: -ExitShareX closes the first copy.
    launch_again(&exe, &["-ExitShareX"]);
    log.wait_for("-ExitShareX", |l| l == "CommandLine: -ExitShareX");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(s) = first.0.try_wait().unwrap() {
            break s;
        }
        assert!(Instant::now() < deadline, "-ExitShareX didn't close the first copy");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success(), "{status}");

    // With nobody left, a launch is the first copy again: it starts (and
    // is closed by its own -ExitShareX once startup is done).
    let mut again =
        Started(Command::new(&exe).args(["-silent", "-ExitShareX"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap());
    let mut log = Log::new(&mut again.0);
    log.wait_for("startup", |l| l.starts_with("Startup time:"));
    let deadline = Instant::now() + Duration::from_secs(30);
    while again.0.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "-ExitShareX didn't close it");
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(again);
    drop(first);
    let _ = std::fs::remove_dir_all(&dir);
}
