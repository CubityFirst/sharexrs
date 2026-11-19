//! The mouse highlighter: `ShareX.Tools/Tools/MouseHighlighter` —
//! `MouseHighlighterManager` (on by hand, and while recording),
//! `MouseHighlighterService` (the clicks, at 60 frames a second),
//! `MouseHighlighterMouseHook` (a low-level mouse hook on its own thread),
//! `MouseHighlighterOverlayWindow` (a click-through layered window per
//! monitor, sized to what it draws, drawn with Skia's rasteriser and
//! `UpdateLayeredWindow` as ShareX's is) and `MouseHighlighterWindow`.
//!
//! The overlays are layered windows, so recordings that capture with
//! `CAPTUREBLT` (FFmpeg's gdigrab) show them, as ShareX's.
//!
//! Outside Windows the hook and the overlays aren't ported: starting the
//! highlighter logs that.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use egui::{Id, Rect, Ui, pos2, vec2};
use serde_json::Value;
use sharex_strings::tools as t;
use sharex_tools::mouse_highlighter::{Highlighter, MouseHighlighterOptions};
use sharex_ui::controls;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;

use crate::dialogs::{Cx, Dialog, Effect, Spec};

/// `IsManuallyActive` (bit 0) and `IsRecordingActive` (bit 1), for the
/// window's button text.
static STATE: AtomicU8 = AtomicU8::new(0);

fn manual_active() -> bool {
    STATE.load(Ordering::Relaxed) & 1 != 0
}

fn recording_active() -> bool {
    STATE.load(Ordering::Relaxed) & 2 != 0
}

/// `MouseHighlighterManager`.
#[derive(Default)]
pub struct Manager {
    manual: Option<MouseHighlighterOptions>,
    recording: Option<MouseHighlighterOptions>,
    service: Option<Service>,
}

impl Manager {
    /// `ApplyState`: the recording's options win over the manual ones.
    fn apply(&mut self) -> Result<(), String> {
        STATE.store(u8::from(self.manual.is_some()) | (u8::from(self.recording.is_some()) << 1), Ordering::Relaxed);
        let options = self.recording.clone().or(self.manual.clone());
        match (options, self.service.as_mut()) {
            (None, _) => self.service = None,
            (Some(o), None) => self.service = Some(Service::new(o)?),
            (Some(o), Some(s)) => {
                if s.highlighter.options != {
                    let mut v = o.clone();
                    v.validate();
                    v
                } {
                    s.highlighter.update_options(o);
                }
            }
        }
        Ok(())
    }

    /// `SetManualActive`.
    pub fn set_manual(&mut self, active: bool, options: MouseHighlighterOptions) -> Result<(), String> {
        let previous = self.manual.take();
        self.manual = active.then_some(options);
        if let Err(e) = self.apply() {
            self.manual = previous;
            STATE.store(u8::from(self.manual.is_some()) | (u8::from(self.recording.is_some()) << 1), Ordering::Relaxed);
            return Err(e);
        }
        Ok(())
    }

    pub fn is_manual(&self) -> bool {
        self.manual.is_some()
    }

    /// `RefreshOptions`: a running highlighter on these options takes them.
    pub fn refresh(&mut self, options: MouseHighlighterOptions) {
        if self.manual.is_some() {
            self.manual = Some(options.clone());
        }
        if self.recording.is_none()
            && let Some(s) = self.service.as_mut()
        {
            s.highlighter.update_options(options);
        }
    }

    /// `BeginRecordingAsync`.
    pub fn begin_recording(&mut self, options: MouseHighlighterOptions) -> Result<(), String> {
        self.recording = Some(options);
        let r = self.apply();
        if r.is_err() {
            self.recording = None;
            let _ = self.apply();
        }
        r
    }

    /// The recording lease disposed.
    pub fn end_recording(&mut self) {
        if self.recording.take().is_some() {
            let _ = self.apply();
        }
    }

    /// `Shutdown`.
    pub fn shutdown(&mut self) {
        self.manual = None;
        self.recording = None;
        self.service = None;
        STATE.store(0, Ordering::Relaxed);
    }

    /// `OnTick` while running: when to tick next.
    pub fn tick(&mut self, now: Instant) -> Option<Instant> {
        let s = self.service.as_mut()?;
        if now >= s.next_tick {
            if let Err(e) = s.tick() {
                // `StopOnError`.
                eprintln!("Mouse highlighting failed. {e}");
                self.shutdown();
                return None;
            }
            s.next_tick = now + Duration::from_micros(16_667);
        }
        Some(s.next_tick)
    }
}

