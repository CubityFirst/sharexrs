//! `ShareX.NativeMessagingHost`: the program a browser extension talks to.
//! The browser starts it with the extension's origin as an argument and
//! sends one message on standard input; the host echoes it back, writes it
//! to a temporary `.json` file and starts the app beside it with
//! `-NativeMessagingInput "<file>"`, which ShareXrs (running already, or
//! passed on by the single instance pipe) reads and deletes
//! (`TaskHelpers.HandleNativeMessagingInput`).
//!
//! The framing is Chrome's and Firefox's native messaging: a message is its
//! UTF-8 length as a little-endian 32-bit integer, then the UTF-8 bytes
//! (`HelpersLib.NativeMessagingHost`).
//!
//! ShareXrs registers its own host names ([`CHROME_HOST_NAME`],
//! [`FIREFOX_HOST_NAME`]), never ShareX's (`com.getsharex.sharex`, `ShareX`),
//! so an installed ShareX keeps its own.

pub mod input;

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub use input::{NativeMessagingAction, NativeMessagingInput};

/// ShareX's `com.getsharex.sharex`, under ShareXrs's name.
pub const CHROME_HOST_NAME: &str = "com.sharexrs.sharexrs";
/// ShareX's `ShareX`, under ShareXrs's name.
pub const FIREFOX_HOST_NAME: &str = "ShareXrs";
/// The extensions ShareX's manifests allow (ShareX's own; a browser only
/// lets them reach this host if they ask for ShareXrs's host name).
pub const CHROME_ALLOWED_ORIGIN: &str = "chrome-extension://nlkoigbdolhchiicbonbihbphgamnaoc/";
pub const FIREFOX_ALLOWED_EXTENSION: &str = "firefox@getsharex.com";
/// `ShareX_NativeMessagingHost.exe`.
pub const HOST_FILE_NAME: &str = if cfg!(windows) { "sharexrs-native-messaging.exe" } else { "sharexrs-native-messaging" };
/// `ShareX.exe`, which the host starts from its own folder.
pub const APP_FILE_NAME: &str = if cfg!(windows) { "sharexrs.exe" } else { "sharexrs" };
/// The manifests' file names beside the app (`host-manifest-*.json`).
pub const CHROME_MANIFEST_FILE_NAME: &str = "host-manifest-chrome.json";
pub const FIREFOX_MANIFEST_FILE_NAME: &str = "host-manifest-firefox.json";
/// The largest message read: Chrome and Firefox send at most 64 MiB to a
/// host. (ShareX allocates whatever length it reads.)
pub const MAX_MESSAGE_LENGTH: usize = 64 * 1024 * 1024;

/// `NativeMessagingHost.Read`: the message, or `None` for a length of 0 or
/// less. Ending before the length or the message is an error
/// (`ReadExactly`).
pub fn read_message(r: &mut impl Read) -> io::Result<Option<String>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = i32::from_le_bytes(len);
    if len <= 0 {
        return Ok(None);
    }
    let len = len as usize;
    if len > MAX_MESSAGE_LENGTH {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("message too long: {len} bytes")));
    }
    let mut bytes = vec![0u8; len];
    r.read_exact(&mut bytes)?;
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

/// `NativeMessagingHost.Write`.
pub fn write_message(w: &mut impl Write, data: &str) -> io::Result<()> {
    w.write_all(&(data.len() as i32).to_le_bytes())?;
    w.write_all(data.as_bytes())?;
    w.flush()
}

/// The Chrome host manifest (`host-manifest-chrome.json`). `path` is the
/// host: on Windows a name beside the manifest is enough; elsewhere the
/// browsers want an absolute path.
pub fn chrome_manifest(path: &str) -> String {
    manifest(CHROME_HOST_NAME, path, "allowed_origins", CHROME_ALLOWED_ORIGIN)
}

/// The Firefox host manifest (`host-manifest-firefox.json`).
pub fn firefox_manifest(path: &str) -> String {
    manifest(FIREFOX_HOST_NAME, path, "allowed_extensions", FIREFOX_ALLOWED_EXTENSION)
}

fn manifest(name: &str, path: &str, allowed_key: &str, allowed: &str) -> String {
    let v = serde_json::json!({
        "name": name,
        "description": "ShareXrs",
        "path": path,
        "type": "stdio",
        allowed_key: [allowed],
    });
    serde_json::to_string_pretty(&v).expect("a manifest") + "\n"
}

/// What a run did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// An empty message: nothing to do.
    Empty,
    /// The message went to this file and the app was started on it.
    Forwarded(PathBuf),
}

/// `Program.Main` with arguments: reads the message, echoes it, writes it
/// to a new `.json` file in `temp_dir` and starts `app` with
/// `-NativeMessagingInput <file>` through `launch`.
pub fn run(
    stdin: &mut impl Read,
    stdout: &mut impl Write,
    app: &Path,
    temp_dir: &Path,
    launch: impl FnOnce(&Path, &[String]) -> io::Result<()>,
) -> io::Result<Outcome> {
    let Some(input) = read_message(stdin)?.filter(|s| !s.is_empty()) else {
        return Ok(Outcome::Empty);
    };
    write_message(stdout, &input)?;
    let file = write_temp_json(temp_dir, &input)?;
    launch(app, &["-NativeMessagingInput".to_owned(), file.to_string_lossy().into_owned()])?;
    Ok(Outcome::Forwarded(file))
}

