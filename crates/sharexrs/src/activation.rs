//! Bringing ShareXrs's own main window to the front as ShareX does
//! (`MainWindowIntegration.Activate`, `MainWindow.ShowAndActivate`).
//!
//! Avalonia's `Window.Activate()` on Windows is `SetForegroundWindow(hwnd)`
//! and nothing else (Avalonia.Win32 12.1.6 `WindowImpl.Activate`), so
//! Windows' foreground lock applies: unless this process may take the
//! foreground (it got the last input, a hotkey, or the process in front
//! passed its right on with `AllowSetForegroundWindow`, as a second launch
//! does over the single-instance pipe), the window only flashes in the
//! taskbar and the window in front stays there. winit's `focus_window`
//! instead synthesizes an Alt press (`SendInput`) before
//! `SetForegroundWindow`, which gets past the lock. When ShareXrs used it,
//! a forwarded `-ScreenRecorderActiveWindow` (or `-ActiveWindow`, or the
//! GIF variant) with the main window visible took the foreground away from
//! the window the user had active, and then recorded the main window
//! (live check REC2-03a).

/// `ApplicationCommandLine.ExecuteReceivedAsync`: a later launch's command
/// line activates the main window when it has no arguments, or when the
/// main window is visible.
pub(crate) fn activates_on_receive(arguments_empty: bool, main_visible: bool) -> bool {
    arguments_empty || main_visible
}

/// The one desktop call activating a window needs, so the activation can be
/// checked without a desktop.
#[cfg(any(windows, test))]
pub(crate) trait Foreground {
    /// `SetForegroundWindow`: Windows ignores it (bar a taskbar flash) when
    /// the foreground lock doesn't let this process take the foreground.
    fn set_foreground(&mut self, handle: isize);
}

/// Avalonia.Win32 `WindowImpl.Activate`: `SetForegroundWindow(_hwnd)`,
/// no synthesized input, no `AttachThreadInput`, no check of what's in
/// front first.
#[cfg(any(windows, test))]
pub(crate) fn activate(desktop: &mut impl Foreground, handle: isize) {
    desktop.set_foreground(handle);
}

/// The real desktop.
#[cfg(windows)]
struct Desktop;

#[cfg(windows)]
impl Foreground for Desktop {
    fn set_foreground(&mut self, handle: isize) {
        // `WindowInfo.Activate` is the same call.
        sharex_capture::platform::activate_window(handle);
    }
}

/// `Window.Activate()` for one of ShareXrs's windows: on Windows, Avalonia's
/// plain `SetForegroundWindow` (not winit's `focus_window`, see the module
/// docs); elsewhere winit's `focus_window` (`[NSApp activate…]` and
/// `makeKeyAndOrderFront` on macOS, as Avalonia.Native's `Activate`).
pub(crate) fn activate_window(window: &winit::window::Window) {
    #[cfg(windows)]
    if let Some(handle) = sharex_capture::platform::window_handle(window) {
        activate(&mut Desktop, handle);
    }
    #[cfg(not(windows))]
    window.focus_window();
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: isize = 0x1001;
    const TEST_WINDOW: isize = 0x2002;

    /// A desktop with Windows' foreground lock: `SetForegroundWindow` only
    /// moves the foreground when this process may take it. Any other way of
    /// taking the foreground (winit's synthesized Alt press) isn't on the
    /// trait, so `activate` can't use one.
    struct LockedDesktop {
        foreground: isize,
        may_take_foreground: bool,
        requests: Vec<isize>,
    }

    impl LockedDesktop {
        fn new(foreground: isize, may_take_foreground: bool) -> Self {
            Self { foreground, may_take_foreground, requests: Vec::new() }
        }

        /// `GetForegroundWindow`.
        fn foreground(&self) -> isize {
            self.foreground
        }
    }

    impl Foreground for LockedDesktop {
        fn set_foreground(&mut self, handle: isize) {
            self.requests.push(handle);
            if self.may_take_foreground {
                self.foreground = handle;
            }
        }
    }

    /// `ExecuteReceivedAsync` up to the job: the activation it asks for,
    /// then what the active-window job reads (`GetForegroundWindow` in
    /// `ScreenRecordManager.StartRecording` / `CaptureActiveWindow`).
    fn received_then_read_foreground(desktop: &mut LockedDesktop, arguments_empty: bool, main_visible: bool) -> isize {
        if activates_on_receive(arguments_empty, main_visible) {
            activate(desktop, MAIN);
        }
        desktop.foreground()
    }

    #[test]
    fn received_arguments_activate_the_main_window_as_sharex() {
        // `if (args.Length == 0) … Activate(); else if (IsVisible) Activate();`
        assert!(activates_on_receive(true, false));
        assert!(activates_on_receive(true, true));
        assert!(activates_on_receive(false, true));
        assert!(!activates_on_receive(false, false));
    }

    #[test]
    fn a_forwarded_active_window_job_keeps_the_active_window_in_front() {
        // REC2-03a: the test window is in front, the main window visible,
        // and the sender couldn't pass on a foreground right.
        let mut desktop = LockedDesktop::new(TEST_WINDOW, false);
        assert_eq!(received_then_read_foreground(&mut desktop, false, true), TEST_WINDOW);
        // Activation was asked for, as ShareX asks, but only plainly.
        assert_eq!(desktop.requests, [MAIN]);
    }

    #[test]
    fn a_hidden_main_window_isnt_activated() {
        // REC2-03b: hidden to the tray, nothing is asked for.
        let mut desktop = LockedDesktop::new(TEST_WINDOW, false);
        assert_eq!(received_then_read_foreground(&mut desktop, false, false), TEST_WINDOW);
        assert!(desktop.requests.is_empty());
    }

    #[test]
    fn a_permitted_activation_takes_the_foreground_as_avalonias() {
        // Launched from the process in front (Explorer), whose right the
        // sender passes on: ShareX's main window comes to the front, and so
        // does ShareXrs's.
        let mut desktop = LockedDesktop::new(TEST_WINDOW, true);
        assert_eq!(received_then_read_foreground(&mut desktop, false, true), MAIN);
    }
}