/// `MouseHighlighterService`.
#[cfg_attr(not(windows), allow(dead_code))]
struct Service {
    highlighter: Highlighter,
    started: Instant,
    next_tick: Instant,
    #[cfg(windows)]
    hook: win::Hook,
    #[cfg(windows)]
    overlays: Vec<win::Overlay>,
}

impl Service {
    fn new(options: MouseHighlighterOptions) -> Result<Self, String> {
        let cursor = sharex_capture::platform::cursor_position().unwrap_or_default();
        #[cfg(windows)]
        {
            let hook = win::Hook::start(cursor)?;
            let overlays = sharex_capture::screenshot::screens().iter().map(|s| win::Overlay::new(s.bounds)).collect::<Result<Vec<_>, _>>()?;
            let now = Instant::now();
            Ok(Self { highlighter: Highlighter::new(options, cursor), started: now, next_tick: now, hook, overlays })
        }
        #[cfg(not(windows))]
        {
            let _ = (options, cursor);
            Err("the mouse highlighter isn't ported to this platform yet".into())
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn time(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }

    /// `ProcessPendingInput`, the clean-up and the overlays' `Refresh`.
    fn tick(&mut self) -> Result<(), String> {
        #[cfg(windows)]
        {
            if let Some(e) = self.hook.failure() {
                return Err(e);
            }
            let (events, position, overflow, pressed) = self.hook.drain();
            if overflow {
                self.highlighter.reset(position, pressed, self.time());
            }
            for e in events {
                self.highlighter.move_cursor(e.position);
                let time = e.at.saturating_duration_since(self.started).as_secs_f64() * 1000.0;
                if e.pressed {
                    self.highlighter.press(e.button, e.position, time);
                } else {
                    self.highlighter.release(e.button, e.position, time);
                }
            }
            self.highlighter.move_cursor(position);
            let time = self.time();
            self.highlighter.tick(time);
            for o in &mut self.overlays {
                o.refresh(&self.highlighter, time)?;
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU8, Ordering};
    use std::time::Instant;

    use sharex_capture::Rect;
    use sharex_tools::mouse_highlighter::{Button, Highlighter};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, GdiFlush, HBITMAP, HDC, HGDIOBJ, SelectObject,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, GetSystemMetrics, HWND_TOPMOST, MSG, MSLLHOOKSTRUCT,
        PeekMessageW, PostThreadMessageW, RegisterClassW, SM_SWAPBUTTON, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, SetWindowsHookExW, ShowWindow,
        TranslateMessage, ULW_ALPHA, UnhookWindowsHookEx, UpdateLayeredWindow, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
        WM_NCHITTEST, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_EX_TRANSPARENT, WS_POPUP,
    };

    /// `MouseHighlighterButtonEvent`.
    pub struct ButtonEvent {
        pub button: Button,
        pub pressed: bool,
        pub position: (i32, i32),
        pub at: Instant,
    }

    /// `MouseHighlighterInputBuffer`, shared with the hook's callback.
    static POSITION: AtomicI64 = AtomicI64::new(0);
    static PRESSED: AtomicU8 = AtomicU8::new(0);
    static OVERFLOW: AtomicBool = AtomicBool::new(false);
    static EVENTS: Mutex<Vec<ButtonEvent>> = Mutex::new(Vec::new());
    const CAPACITY: usize = 256;

    fn pack((x, y): (i32, i32)) -> i64 {
        ((x as i64) << 32) | (y as u32 as i64)
    }

    fn unpack(v: i64) -> (i32, i32) {
        ((v >> 32) as i32, v as i32)
    }

    unsafe extern "system" fn on_mouse(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            // SAFETY: the system passes an MSLLHOOKSTRUCT for WH_MOUSE_LL.
            let info = unsafe { &*(l as *const MSLLHOOKSTRUCT) };
            let p = (info.pt.x, info.pt.y);
            POSITION.store(pack(p), Ordering::Relaxed);
            let msg = w as u32;
            if matches!(msg, WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP) {
                // SAFETY: a plain query.
                let swapped = unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0;
                let right = matches!(msg, WM_RBUTTONDOWN | WM_RBUTTONUP);
                let button = if matches!(msg, WM_MBUTTONDOWN | WM_MBUTTONUP) {
                    Button::Middle
                } else if right != swapped {
                    Button::Secondary
                } else {
                    Button::Primary
                };
                let pressed = matches!(msg, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN);
                let bit = 1u8 << button as u8;
                if pressed {
                    PRESSED.fetch_or(bit, Ordering::Relaxed);
                } else {
                    PRESSED.fetch_and(!bit, Ordering::Relaxed);
                }
                // Never wait on the UI thread: a busy queue is an overflow.
                match EVENTS.try_lock() {
                    Ok(mut q) if q.len() < CAPACITY => q.push(ButtonEvent { button, pressed, position: p, at: Instant::now() }),
                    _ => OVERFLOW.store(true, Ordering::Relaxed),
                }
            }
        }
        // SAFETY: passes the event on.
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, w, l) }
    }

