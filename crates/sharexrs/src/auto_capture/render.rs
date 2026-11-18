//! `sharexrs --render auto-capture` (`--state`), set up as ShareX's window
//! was captured (default settings):
//!
//! - `rest` (the default): no region selected,
//! - `fullscreen`: Fullscreen chosen (the screen's bounds),
//! - `running`: then Start with minimize to tray off, the first capture
//!   taken 1 s after Start and the status 3.5 s after that.

use std::time::{Duration, Instant};

use super::window::AutoCaptureWindow;
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    name == "auto-capture"
}

/// The settings the render runs with.
pub fn settings(o: &Options) -> sharex_settings::ApplicationConfig {
    let mut s = sharex_settings::ApplicationConfig::default();
    if o.state.as_deref() == Some("running") {
        s.AutoCaptureMinimizeToTray = false;
    }
    s
}

pub fn render_dialog(_name: &str, o: &Options, settings: &mut sharex_settings::ApplicationConfig) -> Result<Box<dyn Dialog>, String> {
    let mut w = AutoCaptureWindow::new(sharex_settings::TaskSettings::default(), settings);
    match o.state.as_deref().unwrap_or("rest") {
        "rest" => {}
        "fullscreen" => w.choose_fullscreen(settings),
        "running" => {
            w.choose_fullscreen(settings);
            let t0 = Instant::now() - Duration::from_millis(3400);
            w.timer.start(t0, settings.AutoCaptureRepeatTime, settings.AutoCaptureWaitUpload);
            w.timer.tick(t0 + Duration::from_secs(1), false);
            // The status as the 250 ms timer last showed it (ShareX's shot
            // read "56.5 s (5%)").
            w.update_status(t0 + Duration::from_millis(4500));
        }
        other => return Err(format!("unknown state {other}")),
    }
    Ok(Box::new(w))
}
