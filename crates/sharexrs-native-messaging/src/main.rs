//! `ShareX.NativeMessagingHost/Program.cs`: started by a browser (with the
//! extension's origin as an argument), it passes one message on to ShareXrs
//! beside it. Started by hand, with no arguments, it says what it's for.

// A window-less program, as ShareX's `WinExe`; the browser still gives it
// its standard input and output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sharexrs_native_messaging::{APP_FILE_NAME, launch, run};

const TITLE: &str = "ShareXrs NativeMessagingHost";

fn main() {
    if std::env::args_os().len() > 1 {
        let app = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join(APP_FILE_NAME)))
            .unwrap_or_else(|| APP_FILE_NAME.into());
        let result = run(&mut std::io::stdin().lock(), &mut std::io::stdout().lock(), &app, &std::env::temp_dir(), launch);
        // `e.ShowError()`.
        if let Err(e) = result {
            message_box(&e.to_string(), true);
        }
    } else {
        message_box("This executable is used to receive data from browser addon and send it to ShareXrs.", false);
    }
}

#[cfg(windows)]
fn message_box(text: &str, error: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MessageBoxW};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (text, title) = (wide(text), wide(if error { "ShareXrs - Error" } else { TITLE }));
    let icon = if error { MB_ICONERROR } else { MB_ICONINFORMATION };
    // SAFETY: null-terminated strings that outlive the call.
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | icon) };
}

#[cfg(not(windows))]
fn message_box(text: &str, _error: bool) {
    eprintln!("{TITLE}: {text}");
}