/// `FileHelpers.GetTempFilePath("json")` then `File.WriteAllText(path,
/// input, Encoding.UTF8)` (with its BOM). ShareX's `Path.GetTempFileName`
/// also leaves an empty `.tmp` behind; this makes the `.json` directly.
pub fn write_temp_json(temp_dir: &Path, input: &str) -> io::Result<PathBuf> {
    for _ in 0..100 {
        let mut n = [0u8; 4];
        getrandom::fill(&mut n).map_err(|e| io::Error::other(e.to_string()))?;
        let path = temp_dir.join(format!("tmp{:08X}.json", u32::from_le_bytes(n)));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all("\u{feff}".as_bytes())?;
                f.write_all(input.as_bytes())?;
                return Ok(path);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "no free temporary file name"))
}

/// `NativeMethods.CreateProcess(filePath, argument,
/// CREATE_BREAKAWAY_FROM_JOB)`: started out of the browser's job, so it
/// outlives the host; with nothing of the host's standard streams.
pub fn launch(app: &Path, args: &[String]) -> io::Result<()> {
    let start = |breakaway: bool| {
        let mut c = std::process::Command::new(app);
        c.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
            if breakaway {
                c.creation_flags(CREATE_BREAKAWAY_FROM_JOB);
            }
        }
        #[cfg(not(windows))]
        let _ = breakaway;
        c.spawn().map(drop)
    };
    match start(true) {
        // A job that doesn't allow breaking away refuses the flag; ShareX
        // shows the error, ShareXrs starts the app inside the job instead.
        Err(e) if cfg!(windows) && e.raw_os_error() == Some(5) => start(false),
        r => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing() {
        let mut out = Vec::new();
        write_message(&mut out, "{\"a\":\"é\"}").unwrap();
        assert_eq!(&out[..4], &[10, 0, 0, 0]);
        assert_eq!(read_message(&mut out.as_slice()).unwrap().as_deref(), Some("{\"a\":\"é\"}"));
        assert_eq!(read_message(&mut [0u8, 0, 0, 0].as_slice()).unwrap(), None);
        assert_eq!(read_message(&mut [0xFFu8, 0xFF, 0xFF, 0xFF].as_slice()).unwrap(), None);
        // Short: ReadExactly throws.
        assert!(read_message(&mut [5u8, 0, 0].as_slice()).is_err());
        assert!(read_message(&mut [5u8, 0, 0, 0, b'a'].as_slice()).is_err());
        assert!(read_message(&mut [0u8, 0, 0, 0x7F].as_slice()).is_err());
        let mut empty = Vec::new();
        write_message(&mut empty, "").unwrap();
        assert_eq!(empty, [0, 0, 0, 0]);
    }

    #[test]
    fn manifests() {
        let chrome: serde_json::Value = serde_json::from_str(&chrome_manifest("sharexrs-native-messaging.exe")).unwrap();
        assert_eq!(
            chrome,
            serde_json::json!({
                "name": "com.sharexrs.sharexrs",
                "description": "ShareXrs",
                "path": "sharexrs-native-messaging.exe",
                "type": "stdio",
                "allowed_origins": ["chrome-extension://nlkoigbdolhchiicbonbihbphgamnaoc/"]
            })
        );
        let firefox: serde_json::Value = serde_json::from_str(&firefox_manifest("/opt/x/sharexrs-native-messaging")).unwrap();
        assert_eq!(firefox["name"], "ShareXrs");
        assert_eq!(firefox["allowed_extensions"], serde_json::json!(["firefox@getsharex.com"]));
        assert_eq!(firefox["path"], "/opt/x/sharexrs-native-messaging");
    }

    /// The manifests shipped beside the app are these.
    #[test]
    fn shipped_manifests_match() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/windows");
        let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap().replace("\r\n", "\n");
        assert_eq!(read(CHROME_MANIFEST_FILE_NAME), chrome_manifest("sharexrs-native-messaging.exe"));
        assert_eq!(read(FIREFOX_MANIFEST_FILE_NAME), firefox_manifest("sharexrs-native-messaging.exe"));
    }

    #[test]
    fn run_forwards_to_the_app() {
        let dir = std::env::temp_dir().join(format!("sharexrs-nm-run-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let message = r#"{"Action":4,"Text":"hello"}"#;
        let mut stdin = Vec::new();
        write_message(&mut stdin, message).unwrap();
        let mut stdout = Vec::new();
        let mut launched = None;
        let app = dir.join(APP_FILE_NAME);
        let outcome = run(&mut stdin.as_slice(), &mut stdout, &app, &dir, |a, args| {
            launched = Some((a.to_path_buf(), args.to_vec()));
            Ok(())
        })
        .unwrap();
        let Outcome::Forwarded(file) = outcome else { panic!("not forwarded") };
        // Echoed back to the browser.
        assert_eq!(read_message(&mut stdout.as_slice()).unwrap().as_deref(), Some(message));
        let (a, args) = launched.unwrap();
        assert_eq!(a, app);
        assert_eq!(args, vec!["-NativeMessagingInput".to_owned(), file.to_string_lossy().into_owned()]);
        assert!(file.extension().is_some_and(|e| e == "json"));
        let written = std::fs::read(&file).unwrap();
        assert_eq!(&written[..3], b"\xEF\xBB\xBF");
        assert_eq!(NativeMessagingInput::parse(std::str::from_utf8(&written).unwrap()).unwrap().text.as_deref(), Some("hello"));
        // Nothing to do for an empty message: no echo, no file, no app.
        let mut stdout = Vec::new();
        let r = run(&mut [0u8, 0, 0, 0].as_slice(), &mut stdout, &app, &dir, |_, _| panic!("launched")).unwrap();
        assert_eq!(r, Outcome::Empty);
        assert!(stdout.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
