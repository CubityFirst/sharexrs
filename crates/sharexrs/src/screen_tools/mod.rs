//! The Screen category of the Tools menu (`MainMenuBuilder.BuildToolCategories`):
//! the colour picker window and the screen colour picker, the ruler
//! ("Measure"), the classic ruler, the mouse highlighter and pin to screen
//! (`ShareX.Tools`, `ShareX.HelpersLib/Presentation/ColorPicker`,
//! `ShareX.Avalonia/Windows/ScreenColorPicker*`), and where ShareX opens
//! them from (`TaskHelpers.ShowColorPickerDialog`, `OpenScreenColorPicker`,
//! `OpenRuler`, `OpenClassicRuler`, `OpenMouseHighlighter`, `PinToScreen*`,
//! `MouseHighlighterManager`).

pub mod overlay;
pub mod screen_picker;

use sharex_settings::{HotkeyType, TaskSettings};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::App;
use screen_picker::{PickResult, ScreenColorPickerOptions, ScreenPicker};

/// Who asked for a screen colour.
pub enum PickPurpose {
    /// Tools › Screen color picker / its hotkey (`OpenScreenColorPicker`):
    /// the text to the clipboard.
    Tool(Box<TaskSettings>),
    /// A colour picker window's Pick color from screen.
    #[allow(dead_code)]
    Dialog(WindowId),
}

/// The screen tools that are open.
#[derive(Default)]
pub struct ScreenTools {
    picker: Option<(ScreenPicker, PickPurpose)>,
}

/// The task's tools settings (`ToolsSettingsReference`): its own, or the
/// default task's when it uses those.
pub fn tools(task: &TaskSettings, defaults: &TaskSettings) -> sharex_settings::task_settings::TaskSettingsTools {
    let t = if task.UseDefaultToolsSettings { defaults.ToolsSettings.as_ref() } else { task.ToolsSettings.as_ref() };
    t.or(defaults.ToolsSettings.as_ref()).cloned().unwrap_or_default()
}

/// The Screen tools by their Tools menu key.
pub fn tool_job(key: &str) -> Option<HotkeyType> {
    Some(match key {
        "MainMenuBuilder_ColorPicker" => HotkeyType::ColorPicker,
        "MainMenuBuilder_ScreenColorPicker" => HotkeyType::ScreenColorPicker,
        "MainMenuBuilder_Ruler" => HotkeyType::Ruler,
        "MainMenuBuilder_ClassicRuler" => HotkeyType::ClassicRuler,
        "MainMenuBuilder_MouseHighlighter" => HotkeyType::MouseHighlighter,
        "MainMenuBuilder_PinToScreenDialog" => HotkeyType::PinToScreen,
        _ => return None,
    })
}

impl App {
    /// A Tools menu entry of the Screen tools; false when it isn't one.
    /// The menu's Mouse highlighter opens its window
    /// (`TaskHelpers.OpenMouseHighlighter`), where the hotkey toggles it.
    pub(crate) fn open_screen_tool(&mut self, event_loop: &ActiveEventLoop, key: &str) -> bool {
        let Some(job) = tool_job(key) else { return false };
        let defaults = &self.settings.settings.DefaultTaskSettings;
        let task = TaskSettings::default_task_settings(defaults);
        let task = task.safe_task_settings(defaults);
        match job {
            HotkeyType::MouseHighlighter => eprintln!("Mouse highlighter window: not ported yet"),
            job => {
                self.run_screen_job(event_loop, &job, &task, None);
            }
        }
        true
    }

    /// `TaskHelpers.ExecuteJob` for the Screen tools' jobs; false for others.
    pub(crate) fn run_screen_job(&mut self, event_loop: &ActiveEventLoop, job: &HotkeyType, task: &TaskSettings, file: Option<String>) -> bool {
        let _ = file;
        match job {
            HotkeyType::ScreenColorPicker => self.open_screen_color_picker(event_loop, PickPurpose::Tool(Box::new(task.clone()))),
            HotkeyType::ColorPicker
            | HotkeyType::Ruler
            | HotkeyType::ClassicRuler
            | HotkeyType::MouseHighlighter
            | HotkeyType::PinToScreen
            | HotkeyType::PinToScreenFromScreen
            | HotkeyType::PinToScreenFromClipboard
            | HotkeyType::PinToScreenFromFile
            | HotkeyType::PinToScreenCloseAll => eprintln!("{}: not ported yet", job.name()),
            _ => return false,
        }
        true
    }

