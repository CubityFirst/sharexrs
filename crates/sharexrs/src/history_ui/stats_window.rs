//! `ShareX.HistoryLib/Views/StatsWindow` (stub, being ported).

use crate::dialogs::render::Options;
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub struct StatsWindow {}

impl StatsWindow {
    pub fn new() -> Self {
        Self {}
    }
}

impl Dialog for StatsWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(sharex_strings::history_lib::StatsWindow_Title()), 1060.0, 900.0, 720.0, 480.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let _ = ui;
        Vec::new()
    }

    fn single(&self) -> Option<&'static str> {
        Some("stats")
    }
}

pub fn render_dialog(_name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let _ = o;
    Ok(Box::new(StatsWindow::new()))
}
