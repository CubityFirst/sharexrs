//! `WindowsTrayIcon`: a notification area icon through `Shell_NotifyIconW`
//! that reports every mouse button. Its messages go to a hidden window on
//! the main thread (snapr's approach), so winit's message loop drives it.

use std::cell::RefCell;
use std::ptr::null;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, GetSystemMetrics, HICON,
    LR_DEFAULTCOLOR, RegisterClassW, RegisterWindowMessageW, SM_CXSMICON, WNDCLASSW,
};

use super::{Button, Callback, NativeEntry, TrayEvent};

const CLASS_NAME: &str = "ShareXrs.TrayIcon";
/// `WindowsTrayIcon.CallbackMessage`.
const CALLBACK_MESSAGE: u32 = 0x8000 + 107;

struct State {
    hwnd: HWND,
    icon: HICON,
    tooltip: String,
    visible: bool,
    added: bool,
    taskbar_created: u32,
    callback: Callback,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// `WindowsTrayIcon.DoubleClickTime`.
pub fn double_click_time() -> std::time::Duration {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
    // SAFETY: no arguments.
    std::time::Duration::from_millis(unsafe { GetDoubleClickTime() } as u64)
}

pub struct Tray {
    hwnd: HWND,
}

impl Tray {
    /// `new DesktopTrayIconService(host, icon, text, visible)`.
    pub fn new(tooltip: &str, visible: bool, callback: Callback) -> Result<Self, String> {
        let class = wide(CLASS_NAME);
        // SAFETY: plain Win32 calls with valid, NUL-terminated strings.
        let hwnd = unsafe {
            let hinstance = GetModuleHandleW(null());
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(wndproc);
            wc.hInstance = hinstance;
            wc.lpszClassName = class.as_ptr();
            RegisterClassW(&wc);
            // A hidden top-level window rather than a message-only one, so
            // it gets the "TaskbarCreated" broadcast after Explorer restarts.
            CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0, 0, 0, 0, 0, 0 as HWND, 0 as _, hinstance, null())
        };
        if hwnd.is_null() {
            return Err("couldn't create the tray icon's window".into());
        }
        let state = State {
            hwnd,
            icon: std::ptr::null_mut(),
            tooltip: String::new(),
            visible: false,
            added: false,
            // SAFETY: a NUL-terminated string.
            taskbar_created: unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) },
            callback,
        };
        STATE.with(|s| *s.borrow_mut() = Some(state));
        let tray = Self { hwnd };
        tray.set_tooltip(tooltip);
        tray.set_icon(&super::app_icon(Self::icon_size()));
        tray.set_visible(visible);
        Ok(tray)
    }

    /// `SM_CXSMICON`: the size `SetIcon` decodes the icon at.
    pub fn icon_size() -> u32 {
        // SAFETY: no pointers.
        unsafe { GetSystemMetrics(SM_CXSMICON) }.clamp(16, 256) as u32
    }

    /// `SetIcon`: the picture, as a PNG icon resource
    /// (`CreateIconFromResourceEx`).
    pub fn set_icon(&self, image: &image::RgbaImage) {
        let mut png = Vec::new();
        if image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).is_err() {
            return;
        }
        // SAFETY: a PNG in memory, read during the call.
        let icon = unsafe {
            CreateIconFromResourceEx(
                png.as_ptr(),
                png.len() as u32,
                1,
                0x30000,
                image.width() as i32,
                image.height() as i32,
                LR_DEFAULTCOLOR,
            )
        };
        if icon.is_null() {
            return;
        }
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                let previous = std::mem::replace(&mut state.icon, icon);
                update(state);
                if !previous.is_null() {
                    // SAFETY: ours, and the shell has the new one.
                    unsafe { DestroyIcon(previous) };
                }
            }
        });
    }

    /// `ToolTipText` (at most 63 characters).
    pub fn set_tooltip(&self, text: &str) {
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                let text: String = text.chars().take(63).collect();
                if state.tooltip != text {
                    state.tooltip = text;
                    update(state);
                }
            }
        });
    }

    /// `Visible`.
    pub fn set_visible(&self, visible: bool) {
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                state.visible = visible;
                update(state);
            }
        });
    }

    /// Windows shows ShareX's own menu instead (`tray_menu.rs`).
    pub fn set_menu(&self, _entries: Vec<NativeEntry>) {}
}

impl Drop for Tray {
    fn drop(&mut self) {
        if let Some(mut state) = STATE.with(|s| s.borrow_mut().take()) {
            state.visible = false;
            update(&mut state);
            if !state.icon.is_null() {
                // SAFETY: ours, no longer shown.
                unsafe { DestroyIcon(state.icon) };
            }
        }
        // SAFETY: our window.
        unsafe { DestroyWindow(self.hwnd) };
    }
}

/// `WindowsTrayIcon.Update`: adds, modifies or removes the icon.
fn update(state: &mut State) {
    if !state.visible && !state.added {
        return;
    }
    // SAFETY: NOTIFYICONDATAW is plain data; zeroed is a valid start.
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = state.hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = CALLBACK_MESSAGE;
    nid.hIcon = state.icon;
    for (dst, src) in nid.szTip.iter_mut().zip(state.tooltip.encode_utf16().take(127)) {
        *dst = src;
    }
    if !state.visible {
        // SAFETY: a filled-in NOTIFYICONDATAW.
        unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
        state.added = false;
    } else if !state.icon.is_null() {
        // SAFETY: as above.
        state.added = unsafe { Shell_NotifyIconW(if state.added { NIM_MODIFY } else { NIM_ADD }, &nid) } != 0;
        if !state.added {
            eprintln!("Unable to update the Windows tray icon.");
        }
    }
}

fn dispatch(event: TrayEvent) {
    // Cloned out of the RefCell so the callback can't re-enter a borrow.
    let callback = STATE.with(|s| s.borrow().as_ref().map(|st| st.callback.clone()));
    if let Some(cb) = callback {
        cb(event);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == CALLBACK_MESSAGE {
        if wparam == 1 {
            // `OnMessage`: double clicks count as presses.
            let m = lparam as u32;
            let button = match m {
                0x201..=0x203 => Some(Button::Left),
                0x204..=0x206 => Some(Button::Right),
                0x207..=0x209 => Some(Button::Middle),
                _ => None,
            };
            if let Some(b) = button {
                if matches!(m, 0x201 | 0x203 | 0x204 | 0x206 | 0x207 | 0x209) {
                    dispatch(TrayEvent::Down(b));
                }
                if matches!(m, 0x202 | 0x205 | 0x208) {
                    dispatch(TrayEvent::Up(b));
                }
            }
        }
        return 0;
    }
    let readded = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let state = s.as_mut().filter(|st| st.taskbar_created == msg)?;
        state.added = false;
        update(state);
        Some(())
    });
    if readded.is_some() {
        return 0;
    }
    // SAFETY: unhandled messages go to the default procedure.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
