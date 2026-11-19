//! Where the Video tools open from (`TaskHelpers.Open*`,
//! `ToolsIntegration.Show*Window`): the Tools menu, their hotkeys and
//! command lines (`TaskHelpers.ExecuteJob`, with a file for the converter
//! and the trimmers), and the task list's and history windows' Trim video
//! and Trim animated GIF. Each `Show` makes a new window, as ShareX's.

use sharex_settings::{HotkeyType, TaskSettings};
use winit::event_loop::ActiveEventLoop;

use super::{converter, gif_maker};
use crate::App;
use crate::dialogs::Dialog;

/// The Video tools, by hotkey (ShareX has hotkeys for four of them).
pub fn tool_for_hotkey(job: &HotkeyType) -> Option<&'static str> {
    Some(match job {
        HotkeyType::VideoConverter => "MainMenuBuilder_VideoConverter",
        HotkeyType::VideoTrimmer => "MainMenuBuilder_VideoTrimmer",
        HotkeyType::AnimatedGifTrimmer => "MainMenuBuilder_AnimatedGifTrimmer",
        HotkeyType::VideoThumbnailer => "MainMenuBuilder_VideoThumbnailer",
        _ => return None,
    })
}

impl App {
    /// A Tools menu entry of the Video tools (its `MainMenuBuilder_*` key),
    /// with `task` (the hotkey's, or the default task settings) and the
    /// command line's file. False when it isn't one of them.
    pub(crate) fn open_video_tool(&mut self, event_loop: &ActiveEventLoop, key: &str, task: Option<TaskSettings>, file: Option<String>) -> bool {
        let task = task.unwrap_or_else(|| TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings));
        let window: Box<dyn Dialog> = match key {
            // `OpenAnimatedGifMaker()`.
            "MainMenuBuilder_AnimatedGifMaker" => Box::new(gif_maker::AnimatedGifMakerWindow::default()),
            // `OpenVideoConverter(taskSettings)` / `(filePath, taskSettings)`:
            // FFmpeg checked first.
            "MainMenuBuilder_VideoConverter" => {
                let Some(ffmpeg) = self.check_ffmpeg(event_loop, &task) else { return true };
                let defaults = &self.settings.settings.DefaultTaskSettings;
                Box::new(converter::VideoConverterWindow::new(task, defaults, Some(ffmpeg), file))
            }
            _ => return false,
        };
        self.open_dialog_window(event_loop, window);
        true
    }
}
