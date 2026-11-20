//! Linux: what the session lets ShareXrs capture, and the monitors as winit
//! sees them. ShareX has no Linux capture at all (`docs/PLATFORMS.md`);
//! this is the "Beyond ShareX" path.
//!
//! - **X11:** xcap, through XCB (`other.rs`).
//! - **Wayland:** xcap only sees XWayland, and its Wayland code panics
//!   where the compositor lacks `zxdg_output_manager_v1` version 3 (WSLg's
//!   Weston): every `xcap::Monitor` coordinate asks libwayshot for the
//!   outputs' scale. So on Wayland the desktop is captured whole, by the
//!   first of the compositor's `zwlr_screencopy_manager_v1` (wlroots
//!   desktops: Sway, Hyprland, labwc, river, Wayfire; silent, no
//!   permission) and the xdg-desktop-portal Screenshot API (GNOME, KDE,
//!   anything with a portal backend; it may ask the user once). With
//!   neither, every capture fails and says so once.
//! - **Monitors** come from winit (`remember_screens`), not from a capture
//!   library, so opening windows never depends on one.

use std::sync::{Mutex, Once, OnceLock};

use image::RgbaImage;

use super::guarded;
use crate::Screen;
use crate::geometry::Rect;

/// As xcap decides whether to use its Wayland code.
pub fn wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland")
        || std::env::var("WAYLAND_DISPLAY").is_ok_and(|d| d.to_lowercase().contains("wayland"))
}

// ---- monitors ---------------------------------------------------------------

static SCREENS: Mutex<Vec<Screen>> = Mutex::new(Vec::new());

/// Keeps the monitors winit knows, for [`remembered_screens`]. The app
/// calls it as its event loop starts and after each batch of events (cheap:
/// winit keeps the list).
pub fn remember_screens(event_loop: &winit::event_loop::ActiveEventLoop) {
    let primary = event_loop.primary_monitor();
    let mut screens: Vec<Screen> = event_loop
        .available_monitors()
        .map(|m| {
            let (p, s) = (m.position(), m.size());
            let bounds = Rect::new(p.x, p.y, s.width as i32, s.height as i32);
            Screen { bounds, work_area: bounds, scale: m.scale_factor(), primary: primary.as_ref() == Some(&m) }
        })
        .filter(|s| s.bounds.w > 0 && s.bounds.h > 0)
        .collect();
    // Wayland names no primary monitor: the first is.
    if !screens.iter().any(|s| s.primary)
        && let Some(first) = screens.first_mut()
    {
        first.primary = true;
    }
    let mut kept = SCREENS.lock().unwrap_or_else(|e| e.into_inner());
    if *kept != screens {
        *kept = screens;
    }
}