    /// `ToolsIntegration.PickScreenColorAsync`.
    pub(crate) fn open_screen_color_picker(&mut self, event_loop: &ActiveEventLoop, purpose: PickPurpose) {
        if self.screen_tools.picker.is_some() {
            return;
        }
        let defaults = &self.settings.settings.DefaultTaskSettings;
        let options = match &purpose {
            PickPurpose::Tool(task) => tools(task, defaults).ScreenColorPickerOptions,
            PickPurpose::Dialog(_) => tools(defaults, defaults).ScreenColorPickerOptions,
        };
        let options = ScreenColorPickerOptions::from_json(&options);
        let theme = self.theme();
        let opened = match &self.gpu {
            Some(g) => Ok(g),
            None => sharex_ui::gpu::Gpu::new().map(|g| &*self.gpu.insert(g)),
        }
        .and_then(|gpu| ScreenPicker::open(event_loop, gpu, options, theme));
        match opened {
            Ok(p) => self.screen_tools.picker = Some((p, purpose)),
            Err(e) => {
                eprintln!("screen color picker: {e}");
                self.screen_color_picked(purpose, None);
            }
        }
    }

    /// The screen colour picker closed with `result`.
    fn screen_color_picked(&mut self, purpose: PickPurpose, result: Option<PickResult>) {
        match purpose {
            PickPurpose::Tool(task) => {
                // `OpenScreenColorPicker`: the Ctrl format when Ctrl was
                // held, parsed (`CodeMenuEntryPixelInfo.Parse`), copied.
                let Some(r) = result else { return };
                let defaults = &self.settings.settings.DefaultTaskSettings;
                let o = ScreenColorPickerOptions::from_json(&tools(&task, defaults).ScreenColorPickerOptions);
                let input = if r.control_pressed { o.format_ctrl } else { o.format };
                if input.is_empty() {
                    return;
                }
                let text = sharex_tools::color::pixel_info_parse(&input, &r.color, r.position);
                if let Err(e) = sharex_capture::clipboard::copy_text(&text, self.clipboard_owner()) {
                    eprintln!("Clipboard copy text failed: {e}");
                }
                if task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction) {
                    crate::sound::play_action_completed();
                }
                if task.GeneralSettings.as_ref().is_some_and(|g| g.ShowToastNotificationAfterTaskCompleted) {
                    let message = sharex_tools::text::format_dotnet(
                        sharex_strings::sharex::TaskHelpers_OpenQuickScreenColorPicker_Copied_to_clipboard___0_(),
                        &[text.as_str().into()],
                    );
                    eprintln!("Notification not ported yet: ShareXrs - {}: {message}", sharex_strings::sharex::ScreenColorPicker());
                }
            }
            PickPurpose::Dialog(_) => {}
        }
    }

    /// Routes an event to a screen tool's window; false when it's none of theirs.
    pub(crate) fn screen_tools_window_event(&mut self, _event_loop: &ActiveEventLoop, id: WindowId, event: &WindowEvent) -> bool {
        if let Some((p, _)) = self.screen_tools.picker.as_mut().filter(|(p, _)| p.owns(id)) {
            if let screen_picker::Outcome::Done(result) = p.window_event(id, event.clone())
                && let Some((_, purpose)) = self.screen_tools.picker.take()
            {
                self.screen_color_picked(purpose, result);
            }
            return true;
        }
        false
    }

    /// When the screen tools want to be woken.
    pub(crate) fn screen_tools_wake_at(&mut self, now: std::time::Instant) -> Option<std::time::Instant> {
        let mut next: Option<std::time::Instant> = None;
        let mut at = |t: Option<std::time::Instant>| {
            if let Some(t) = t {
                next = Some(next.map_or(t, |n| n.min(t)));
            }
        };
        if let Some((p, _)) = self.screen_tools.picker.as_mut() {
            at(p.wake_at(now));
        }
        next
    }
}
