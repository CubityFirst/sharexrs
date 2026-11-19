//! Where the System tools open from.

use sharex_settings::{HotkeyType, TaskSettings};
use winit::event_loop::ActiveEventLoop;

use crate::App;

impl App {
    pub(crate) fn open_system_tool(&mut self, _event_loop: &ActiveEventLoop, _key: &str) -> bool {
        false
    }

    pub(crate) fn run_system_job(&mut self, _event_loop: &ActiveEventLoop, _job: &HotkeyType, _task: &TaskSettings) -> bool {
        false
    }
}