/// The monitors winit last reported; empty before the event loop starts
/// (offscreen renders, tests).
pub fn remembered_screens() -> Vec<Screen> {
    SCREENS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

// ---- what the Wayland compositor offers ------------------------------------

/// The compositor's globals that decide what can capture.
#[derive(Clone, Copy, Debug, Default)]
pub struct Compositor {
    /// `zwlr_screencopy_manager_v1`.
    pub screencopy: bool,
    /// `zxdg_output_manager_v1` at version 3, which libwayshot (and so
    /// xcap's monitor coordinates) needs or panics.
    pub xdg_output_v3: bool,
}

/// The Wayland compositor's capture globals, asked once.
pub fn compositor() -> Compositor {
    static C: OnceLock<Compositor> = OnceLock::new();
    *C.get_or_init(|| if wayland() { probe_compositor().unwrap_or_default() } else { Compositor::default() })
}

fn probe_compositor() -> Option<Compositor> {
    use wayland_client::globals::{GlobalListContents, registry_queue_init};
    use wayland_client::protocol::wl_registry::{self, WlRegistry};
    use wayland_client::{Connection, Dispatch, QueueHandle};

    struct Probe;
    impl Dispatch<WlRegistry, GlobalListContents> for Probe {
        fn event(_: &mut Self, _: &WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
    }
    let conn = Connection::connect_to_env().ok()?;
    let (globals, _queue) = registry_queue_init::<Probe>(&conn).ok()?;
    Some(globals.contents().with_list(|list| Compositor {
        screencopy: list.iter().any(|g| g.interface == "zwlr_screencopy_manager_v1"),
        xdg_output_v3: list.iter().any(|g| g.interface == "zxdg_output_manager_v1" && g.version >= 3),
    }))
}

/// Whether the xdg-desktop-portal offers Screenshot (its version), asked
/// once. D-Bus activation of a portal can take long (25 s when a backend
/// hangs), so the app asks on a thread at startup ([`probe_in_background`]).
fn portal_screenshot_version() -> Option<u32> {
    static V: OnceLock<Option<u32>> = OnceLock::new();
    *V.get_or_init(|| {
        if !wayland() {
            return None;
        }
        guarded("asking the screenshot portal", || {
            pollster::block_on(async {
                use ashpd::zbus;
                let conn = zbus::Connection::session().await.ok()?;
                let proxy = zbus::Proxy::new(
                    &conn,
                    "org.freedesktop.portal.Desktop",
                    "/org/freedesktop/portal/desktop",
                    "org.freedesktop.portal.Screenshot",
                )
                .await
                .ok()?;
                proxy.get_property::<u32>("version").await.ok()
            })
        })
        .flatten()
    })
}

/// Finds out, off the event loop's thread, what can capture on Wayland, so
/// the first capture doesn't wait for the portal to start.
pub fn probe_in_background() {
    if !wayland() {
        return;
    }
    let _ = std::thread::Builder::new().name("capture-probe".into()).spawn(|| {
        let c = compositor();
        let portal = portal_screenshot_version();
        eprintln!(
            "Wayland screen capture: wlr-screencopy {}, screenshot portal {}.",
            if c.screencopy { "yes" } else { "no" },
            portal.map_or("no".to_string(), |v| format!("version {v}")),
        );
        if !c.screencopy && portal.is_none() {
            unavailable();
        }
    });
}

/// Says once that nothing here can capture.
fn unavailable() {
    static SAID: Once = Once::new();
    SAID.call_once(|| {
        eprintln!(
            "Screen capture is unavailable on this Wayland compositor (no wlr-screencopy, no xdg-desktop-portal Screenshot)."
        );
    });
}

// ---- capturing on Wayland --------------------------------------------------

/// Whether a grab can happen without the user seeing it (a screenshot
/// portal may flash the screen or ask): X11, or Wayland with screencopy.
pub fn quiet_grab() -> bool {
    !wayland() || compositor().screencopy
}

/// The whole desktop on Wayland, its top left at the monitors' top left.
pub fn capture_desktop() -> Result<RgbaImage, String> {
    let c = compositor();
    let mut error = None;
    if c.screencopy && c.xdg_output_v3 {
        match guarded("wlr-screencopy", wlr_screencopy) {
            Some(Ok(image)) => return Ok(image),
            Some(Err(e)) => error = Some(format!("wlr-screencopy: {e}")),
            None => error = Some("wlr-screencopy failed".into()),
        }
    }
    if portal_screenshot_version().is_some() {
        match guarded("the screenshot portal", portal_screenshot) {
            Some(Ok(image)) => return Ok(image),
            Some(Err(e)) => error = Some(format!("screenshot portal: {e}")),
            None => error = Some("screenshot portal failed".into()),
        }
    }
    match error {
        Some(e) => Err(e),
        None => {
            unavailable();
            Err("screen capture is unavailable on this Wayland compositor".into())
        }
    }
}

/// Every output through `zwlr_screencopy_manager_v1`, laid out as the
/// compositor lays them out.
fn wlr_screencopy() -> Result<RgbaImage, String> {
    let conn = libwayshot_xcap::WayshotConnection::new().map_err(|e| e.to_string())?;
    let image = conn.screenshot_all(false).map_err(|e| e.to_string())?;
    Ok(image.to_rgba8())
}

/// The portal's picture of the screen, without its dialog
/// (`interactive: false`); the file it saved is deleted, as xcap does.
fn portal_screenshot() -> Result<RgbaImage, String> {
    let uri = pollster::block_on(async {
        let request = ashpd::desktop::screenshot::Screenshot::request()
            .interactive(false)
            .modal(false)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let response = request.response().map_err(|e| e.to_string())?;
        Ok::<String, String>(response.uri().as_str().to_owned())
    })?;
    let path = file_path(&uri).ok_or_else(|| format!("not a file: {uri}"))?;
    let image = image::open(&path).map(|i| i.to_rgba8()).map_err(|e| format!("{}: {e}", path.display()));
    let _ = std::fs::remove_file(&path);
    image
}

/// The file a `file://` URI names (percent-decoded).
fn file_path(uri: &str) -> Option<std::path::PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let path = uri.strip_prefix("file://")?.as_bytes();
    let mut bytes = Vec::with_capacity(path.len());
    let mut i = 0;
    while i < path.len() {
        let hex = (path[i] == b'%')
            .then(|| path.get(i + 1..i + 3))
            .flatten()
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(b) => {
                bytes.push(b);
                i += 3;
            }
            None => {
                bytes.push(path[i]);
                i += 1;
            }
        }
    }
    Some(std::ffi::OsString::from_vec(bytes).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uris_are_decoded() {
        assert_eq!(file_path("file:///tmp/a%20b.png"), Some("/tmp/a b.png".into()));
        assert_eq!(file_path("file:///tmp/100%.png"), Some("/tmp/100%.png".into()));
        assert_eq!(file_path("https://x/y.png"), None);
    }
}