    /// `MouseHighlighterMouseHook`: `WH_MOUSE_LL` on its own thread.
    pub struct Hook {
        thread: Option<std::thread::JoinHandle<()>>,
        thread_id: u32,
        failure: std::sync::Arc<Mutex<Option<String>>>,
    }

    impl Hook {
        pub fn start(cursor: (i32, i32)) -> Result<Self, String> {
            POSITION.store(pack(cursor), Ordering::Relaxed);
            PRESSED.store(0, Ordering::Relaxed);
            OVERFLOW.store(false, Ordering::Relaxed);
            EVENTS.lock().map_err(|e| e.to_string())?.clear();
            let (tx, rx) = std::sync::mpsc::channel::<Result<u32, String>>();
            let failure = std::sync::Arc::new(Mutex::new(None));
            let fail = failure.clone();
            let thread = std::thread::Builder::new()
                .name("ShareXrs mouse highlighter input".into())
                .spawn(move || {
                    // SAFETY: a hook owned and removed by this thread; the
                    // message queue exists before the id is reported.
                    unsafe {
                        let mut msg: MSG = std::mem::zeroed();
                        PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, 0);
                        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(on_mouse), GetModuleHandleW(std::ptr::null()), 0);
                        if hook.is_null() {
                            let _ = tx.send(Err(std::io::Error::last_os_error().to_string()));
                            return;
                        }
                        let _ = tx.send(Ok(GetCurrentThreadId()));
                        loop {
                            let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                            if r == -1 {
                                *fail.lock().unwrap() = Some(std::io::Error::last_os_error().to_string());
                                break;
                            }
                            if r == 0 {
                                break;
                            }
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                        UnhookWindowsHookEx(hook);
                    }
                })
                .map_err(|e| e.to_string())?;
            let thread_id = rx.recv().map_err(|e| e.to_string())??;
            Ok(Self { thread: Some(thread), thread_id, failure })
        }

        pub fn failure(&self) -> Option<String> {
            self.failure.lock().ok().and_then(|f| f.clone())
        }

