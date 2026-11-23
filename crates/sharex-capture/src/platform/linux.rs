//! Linux: what the session lets ShareXrs capture, and the monitors as winit
//! sees them. ShareX has no Linux capture at all (`docs/PLATFORMS.md`);
//! this is the "Beyond ShareX" path.
//!
//! - **X11:** xcap, through XCB (`other.rs`).
//! - **Wayland:** xcap only sees XWayland, and its Wayland code panics
//!   where the compositor lacks `zxdg_output_manager_v1` version 3 (WSLg's
//!   Weston): every `xcap::Monitor` coordinate asks libwayshot for the
//!   outputs' scale. So on Wayland the outputs are captured by the first
//!   of the compositor's `zwlr_screencopy_manager_v1` (wlroots desktops:
//!   Sway, Hyprland, labwc, river, Wayfire; silent, no permission; each
//!   output at its own scale) and the xdg-desktop-portal Screenshot API
//!   (GNOME, KDE, anything with a portal backend; it may ask the user once;
//!   one picture of the desktop, cut into its outputs). Each output's
//!   picture goes where winit has that monitor (by output name), the
//!   coordinates the overlays and regions use. With neither, every capture
//!   fails and says so once.
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

/// The monitors winit reports, with their output names (Wayland's
/// `wl_output` names, which place each output's capture).
static SCREENS: Mutex<Vec<(Option<String>, Screen)>> = Mutex::new(Vec::new());

/// Keeps the monitors winit knows, for [`remembered_screens`]. The app
/// calls it as its event loop starts and after each batch of events (cheap:
/// winit keeps the list).
pub fn remember_screens(event_loop: &winit::event_loop::ActiveEventLoop) {
    let primary = event_loop.primary_monitor();
    let mut screens: Vec<(Option<String>, Screen)> = event_loop
        .available_monitors()
        .map(|m| {
            let (p, s) = (m.position(), m.size());
            let bounds = Rect::new(p.x, p.y, s.width as i32, s.height as i32);
            let screen = Screen { bounds, work_area: bounds, scale: m.scale_factor(), primary: primary.as_ref() == Some(&m) };
            (m.name(), screen)
        })
        .filter(|(_, s)| s.bounds.w > 0 && s.bounds.h > 0)
        .collect();
    // Wayland names no primary monitor: the first is.
    if !screens.iter().any(|(_, s)| s.primary)
        && let Some((_, first)) = screens.first_mut()
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
    SCREENS.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(_, s)| *s).collect()
}

fn remembered_named_screens() -> Vec<(Option<String>, Screen)> {
    SCREENS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

// ---- X11 -------------------------------------------------------------------

/// ShareXrs's own connection to the X server (`$DISPLAY`) and its screen,
/// opened once; `None` without one (it says why once).
fn x11() -> Option<&'static (x11rb::rust_connection::RustConnection, usize)> {
    static CONNECTION: OnceLock<Option<(x11rb::rust_connection::RustConnection, usize)>> = OnceLock::new();
    CONNECTION
        .get_or_init(|| match x11rb::connect(None) {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("X11: couldn't connect to the display: {e}");
                None
            }
        })
        .as_ref()
}

/// Where the pointer is on X11 (`XQueryPointer` on the root window), in
/// desktop pixels, as winit places X11 monitors. `None` on Wayland, where a
/// client can't ask.
pub fn x11_cursor_position() -> Option<(i32, i32)> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt as _;
    if wayland() || std::env::var_os("DISPLAY").is_none_or(|d| d.is_empty()) {
        return None;
    }
    let (conn, screen) = x11()?;
    let root = conn.setup().roots.get(*screen)?.root;
    let reply = conn.query_pointer(root).ok()?.reply().ok()?;
    Some((i32::from(reply.root_x), i32::from(reply.root_y)))
}

