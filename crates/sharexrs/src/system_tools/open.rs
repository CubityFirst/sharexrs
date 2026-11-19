//! Where the System tools open from (`TaskHelpers.Open*`, `ExecuteJob`,
//! `ToolsIntegration.Show*Window`): the Tools menu, their hotkeys and
//! command lines (`-ClipboardViewer`, `-InspectWindow`, `-MonitorTest`,
//! `-NetworkMonitor`, `-BorderlessWindow`), and the Make active window
//! borderless / top most hotkeys. Inspect window's picker overlays are
//! opened and answered here too.

use sharex_settings::{HotkeyType, TaskSettings};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::native::{NativeClipboard, NativeWindowOps, NativeWindows, WindowSource};
use super::{borderless_window, clipboard_viewer, inspect_window, monitor_test, network_monitor};
use crate::App;

impl App {
    /// A Tools menu entry of the System tools; false when it isn't one.
    pub(crate) fn open_system_tool(&mut self, event_loop: &ActiveEventLoop, key: &str) -> bool {
        let job = match key {
            "MainMenuBuilder_BorderlessWindow" => HotkeyType::BorderlessWindow,
            "MainMenuBuilder_ClipboardViewer" => HotkeyType::ClipboardViewer,
            "MainMenuBuilder_InspectWindow" => HotkeyType::InspectWindow,
            "MainMenuBuilder_MonitorTest" => HotkeyType::MonitorTest,
            "MainMenuBuilder_NetworkMonitor" => HotkeyType::NetworkMonitor,
            _ => return false,
        };
        let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
        self.run_system_job(event_loop, &job, &task)
    }

    /// `TaskHelpers.ExecuteJob` for the System tools' jobs; false for others.
    pub(crate) fn run_system_job(&mut self, event_loop: &ActiveEventLoop, job: &HotkeyType, task: &TaskSettings) -> bool {
        match job {
            HotkeyType::ClipboardViewer => {
                let owner = self.clipboard_owner();
                let w = clipboard_viewer::ClipboardViewerWindow::new(Box::new(NativeClipboard { owner }));
                self.open_dialog_window(event_loop, Box::new(w));
            }
            HotkeyType::InspectWindow => {
                self.open_dialog_window(event_loop, Box::new(inspect_window::InspectWindowWindow::new(Box::new(NativeWindows))));
            }
            HotkeyType::MonitorTest => self.open_dialog_window(event_loop, Box::new(monitor_test::MonitorTestWindow::new())),
            // `OpenNetworkMonitor`: the log in the Logs folder.
            HotkeyType::NetworkMonitor => {
                let log = self.settings.paths.logs_folder().join("NetworkMonitor.log");
                let w = network_monitor::NetworkMonitorWindow::new(network_monitor::icmp_prober(), Some(log));
                self.open_dialog_window(event_loop, Box::new(w));
            }
            // `OpenBorderlessWindow(taskSettings)`.
            HotkeyType::BorderlessWindow => {
                let defaults = self.settings.settings.DefaultTaskSettings.clone();
                let w = borderless_window::BorderlessWindowWindow::new(task.clone(), &defaults, Box::new(NativeWindows), Box::new(NativeWindowOps));
                self.open_dialog_window(event_loop, Box::new(w));
            }
            // `MakeActiveWindowBorderless`: the foreground window toggled.
            HotkeyType::ActiveWindowBorderless => {
                let handle = sharex_capture::platform::foreground_window();
                if handle > 0 {
                    let exclude = sharex_tools::system::borderless_window::BorderlessWindowSettings::from_json(&task.ToolsSettings.clone().unwrap_or_default().BorderlessWindowSettings).exclude_taskbar_area;
                    borderless_window::toggle_handle(&mut NativeWindowOps, handle, exclude);
                    crate::video_tools::action_sound(task);
                }
            }
            // `MakeActiveWindowTopMost`: the foreground window's top-most flipped.
            HotkeyType::ActiveWindowTopMost => {
                let handle = sharex_capture::platform::foreground_window();
                if handle > 0
                    && let Some(snap) = NativeWindows.snapshot(handle)
                {
                    NativeWindows.set_top_most(handle, !snap.top_most);
                    crate::video_tools::action_sound(task);
                }
            }
            _ => return false,
        }
        true
    }

    /// `StartPicking`: the Inspect window hidden, an overlay on each screen.
    pub(crate) fn start_window_picking(&mut self, event_loop: &ActiveEventLoop, owner: WindowId, top_level: bool) {
        self.close_window_pickers(owner);
        if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == owner) {
            d.window.window.set_visible(false);
        }
        for screen in sharex_capture::screenshot::screens() {
            let overlay = inspect_window::PickerOverlay { owner: Some(owner), top_level, bounds: screen.bounds, scale: screen.scale };
            self.open_dialog_window(event_loop, Box::new(overlay));
        }
    }

    fn close_window_pickers(&mut self, owner: WindowId) {
        self.dialogs.retain(|d| d.dialog.picker_owner() != Some(owner));
    }

    /// `CompletePicking` / `CancelPicking`: the overlays closed, the window
    /// at the pointer handed to the Inspect window, shown again.
    pub(crate) fn finish_window_picking(&mut self, owner: WindowId, picked: bool, top_level: bool) {
        self.close_window_pickers(owner);
        let handle = if picked {
            sharex_capture::platform::cursor_position().map_or(0, |(x, y)| NativeWindows.window_at_point(x, y, top_level))
        } else {
            0
        };
        if let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == owner) {
            d.window.window.set_visible(true);
            crate::window::activate(&d.window.window);
            if picked {
                d.dialog.window_picked(handle, top_level);
            }
            d.window.window.request_redraw();
        }
    }
}