        /// The button events so far, the position, whether the buffer
        /// overflowed and the buttons held.
        pub fn drain(&self) -> (Vec<ButtonEvent>, (i32, i32), bool, u8) {
            let overflow = OVERFLOW.swap(false, Ordering::Relaxed);
            let events = if overflow {
                // `DiscardPendingEvents`.
                EVENTS.lock().map(|mut q| {
                    q.clear();
                    Vec::new()
                })
            } else {
                EVENTS.lock().map(|mut q| std::mem::take(&mut *q))
            }
            .unwrap_or_default();
            (events, unpack(POSITION.load(Ordering::Relaxed)), overflow, PRESSED.load(Ordering::Relaxed))
        }
    }

    impl Drop for Hook {
        fn drop(&mut self) {
            // SAFETY: WM_QUIT to the hook's own thread.
            unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    unsafe extern "system" fn overlay_proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if msg == WM_NCHITTEST {
            return -1; // HTTRANSPARENT
        }
        if msg == 0x0021 {
            return 3; // WM_MOUSEACTIVATE -> MA_NOACTIVATE
        }
        // SAFETY: the default procedure.
        unsafe { DefWindowProcW(h, msg, w, l) }
    }

    fn class_name() -> Vec<u16> {
        "ShareXrsMouseHighlighterOverlay\0".encode_utf16().collect()
    }

    /// `MouseHighlighterOverlayWindow`.
    pub struct Overlay {
        hwnd: HWND,
        screen: Rect,
        dc: HDC,
        bitmap: HBITMAP,
        previous: HGDIOBJ,
        bits: *mut u8,
        buffer: (i32, i32),
        visible: bool,
    }

    impl Overlay {
        pub fn new(screen: Rect) -> Result<Self, String> {
            let class = class_name();
            let title: Vec<u16> = "ShareXrs - Mouse highlighter overlay\0".encode_utf16().collect();
            // SAFETY: a class registered once (a second registration fails
            // harmlessly) and a window of ours, destroyed in Drop.
            let hwnd = unsafe {
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(overlay_proc),
                    hInstance: GetModuleHandleW(std::ptr::null()),
                    lpszClassName: class.as_ptr(),
                    ..std::mem::zeroed()
                };
                RegisterClassW(&wc);
                CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                    class.as_ptr(),
                    title.as_ptr(),
                    WS_POPUP,
                    screen.x,
                    screen.y,
                    1,
                    1,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                )
            };
            if hwnd.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(Self { hwnd, screen, dc: std::ptr::null_mut(), bitmap: std::ptr::null_mut(), previous: std::ptr::null_mut(), bits: std::ptr::null_mut(), buffer: (0, 0), visible: false })
        }

        /// `EnsureBuffer`: capacity rounded up to 64 pixels, reused while
        /// the effect is at least a quarter of it.
        fn ensure_buffer(&mut self, w: i32, h: i32) -> Result<(), String> {
            let (bw, bh) = self.buffer;
            if !self.bitmap.is_null() && w <= bw && h <= bh && w >= bw / 4 && h >= bh / 4 {
                return Ok(());
            }
            self.release_buffer();
            let (bw, bh) = ((w + 63) / 64 * 64, (h + 63) / 64 * 64);
            // SAFETY: GDI objects owned by the overlay, freed in release_buffer.
            unsafe {
                let dc = CreateCompatibleDC(std::ptr::null_mut());
                let mut bi: BITMAPINFO = std::mem::zeroed();
                bi.bmiHeader = BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: bw,
                    biHeight: -bh,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..std::mem::zeroed()
                };
                let mut bits: *mut c_void = std::ptr::null_mut();
                let bitmap = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
                if dc.is_null() || bitmap.is_null() || bits.is_null() {
                    if !dc.is_null() {
                        DeleteDC(dc);
                    }
                    return Err("Could not allocate the mouse highlighter bitmap.".into());
                }
                self.previous = SelectObject(dc, bitmap);
                self.dc = dc;
                self.bitmap = bitmap;
                self.bits = bits.cast();
            }
            self.buffer = (bw, bh);
            Ok(())
        }

        fn release_buffer(&mut self) {
            if self.bitmap.is_null() {
                return;
            }
            // SAFETY: created in ensure_buffer.
            unsafe {
                SelectObject(self.dc, self.previous);
                DeleteObject(self.bitmap);
                DeleteDC(self.dc);
            }
            self.bitmap = std::ptr::null_mut();
            self.dc = std::ptr::null_mut();
            self.bits = std::ptr::null_mut();
            self.buffer = (0, 0);
        }

        /// `Refresh`: draws what's on this monitor, or hides.
        pub fn refresh(&mut self, h: &Highlighter, time: f64) -> Result<(), String> {
            let s = self.screen;
            let Some((x, y, w, hh)) = h.effect_bounds((s.x, s.y, s.w, s.h)) else {
                if self.visible {
                    // SAFETY: our window.
                    unsafe { ShowWindow(self.hwnd, SW_HIDE) };
                    self.visible = false;
                }
                return Ok(());
            };
            self.ensure_buffer(w, hh)?;
            let (bw, bh) = self.buffer;
            let mut pm = sharex_skia::Pixmap::new(bw as u32, bh as u32);
            h.draw(&mut pm, (x, y), time);
            // SAFETY: the DIB section is bw*bh BGRA pixels; Skia's N32 is
            // premultiplied BGRA in memory, as UpdateLayeredWindow wants.
            unsafe {
                let dst = std::slice::from_raw_parts_mut(self.bits as *mut u32, (bw * bh) as usize);
                dst.copy_from_slice(pm.pixels());
                GdiFlush();
                let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
                let at = POINT { x, y };
                let size = SIZE { cx: w, cy: hh };
                let origin = POINT { x: 0, y: 0 };
                if UpdateLayeredWindow(self.hwnd, std::ptr::null_mut(), &at, &size, self.dc, &origin, 0, &blend, ULW_ALPHA) == 0 {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                if !self.visible {
                    SetWindowPos(self.hwnd, HWND_TOPMOST, x, y, w, hh, SWP_NOACTIVATE | SWP_SHOWWINDOW);
                    self.visible = true;
                }
            }
            Ok(())
        }
    }

    impl Drop for Overlay {
        fn drop(&mut self) {
            self.release_buffer();
            // SAFETY: our window.
            unsafe { DestroyWindow(self.hwnd) };
        }
    }
}