/// `SetWindowRgn` on X11: the window's bounding and input shapes become
/// `rects` (window pixels; the X Shape extension), so what's outside them
/// is neither drawn nor clicked. Without it the recording frame's inside
/// covers what's recorded: black where no compositor blends it, and
/// x11grab records that. Nothing for a Wayland window.
pub fn set_window_shape(window: &winit::window::Window, rects: &[Rect]) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
    use x11rb::protocol::xproto::{ClipOrdering, Rectangle};

    let id = match window.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Xlib(h)) => h.window as u32,
        Ok(RawWindowHandle::Xcb(h)) => h.window.get(),
        _ => return,
    };
    // ShareXrs's own connection: the request applies to the window
    // whoever sends it.
    let Some((conn, _)) = x11() else { return };
    let clamp = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    let rectangles: Vec<Rectangle> = rects
        .iter()
        .filter(|r| r.w > 0 && r.h > 0)
        .map(|r| Rectangle { x: clamp(r.x), y: clamp(r.y), width: r.w.min(u16::MAX as i32) as u16, height: r.h.min(u16::MAX as i32) as u16 })
        .collect();
    let shaped = [SK::BOUNDING, SK::INPUT]
        .into_iter()
        .all(|kind| conn.shape_rectangles(SO::SET, kind, ClipOrdering::UNSORTED, id, 0, 0, &rectangles).is_ok());
    if !shaped || conn.flush().is_err() {
        eprintln!("couldn't set the window's shape (X Shape extension)");
    }
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

/// A Wayland output as the compositor lays it out (`xdg_output`).
#[derive(Clone, Debug, PartialEq)]
struct Output {
    name: String,
    /// Where it is in the compositor's logical (scaled) space.
    logical: Rect,
    /// Its picture's size in pixels.
    pixels: (u32, u32),
}

/// Where an output's picture goes in desktop pixels: where winit puts the
/// monitor of the same name, which is where the overlays and the region's
/// coordinates are (winit places a Wayland monitor at its logical position
/// times its integer scale, its size the mode's). Else, its logical place
/// at its own scale.
fn placement(output: &Output, screens: &[(Option<String>, Screen)]) -> Rect {
    if let Some((_, s)) = screens.iter().find(|(n, _)| n.as_deref() == Some(output.name.as_str())) {
        return s.bounds;
    }
    let scale = output.pixels.1 as f64 / output.logical.h.max(1) as f64;
    Rect::new(
        (output.logical.x as f64 * scale).round() as i32,
        (output.logical.y as f64 * scale).round() as i32,
        output.pixels.0 as i32,
        output.pixels.1 as i32,
    )
}

