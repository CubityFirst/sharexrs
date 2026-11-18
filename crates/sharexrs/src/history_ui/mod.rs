//! ShareX.HistoryLib's windows (`HistoryWindow`, `ImageHistoryWindow`,
//! `StatsWindow`) over `sharex-history`, each an OS window of its own (a
//! [`Dialog`]), opened as `TaskHelpers.OpenHistory`, `OpenImageHistory` and
//! `OpenStats` open them (`HistoryIntegration`). docs/HISTORY.md maps them.

pub mod common;
pub mod history_window;
pub mod image_history_window;
pub mod stats_window;

use winit::event_loop::ActiveEventLoop;

use crate::App;
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

/// The `--render` screens of these windows.
pub fn is_render_screen(name: &str) -> bool {
    name.starts_with("history") || name.starts_with("image-history") || name.starts_with("stats")
}

/// The window a `--render` screen draws, set up in its state (`--state`).
pub fn render_dialog(name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    if name.starts_with("image-history") {
        image_history_window::render_dialog(name, o)
    } else if name.starts_with("history") {
        history_window::render_dialog(name, o)
    } else {
        stats_window::render_dialog(name, o)
    }
}

impl App {
    /// `TaskHelpers.OpenHistory`: a new History window each time.
    pub(crate) fn show_history(&mut self, event_loop: &ActiveEventLoop) {
        let w = history_window::HistoryWindow::new(&self.settings.settings);
        self.open_dialog_window(event_loop, Box::new(w));
    }

    /// `TaskHelpers.OpenImageHistory`: a new Image history window each time.
    pub(crate) fn show_image_history(&mut self, event_loop: &ActiveEventLoop) {
        let w = image_history_window::ImageHistoryWindow::new(&self.settings.settings);
        self.open_dialog_window(event_loop, Box::new(w));
    }

    /// `TaskHelpers.OpenStats`: one Stats window, brought to the front when
    /// it's open.
    pub(crate) fn show_stats(&mut self, event_loop: &ActiveEventLoop) {
        self.open_dialog_window(event_loop, Box::new(stats_window::StatsWindow::new()));
    }
}
