//! A harmless stand-in for an action's program (`tools/actions-harness` and
//! the `external_program` tests run it from ShareX's and ShareXrs's
//! `ExternalProgram.Run`): it appends a JSON line with the command line it
//! got (Windows: `GetCommandLineW`, as `CreateProcess` passed it) and its
//! arguments to the file in `SHAREXRS_ACTION_PROBE_LOG`, then does what the
//! first argument says, on files in the caller's scratch folder only:
//!
//! - `copy <from> <to>`: copies a file (an action that converts),
//! - `touch <path>`: writes a small file,
//! - `append <path>`: adds a byte to a file (an action that edits in place),
//! - `sleep <ms>`: waits (ShareX waits for the program to exit),
//! - anything else: nothing.

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Ok(log) = std::env::var("SHAREXRS_ACTION_PROBE_LOG") {
        let line = serde_json::json!({ "cmdline": command_line(), "args": args });
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
            let _ = writeln!(f, "{line}");
        }
    }
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    match arg(0).as_str() {
        "copy" => {
            let _ = std::fs::copy(arg(1), arg(2));
        }
        "touch" => {
            let _ = std::fs::write(arg(1), b"probe");
        }
        "append" => {
            if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(arg(1)) {
                let _ = f.write_all(b"+");
            }
        }
        "sleep" => std::thread::sleep(std::time::Duration::from_millis(arg(1).parse().unwrap_or(0))),
        _ => {}
    }
}

#[cfg(windows)]
fn command_line() -> Option<String> {
    use windows_sys::Win32::System::Environment::GetCommandLineW;
    // SAFETY: the process's command line, NUL-terminated, owned by the OS.
    unsafe {
        let p = GetCommandLineW();
        let mut n = 0;
        while *p.add(n) != 0 {
            n += 1;
        }
        Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, n)))
    }
}

#[cfg(not(windows))]
fn command_line() -> Option<String> {
    None
}
