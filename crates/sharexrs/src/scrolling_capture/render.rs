//! `sharexrs --render scrolling-capture` (`--state`), set up as ShareX's
//! window was captured:
//!
//! - `cancelled` (the default): the selection was cancelled, no result,
//! - `options`: then Options... (the default options),
//! - `result`: a capture of the scroll test page (`--test-pattern --scroll`,
//!   420 wide) succeeded, its whole page the result,
//! - `result-options`: then Options....

use sharex_capture::scrolling::{ScrollingCaptureOptions, ScrollingCaptureStatus};

use super::window::ScrollingCaptureWindow;
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    name == "scrolling-capture"
}

pub fn render_dialog(_name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let mut w = ScrollingCaptureWindow::new(ScrollingCaptureOptions::default(), sharex_settings::TaskSettings::default());
    match o.state.as_deref().unwrap_or("cancelled") {
        "ready" => {}
        "cancelled" => w.show_cancelled(),
        "options" => {
            w.show_cancelled();
            w.open_options();
        }
        "result" => w.show_result(ScrollingCaptureStatus::Successful, crate::test_pattern::page_image(420)),
        "result-options" => {
            w.show_result(ScrollingCaptureStatus::Successful, crate::test_pattern::page_image(420));
            w.open_options();
        }
        other => return Err(format!("unknown state {other}")),
    }
    Ok(Box::new(w))
}