/// `MouseHighlighterWindow`: Start/Stop, the recording tip and the settings.
pub struct HighlighterWindow {
    options: MouseHighlighterOptions,
    scroll: f32,
    settings: super::highlighter_settings::SettingsControl,
}

impl HighlighterWindow {
    pub fn new(options: MouseHighlighterOptions) -> Self {
        let mut options = options;
        options.validate();
        Self { options, scroll: 0.0, settings: super::highlighter_settings::SettingsControl::new("mh") }
    }

    /// `RefreshState`.
    fn toggle_text() -> &'static str {
        match (recording_active(), manual_active()) {
            (true, true) => t::MouseHighlighter_StopAfterRecording(),
            (true, false) => t::MouseHighlighter_KeepAfterRecording(),
            (false, true) => t::MouseHighlighter_Stop(),
            (false, false) => t::MouseHighlighter_Start(),
        }
    }
}

use super::highlighter_settings::wrapped;

impl Dialog for HighlighterWindow {
    fn spec(&self) -> Spec {
        Spec::sized(format!("ShareXrs - {}", t::MouseHighlighter_Title()), 620.0, 690.0, 520.0, 460.0)
    }

    /// `MouseHighlighterManager.ShowWindow`: one window.
    fn single(&self) -> Option<&'static str> {
        Some("mouse-highlighter")
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        // The `ScrollViewer`: the wheel scrolls the content.
        let wheel = ui.input(|i| i.smooth_scroll_delta.y);
        let content_h = 20.0 + 40.0 + 14.0 + 61.0 + 14.0 + super::highlighter_settings::SettingsControl::height(ui, &self.options, full.width() - 40.0) + 20.0;
        self.scroll = (self.scroll - wheel).clamp(0.0, (content_h - full.height()).max(0.0));
        let x0 = full.min.x + 20.0;
        let w = full.width() - 40.0;
        let mut y = full.min.y + 20.0 - self.scroll;
        let mut effects = Vec::new();
        let mut changed = false;
        // Start / Stop.
        let text = Self::toggle_text();
        let bw = controls::Button::default().width(ui, text);
        if controls::Button::default().show(ui, Rect::from_min_size(pos2(x0, y), vec2(bw, 40.0)), Id::new("mh-toggle"), text, true).clicked() {
            effects.push(Effect::HighlighterToggle(Box::new(self.options.clone())));
        }
        y += 40.0 + 14.0;
        // The recording tip.
        let tip = Rect::from_min_size(pos2(x0, y), vec2(w, 61.0));
        sharex_ui::border::border(ui.painter(), tip, 6.0, 1.0, Some(p.background_panel.into()), Some(p.border));
        sharex_ui::icons::paint(ui.painter(), Rect::from_min_size(tip.min + vec2(13.0, 13.0), vec2(18.0, 18.0)), icon::Info, p.accent_icon);
        wrapped(ui, tip.min + vec2(13.0 + 18.0 + 10.0, 13.0), w - 26.0 - 28.0, t::MouseHighlighter_RecordingHelp(), p.text_secondary);
        y += 61.0 + 14.0;
        let o = &mut self.options;
        changed |= self.settings.ui(ui, x0, y, w, o);
        if changed {
            // `Changed()`: validated, the running highlighter refreshed, saved.
            o.validate();
            effects.push(Effect::HighlighterOptions(Box::new(o.clone())));
        }
        // The `ScrollViewer`'s collapsed bar.
        crate::dialogs::layout::scroll_bar(ui, full, content_h, self.scroll, 16.0);
        if wheel != 0.0 {
            ui.ctx().request_repaint();
        }
        effects
    }
}

/// `MouseHighlighterOptions` of the default task settings.
pub fn options_of(task: &sharex_settings::TaskSettings) -> MouseHighlighterOptions {
    let v = task.ToolsSettings.as_ref().map(|t| t.MouseHighlighterOptions.clone()).unwrap_or(Value::Null);
    MouseHighlighterOptions::from_json(&v)
}