/// The monitors' pictures on Wayland that meet `r`, each placed where the
/// app's monitor list has it (see [`placement`]).
pub fn capture_screens(r: Rect) -> Result<Vec<(RgbaImage, Rect)>, String> {
    let c = compositor();
    let mut error = None;
    if c.screencopy && c.xdg_output_v3 {
        match guarded("wlr-screencopy", || wlr_screencopy(r)) {
            Some(Ok(shots)) => return Ok(shots),
            Some(Err(e)) => error = Some(format!("wlr-screencopy: {e}")),
            None => error = Some("wlr-screencopy failed".into()),
        }
    }
    if portal_screenshot_version().is_some() {
        match guarded("the screenshot portal", portal_screenshot) {
            Some(Ok(image)) => {
                let outputs = if c.xdg_output_v3 { guarded("listing the outputs", wayland_outputs).unwrap_or_default() } else { Vec::new() };
                return Ok(split_desktop(image, &outputs, &remembered_named_screens()));
            }
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

fn output_of(o: &libwayshot_xcap::output::OutputInfo) -> Output {
    let l = &o.logical_region.inner;
    Output {
        name: o.name.clone(),
        logical: Rect::new(l.position.x, l.position.y, l.size.width as i32, l.size.height as i32),
        pixels: (o.physical_size.width, o.physical_size.height),
    }
}

/// The outputs, through libwayshot (which needs `zxdg_output_manager_v1`
/// version 3).
fn wayland_outputs() -> Vec<Output> {
    libwayshot_xcap::WayshotConnection::new().map(|c| c.get_all_outputs().iter().map(output_of).collect()).unwrap_or_default()
}

/// Each output that meets `r` through `zwlr_screencopy_manager_v1`, at its
/// own scale (libwayshot's `screenshot_all` would scale every output to the
/// largest scale, one logical picture).
fn wlr_screencopy(r: Rect) -> Result<Vec<(RgbaImage, Rect)>, String> {
    let conn = libwayshot_xcap::WayshotConnection::new().map_err(|e| e.to_string())?;
    let screens = remembered_named_screens();
    let mut shots = Vec::new();
    for o in conn.get_all_outputs() {
        let place = placement(&output_of(o), &screens);
        if place.intersect(&r).is_empty() {
            continue;
        }
        let image = conn.screenshot_outputs(std::slice::from_ref(o), false).map_err(|e| format!("{}: {e}", o.name))?;
        shots.push((opaque(image.to_rgba8()), place));
    }
    Ok(shots)
}

/// The screen is opaque, but libwayshot keeps an `XRGB8888`/`XBGR8888`
/// frame's unused byte as alpha: XWayland windows (and other clients that
/// leave it 0) would come out transparent.
fn opaque(mut image: RgbaImage) -> RgbaImage {
    for pixel in image.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    image
}

/// The portal's one picture of the whole desktop cut into its outputs, each
/// placed as [`placement`] does. A desktop picture is the outputs' logical
/// layout at one scale (the largest, on GNOME and wlroots); without the
/// layout it's placed whole at the monitors' top left.
fn split_desktop(image: RgbaImage, outputs: &[Output], screens: &[(Option<String>, Screen)]) -> Vec<(RgbaImage, Rect)> {
    let layout = crate::geometry::combine(outputs.iter().map(|o| o.logical));
    if outputs.is_empty() || layout.w <= 0 || layout.h <= 0 {
        let origin = crate::geometry::combine(screens.iter().map(|(_, s)| s.bounds));
        let place = Rect::new(origin.x, origin.y, image.width() as i32, image.height() as i32);
        return vec![(image, place)];
    }
    let sx = image.width() as f64 / layout.w as f64;
    let sy = image.height() as f64 / layout.h as f64;
    outputs
        .iter()
        .filter_map(|o| {
            let x = ((o.logical.x - layout.x) as f64 * sx).round() as u32;
            let y = ((o.logical.y - layout.y) as f64 * sy).round() as u32;
            let w = ((o.logical.w as f64 * sx).round() as u32).min(image.width().saturating_sub(x));
            let h = ((o.logical.h as f64 * sy).round() as u32).min(image.height().saturating_sub(y));
            (w > 0 && h > 0).then(|| (image::imageops::crop_imm(&image, x, y, w, h).to_image(), placement(o, screens)))
        })
        .collect()
}

/// How long a screenshot portal may take: long enough to answer the
/// desktop's "allow screenshots" question the first time.
const PORTAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The portal's picture of the screen, without its dialog
/// (`interactive: false`); the file it saved is deleted, as xcap does. The
/// request runs on its own thread so that a portal that never answers fails
/// the capture after [`PORTAL_TIMEOUT`] instead of hanging the app (a late
/// answer's file is deleted when it comes).
fn portal_screenshot() -> Result<RgbaImage, String> {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("screenshot-portal".into())
        .spawn(move || {
            let uri = pollster::block_on(async {
                let request = ashpd::desktop::screenshot::Screenshot::request()
                    .interactive(false)
                    .modal(false)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;
                let response = request.response().map_err(|e| e.to_string())?;
                Ok::<String, String>(response.uri().as_str().to_owned())
            });
            if let Err(std::sync::mpsc::SendError(Ok(uri))) = send.send(uri)
                && let Some(path) = file_path(&uri)
            {
                let _ = std::fs::remove_file(path);
            }
        })
        .map_err(|e| e.to_string())?;
    let uri = receive.recv_timeout(PORTAL_TIMEOUT).map_err(|_| format!("no answer in {} s", PORTAL_TIMEOUT.as_secs()))??;
    let path = file_path(&uri).ok_or_else(|| format!("not a file: {uri}"))?;
    let image = image::open(&path).map(|i| opaque(i.to_rgba8())).map_err(|e| format!("{}: {e}", path.display()));
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

    fn screen(name: &str, x: i32, y: i32, w: i32, h: i32, scale: f64) -> (Option<String>, Screen) {
        let bounds = Rect::new(x, y, w, h);
        (Some(name.into()), Screen { bounds, work_area: bounds, scale, primary: false })
    }

    fn output(name: &str, logical: Rect, pixels: (u32, u32)) -> Output {
        Output { name: name.into(), logical, pixels }
    }

    /// Sway, 1920x1080 at scale 1 and 2560x1440 at scale 2 to its right,
    /// 200 logical pixels down: winit puts the second at (1920, 200) x 2.
    fn mixed() -> (Vec<Output>, Vec<(Option<String>, Screen)>) {
        let outputs = vec![
            output("HEADLESS-1", Rect::new(0, 0, 1920, 1080), (1920, 1080)),
            output("HEADLESS-2", Rect::new(1920, 200, 1280, 720), (2560, 1440)),
        ];
        let screens = vec![screen("HEADLESS-1", 0, 0, 1920, 1080, 1.0), screen("HEADLESS-2", 3840, 400, 2560, 1440, 2.0)];
        (outputs, screens)
    }

    #[test]
    fn screen_pictures_are_opaque() {
        let xrgb = RgbaImage::from_raw(2, 1, vec![10, 20, 30, 0, 40, 50, 60, 7]).unwrap();
        assert_eq!(opaque(xrgb).into_raw(), vec![10, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn outputs_go_where_winit_has_their_monitors() {
        let (outputs, screens) = mixed();
        assert_eq!(placement(&outputs[0], &screens), Rect::new(0, 0, 1920, 1080));
        assert_eq!(placement(&outputs[1], &screens), Rect::new(3840, 400, 2560, 1440));
        // Unknown to winit: its logical place at its own scale.
        assert_eq!(placement(&outputs[1], &[]), Rect::new(3840, 400, 2560, 1440));
        let fractional = output("DP-1", Rect::new(1280, 0, 1920, 1080), (2880, 1620));
        assert_eq!(placement(&fractional, &[]), Rect::new(1920, 0, 2880, 1620));
    }

    #[test]
    fn a_desktop_picture_is_cut_into_its_outputs() {
        let (outputs, screens) = mixed();
        // The logical layout (3200x1080) at the largest scale, 2: the first
        // output doubled, the second at its own pixels.
        let desktop = RgbaImage::from_fn(6400, 2160, |x, y| {
            let first = x < 3840 && y < 2160;
            image::Rgba([(x % 251) as u8, (y % 241) as u8, if first { 1 } else { 2 }, 255])
        });
        let shots = split_desktop(desktop.clone(), &outputs, &screens);
        assert_eq!(shots.len(), 2);
        assert_eq!(shots[0].1, Rect::new(0, 0, 1920, 1080));
        assert_eq!(shots[0].0.dimensions(), (3840, 2160));
        assert_eq!(shots[1].1, Rect::new(3840, 400, 2560, 1440));
        assert_eq!(shots[1].0.dimensions(), (2560, 1440));
        assert_eq!(shots[1].0.get_pixel(0, 0), desktop.get_pixel(3840, 400));
        // Placed and cropped as the app asks: the first monitor whole,
        // scaled back to its own pixels.
        let first = crate::screenshot::crop_placed(&shots, Rect::new(0, 0, 1920, 1080)).unwrap();
        assert_eq!(first.get_pixel(100, 50), desktop.get_pixel(200, 100));
        // Without the outputs' layout, the picture goes at the monitors' top left.
        let whole = split_desktop(desktop, &[], &screens);
        assert_eq!(whole[0].1, Rect::new(0, 0, 6400, 2160));
    }
}
