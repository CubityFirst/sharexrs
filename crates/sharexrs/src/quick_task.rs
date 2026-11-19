//! `ShareX/QuickTaskMenu.cs`: with `ShowQuickTaskMenu`, a capture waits on
//! a menu at the pointer: Continue (the task as it is), the presets
//! (`ApplicationConfig.QuickTaskPresets`, each replacing the after capture
//! and after upload tasks), Edit this menu..., Cancel.

use sharex_settings::{AfterCaptureTasks, AfterUploadTasks, QuickTaskInfo};
use sharex_strings::sharex as s;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::MenuEntry;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::App;
use crate::capture_job::ImageTask;
use crate::tray_menu::TrayMenu;

/// What a quick task menu item does.
#[derive(Clone, Debug, PartialEq)]
pub enum QuickCmd {
    Continue,
    Preset(usize),
    Edit,
    Cancel,
}

/// The open menu and the capture it holds.
pub struct QuickTaskMenu {
    menu: TrayMenu<QuickCmd>,
    job: Box<ImageTask>,
}

fn flag_description(enum_type: &str, bit: u64, names: &[(u64, &'static str)]) -> String {
    let name = names.iter().find(|(b, _)| *b == bit).map_or("", |(_, n)| *n);
    sharex_strings::helpers_lib::lookup(&format!("{enum_type}_{name}")).unwrap_or(name).to_owned()
}

/// `QuickTaskInfo.ToString()`: its name, or its tasks' descriptions (the
/// after upload ones too when it uploads).
pub fn display_name(info: &QuickTaskInfo) -> String {
    if let Some(name) = info.Name.as_deref().filter(|n| !n.is_empty()) {
        return name.to_owned();
    }
    let flags = |value: u64, names: &[(u64, &'static str)], t: &str| -> Vec<String> {
        names.iter().filter(|(b, _)| *b != 0 && value & b == *b).map(|(b, _)| flag_description(t, *b, names)).collect()
    };
    let mut result = flags(info.AfterCaptureTasks.0 as u64, AfterCaptureTasks::NAMES, "AfterCaptureTasks").join(", ");
    if info.AfterCaptureTasks.contains(AfterCaptureTasks::UploadImageToHost) {
        let upload = flags(info.AfterUploadTasks.0 as u64, AfterUploadTasks::NAMES, "AfterUploadTasks");
        if !upload.is_empty() {
            result.push_str(", ");
            result.push_str(&upload.join(", "));
        }
    }
    result
}

/// `MainMenuBuilder.GetAfterCaptureTaskIcon(first flag)`.
pub fn preset_icon(info: &QuickTaskInfo) -> &'static str {
    let first = AfterCaptureTasks::NAMES.iter().find(|(b, _)| *b != 0 && info.AfterCaptureTasks.0 as u64 & b == *b).map(|(_, n)| *n);
    first
        .and_then(|n| crate::main_menu::AFTER_CAPTURE_TASKS.iter().find(|(t, _)| *t == n).map(|(_, i)| *i))
        .unwrap_or(icon::CircleDashed)
}

/// `ShowMenuCore`'s items.
pub fn entries(presets: &[QuickTaskInfo]) -> Vec<MenuEntry<QuickCmd>> {
    let mut items = vec![MenuEntry::item(s::QuickTaskMenu_ShowMenu_Continue(), icon::CirclePlay, QuickCmd::Continue), MenuEntry::separator()];
    if !presets.is_empty() {
        for (i, p) in presets.iter().enumerate() {
            if p.is_valid() {
                items.push(MenuEntry::item(display_name(p), preset_icon(p), QuickCmd::Preset(i)));
            } else {
                items.push(MenuEntry::separator());
            }
        }
        items.push(MenuEntry::separator());
    }
    items.push(MenuEntry::item(s::QuickTaskMenu_ShowMenu_Edit_this_menu___(), icon::Edit, QuickCmd::Edit));
    items.push(MenuEntry::separator());
    items.push(MenuEntry::item(s::QuickTaskMenu_ShowMenu_Cancel(), icon::Close, QuickCmd::Cancel));
    items
}

impl App {
    /// `QuickTaskMenu.ShowMenu()` for a capture.
    pub(crate) fn show_quick_task_menu(&mut self, event_loop: &ActiveEventLoop, job: ImageTask) {
        self.quick_task = None;
        let gpu = match &self.gpu {
            Some(g) => g,
            None => match sharex_ui::gpu::Gpu::new() {
                Ok(g) => self.gpu.insert(g),
                Err(e) => {
                    eprintln!("couldn't open the quick task menu: {e}");
                    return;
                }
            },
        };
        let screens = sharex_capture::screenshot::screens();
        let cursor = sharex_capture::screenshot::cursor_position().unwrap_or((0, 0));
        let items = entries(&self.settings.settings.QuickTaskPresets);
        match TrayMenu::open_centred(event_loop, gpu, &screens, cursor, items, self.current_theme) {
            Ok(menu) => self.quick_task = Some(QuickTaskMenu { menu, job: Box::new(job) }),
            Err(e) => eprintln!("couldn't open the quick task menu: {e}"),
        }
    }

    /// Routes an event to the quick task menu's window; false when it's not
    /// its.
    pub(crate) fn quick_task_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) -> bool {
        let Some(q) = self.quick_task.as_mut().filter(|q| q.menu.owns(id)) else { return false };
        let out = q.menu.window_event(event);
        if let Some(command) = out.command {
            let Some(q) = self.quick_task.take() else { return true };
            let mut job = *q.job;
            match command {
                // `RunImageTask(metadata, taskSettings, true)`.
                QuickCmd::Continue => self.run_image_task_after_quick_menu(event_loop, job),
                QuickCmd::Preset(i) => {
                    if let Some(p) = self.settings.settings.QuickTaskPresets.get(i).filter(|p| p.is_valid()) {
                        job.task.AfterCaptureJob = p.AfterCaptureTasks;
                        job.task.AfterUploadJob = p.AfterUploadTasks;
                        self.run_image_task_after_quick_menu(event_loop, job);
                    }
                }
                // `QuickTaskMenuEditorIntegration.Show`; the capture goes.
                QuickCmd::Edit => self.show_quick_task_editor(event_loop),
                QuickCmd::Cancel => {}
            }
        } else if out.closed {
            // Escape (`CloseMenu`): the capture goes.
            self.quick_task = None;
        }
        true
    }

    /// `QuickTaskMenuEditorIntegration.Show`: one window.
    pub(crate) fn show_quick_task_editor(&mut self, event_loop: &ActiveEventLoop) {
        self.open_dialog_window(event_loop, Box::new(crate::dialogs::quick_task_editor::QuickTaskMenuEditorWindow::new()));
    }

    /// Repaints the quick task menu when it asked to be.
    pub(crate) fn quick_task_due(&mut self, now: std::time::Instant) -> Option<std::time::Instant> {
        let q = self.quick_task.as_mut()?;
        q.menu.redraw_if_due(now);
        q.menu.repaint_at()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_menu_is_sharex_s() {
        let presets = QuickTaskInfo::default_presets();
        let e = entries(&presets);
        let headers: Vec<&str> = e.iter().filter(|e| !e.separator).map(|e| e.header.as_str()).collect();
        assert_eq!(headers[0], "Continue");
        assert_eq!(headers[1], "Save, Upload, Copy URL");
        assert_eq!(headers.last(), Some(&"Cancel"));
        assert_eq!(e.iter().filter(|e| e.separator).count(), 4);
        let unnamed = QuickTaskInfo::new(None, AfterCaptureTasks::SaveImageToFile | AfterCaptureTasks::UploadImageToHost, AfterUploadTasks::CopyURLToClipboard);
        assert_eq!(display_name(&unnamed), "Save image to file, Upload image to host, Copy URL to clipboard");
    }
}
