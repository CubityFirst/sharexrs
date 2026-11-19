//! The Screen category of the Tools menu (`MainMenuBuilder.BuildToolCategories`):
//! the colour picker window and the screen colour picker, the ruler
//! ("Measure"), the classic ruler, the mouse highlighter and pin to screen
//! (`ShareX.Tools`, `ShareX.HelpersLib/Presentation/ColorPicker`,
//! `ShareX.Avalonia/Windows/ScreenColorPicker*`), and where ShareX opens
//! them from (`TaskHelpers.ShowColorPickerDialog`, `OpenScreenColorPicker`,
//! `OpenRuler`, `OpenClassicRuler`, `OpenMouseHighlighter`, `PinToScreen*`,
//! `MouseHighlighterManager`).

pub mod classic_ruler;
pub mod color_picker;
pub mod highlighter;
pub mod overlay;
pub mod pin;
pub mod render;
pub mod ruler;
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
    Dialog(WindowId),
    /// A colour popup of region capture's tool options (its option).
    Region(sharex_capture::region::options_row::OptionItem),
}

/// Who a region capture for Pin to screen is for.
pub struct PinRegion {
    /// The startup window that asked (`CaptureRegionAsync`), hidden meanwhile.
    pub startup: Option<WindowId>,
    pub task: Box<TaskSettings>,
}

/// The screen tools that are open.
#[derive(Default)]
pub struct ScreenTools {
    picker: Option<(ScreenPicker, PickPurpose)>,
    /// The screen colour picker that just closed, kept until its colour
    /// is copied (its window owns the clipboard).
    closing_picker: Option<ScreenPicker>,
    /// `MouseHighlighterManager`.
    pub highlighter: highlighter::Manager,
    /// Tools › Ruler, while it's open.
    classic: Option<classic_ruler::ClassicRuler>,
    /// Tools › Measure, while it's open.
    ruler: Option<ruler::Ruler>,
    /// `PinToScreenManager.Windows`.
    pins: Vec<pin::PinWindow>,
    /// A region capture for Pin to screen in progress.
    pub pin_region: Option<PinRegion>,
    /// The Pin to screen startup windows and their tasks.
    pin_startups: Vec<(WindowId, Box<TaskSettings>)>,
}

/// The task's tools settings (`ToolsSettingsReference`): its own, or the
/// default task's when it uses those.
pub fn tools(task: &TaskSettings, defaults: &TaskSettings) -> sharex_settings::task_settings::TaskSettingsTools {
    let t = if task.UseDefaultToolsSettings { defaults.ToolsSettings.as_ref() } else { task.ToolsSettings.as_ref() };
    t.or(defaults.ToolsSettings.as_ref()).cloned().unwrap_or_default()
}

/// `task` with its effective tools settings (`ToolsSettingsReference`).
pub fn tools_task(task: &TaskSettings, defaults: &TaskSettings) -> TaskSettings {
    let mut t = task.clone();
    t.ToolsSettings = Some(tools(task, defaults));
    t
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
            HotkeyType::MouseHighlighter => {
                // `OpenMouseHighlighter`: the window.
                let o = highlighter::options_of(&self.settings.settings.DefaultTaskSettings);
                self.open_dialog_window(event_loop, Box::new(highlighter::HighlighterWindow::new(o)));
            }
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
            // `ShowColorPickerDialog`.
            HotkeyType::ColorPicker => {
                let s = &self.settings.settings;
                let recent = sharex_tools::color::recent_colors_from_json(&s.RecentColors);
                let w = color_picker::ColorPickerWindow::new(color_picker::recent_selected(s), recent);
                self.open_dialog_window(event_loop, Box::new(w));
            }
            HotkeyType::PinToScreen => match file {
                Some(f) => self.pin_file(event_loop, std::path::Path::new(&f), Some(task.clone())),
                None => self.show_pin_startup(event_loop, task.clone()),
            },
            // `PinToScreenFromScreen`: a region, pinned where it was.
            HotkeyType::PinToScreenFromScreen => self.pin_capture_region(None, task.clone()),
            HotkeyType::PinToScreenFromClipboard => self.pin_from_clipboard(event_loop, task),
            HotkeyType::PinToScreenFromFile => match file {
                Some(f) => self.pin_file(event_loop, std::path::Path::new(&f), Some(task.clone())),
                None => {
                    if let Some(f) = crate::image_tools::open_image_file_dialog() {
                        self.pin_file(event_loop, std::path::Path::new(&f), Some(task.clone()));
                    }
                }
            },
            HotkeyType::PinToScreenCloseAll => {
                // `ToolsIntegration.CloseAllPinnedImages`.
                self.screen_tools.pins.clear();
                play_action_sound(task);
            }
            // `OpenRuler`: `ShowRulerWindow` (a new one each time; one here).
            HotkeyType::Ruler => {
                if self.screen_tools.ruler.is_none() {
                    let theme = self.theme();
                    let opened = match &self.gpu {
                        Some(g) => Ok(g),
                        None => sharex_ui::gpu::Gpu::new().map(|g| &*self.gpu.insert(g)),
                    }
                    .and_then(|gpu| ruler::Ruler::open(event_loop, gpu, theme));
                    match opened {
                        Ok(r) => self.screen_tools.ruler = Some(r),
                        Err(e) => eprintln!("Measure: {e}"),
                    }
                }
            }
            // `OpenClassicRuler`: the task's region capture options.
            HotkeyType::ClassicRuler => {
                if self.screen_tools.classic.is_none() {
                    let o = task.CaptureSettings.clone().unwrap_or_default().RegionCaptureOptions;
                    let options = classic_ruler::ClassicRulerOptions::from_region(&o);
                    let theme = self.theme();
                    let opened = match &self.gpu {
                        Some(g) => Ok(g),
                        None => sharex_ui::gpu::Gpu::new().map(|g| &*self.gpu.insert(g)),
                    }
                    .and_then(|gpu| classic_ruler::ClassicRuler::open(event_loop, gpu, options, theme));
                    match opened {
                        Ok(r) => self.screen_tools.classic = Some(r),
                        Err(e) => eprintln!("Ruler: {e}"),
                    }
                }
            }
            // `MouseHighlighterManager.Toggle` with the task's options.
            HotkeyType::MouseHighlighter => {
                let defaults = &self.settings.settings.DefaultTaskSettings;
                let o = sharex_tools::mouse_highlighter::MouseHighlighterOptions::from_json(&tools(task, defaults).MouseHighlighterOptions);
                self.highlighter_toggle(event_loop, o);
            }
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
            PickPurpose::Dialog(_) | PickPurpose::Region(_) => tools(defaults, defaults).ScreenColorPickerOptions,
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
                    let general = self.default_general();
                    let title = format!("ShareXrs - {}", sharex_strings::sharex::ScreenColorPicker());
                    self.pending_notification = Some(crate::dialogs::notification::NotificationConfig::tip(&general, &message, &title, -1));
                }
            }
            PickPurpose::Region(item) => {
                if let Some(region) = self.region.as_mut() {
                    let c = result.map(|r| egui::Color32::from_rgba_unmultiplied(r.color.r, r.color.g, r.color.b, r.color.a));
                    region.session.picked_color(item, c);
                }
            }
            PickPurpose::Dialog(id) => {
                // `OnPickScreenClick`'s `finally`: shown and activated again.
                let mut cx = crate::dialogs::Cx { settings: &mut self.settings.settings, uploaders: &self.uploads.config };
                if let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == id) {
                    d.window.window.set_minimized(false);
                    d.window.window.focus_window();
                    d.dialog.screen_color(result.map(|r| r.color), &mut cx);
                    d.window.window.request_redraw();
                }
            }
        }
    }

    /// `TaskHelpers.PinToScreen(taskSettings)`: the startup window.
    pub(crate) fn show_pin_startup(&mut self, event_loop: &ActiveEventLoop, task: TaskSettings) {
        let before = self.dialogs.len();
        self.open_dialog_window(event_loop, Box::new(pin::PinStartupWindow::new()));
        if self.dialogs.len() > before
            && let Some(d) = self.dialogs.last()
        {
            self.screen_tools.pin_startups.push((d.window.window.id(), Box::new(task)));
        }
    }

    fn startup_task(&self, id: WindowId) -> TaskSettings {
        self.screen_tools
            .pin_startups
            .iter()
            .find(|(w, _)| *w == id)
            .map(|(_, t)| (**t).clone())
            .unwrap_or_else(|| TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings))
    }

    /// A source of the startup window (`CaptureRegionCommand`,
    /// `FromClipboardCommand`, `FromFileCommand`).
    pub(crate) fn pin_from(&mut self, event_loop: &ActiveEventLoop, id: WindowId, source: pin::Source) {
        let task = self.startup_task(id);
        let image = match source {
            pin::Source::Region => {
                // `RegionCaptureStarted`: hidden, 200 ms, a region.
                if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                    d.window.window.set_visible(false);
                }
                self.pin_capture_region(Some(id), task);
                if let Some(p) = self.pending_capture.as_mut() {
                    p.at += std::time::Duration::from_millis(200);
                }
                return;
            }
            pin::Source::Clipboard => sharex_capture::clipboard::get_image(),
            pin::Source::File => crate::image_tools::open_image_file_dialog().and_then(|f| crate::thumbnail::load(std::path::Path::new(&f))),
        };
        match image {
            Some(image) => self.pinned_from_startup(event_loop, id, image, None, task),
            None => {
                if let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == id) {
                    d.dialog.pin_failed(source);
                    d.window.window.request_redraw();
                }
            }
        }
    }

    /// `SourceSelected`: pinned, the sound, the startup window closed.
    fn pinned_from_startup(&mut self, event_loop: &ActiveEventLoop, id: WindowId, image: image::RgbaImage, location: Option<(i32, i32)>, task: TaskSettings) {
        self.pin_image(event_loop, image, location, &task);
        self.screen_tools.pin_startups.retain(|(w, _)| *w != id);
        self.dialog_effects(event_loop, id, vec![crate::dialogs::Effect::Close]);
    }

    /// `RegionCaptureTasks.GetRegionImageWithRectangleAsync` for a pin.
    fn pin_capture_region(&mut self, startup: Option<WindowId>, task: TaskSettings) {
        if self.region.is_some() || self.transparent.is_some() || self.pending_capture.is_some() {
            return;
        }
        self.screen_tools.pin_region = Some(PinRegion { startup, task: Box::new(task.clone()) });
        self.capture(crate::capture::CaptureJob::Region, task, false);
    }

    /// The region capture for a pin ended: the image and its top-left, or `None`.
    pub(crate) fn pin_region_captured(&mut self, event_loop: &ActiveEventLoop, target: PinRegion, captured: Option<(image::RgbaImage, (i32, i32))>) {
        match (target.startup, captured) {
            (Some(id), Some((image, at))) => self.pinned_from_startup(event_loop, id, image, Some(at), *target.task),
            (Some(id), None) => {
                // `RegionCaptureFinished` with an error: shown again.
                if let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == id) {
                    d.dialog.pin_failed(pin::Source::Region);
                    d.window.window.set_visible(true);
                    d.window.window.focus_window();
                    d.window.window.request_redraw();
                }
            }
            (None, Some((image, at))) => self.pin_image(event_loop, image, Some(at), &target.task),
            (None, None) => {}
        }
    }

    /// `PinToScreenFromClipboard`: ShareX's message when there's no image.
    fn pin_from_clipboard(&mut self, event_loop: &ActiveEventLoop, task: &TaskSettings) {
        match sharex_capture::clipboard::get_image() {
            Some(image) => self.pin_image(event_loop, image, None, task),
            None => {
                let title = format!("ShareXrs - {}", sharex_strings::sharex::PinToScreen());
                let message = crate::message_box::MessageBox::ok(title, sharex_strings::sharex::ClipboardDoesNotContainAnImage(), crate::message_box::MessageBoxIcon::Information);
                if let Err(e) = self.open_dialog(event_loop, message, crate::DialogPurpose::Information) {
                    eprintln!("couldn't open the message box: {e}");
                }
            }
        }
    }

    /// `TaskHelpers.PinToScreen(filePath)`.
    pub(crate) fn pin_file(&mut self, event_loop: &ActiveEventLoop, path: &std::path::Path, task: Option<TaskSettings>) {
        let task = task.unwrap_or_else(|| TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings));
        match crate::thumbnail::load(path) {
            Some(image) => self.pin_image(event_loop, image, None, &task),
            None => eprintln!("Pin to screen: couldn't load {}", path.display()),
        }
    }

    /// `TaskHelpers.PinToScreen(image, location, taskSettings)`: a pin
    /// with the task's options, and the action sound.
    pub(crate) fn pin_image(&mut self, event_loop: &ActiveEventLoop, image: image::RgbaImage, location: Option<(i32, i32)>, task: &TaskSettings) {
        if image.width() == 0 || image.height() == 0 {
            return;
        }
        let defaults = &self.settings.settings.DefaultTaskSettings;
        let options = sharex_tools::pin::PinToScreenOptions::from_json(&tools(task, defaults).PinToScreenOptions);
        let theme = self.theme();
        let opened = match &self.gpu {
            Some(g) => Ok(g),
            None => sharex_ui::gpu::Gpu::new().map(|g| &*self.gpu.insert(g)),
        }
        .and_then(|gpu| pin::PinWindow::open(event_loop, gpu, image, options, location, theme));
        match opened {
            Ok(p) => {
                self.screen_tools.pins.push(p);
                play_action_sound(task);
            }
            Err(e) => eprintln!("Pin to screen: {e}"),
        }
    }

    /// The options window of pin `id` saved: the pin takes them, and so
    /// do the default task settings' (`ToolsSettingsReference`).
    pub(crate) fn pin_options_saved(&mut self, id: WindowId, options: sharex_tools::pin::PinToScreenOptions) {
        if let Some(p) = self.screen_tools.pins.iter_mut().find(|p| p.window.window.id() == id) {
            p.apply_options(options.clone());
        }
        let t = self.settings.settings.DefaultTaskSettings.ToolsSettings.get_or_insert_default();
        options.write_json(&mut t.PinToScreenOptions);
    }

    /// Routes an event to a screen tool's window; false when it's none of theirs.
    pub(crate) fn screen_tools_window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: &WindowEvent) -> bool {
        if let Some(r) = self.screen_tools.classic.as_mut().filter(|r| r.owns(id)) {
            match r.window_event(event_loop, id, event.clone()) {
                classic_ruler::Outcome::Close => self.screen_tools.classic = None,
                classic_ruler::Outcome::Copy(text) => {
                    if let Err(e) = sharex_capture::clipboard::copy_text(&text, self.clipboard_owner()) {
                        eprintln!("Clipboard copy text failed: {e}");
                    }
                }
                classic_ruler::Outcome::Continue => {}
            }
            return true;
        }
        if let Some(r) = self.screen_tools.ruler.as_mut().filter(|r| r.owns(id)) {
            match r.window_event(id, event.clone()) {
                ruler::Outcome::Close => self.screen_tools.ruler = None,
                ruler::Outcome::Copy(text) => {
                    if let Err(e) = sharex_capture::clipboard::copy_text(&text, self.clipboard_owner()) {
                        eprintln!("Clipboard copy text failed: {e}");
                    }
                }
                ruler::Outcome::Continue => {}
            }
            return true;
        }
        if let Some(i) = self.screen_tools.pins.iter().position(|p| p.window.window.id() == id) {
            match self.screen_tools.pins[i].window_event(event_loop, event.clone()) {
                Some(pin::PinAction::Close) => {
                    self.screen_tools.pins.remove(i);
                }
                Some(pin::PinAction::Copy(image)) => {
                    if let Err(e) = sharex_capture::clipboard::copy_image(&image, self.clipboard_owner()) {
                        eprintln!("Clipboard copy image failed: {e}");
                    }
                }
                Some(pin::PinAction::Options) => {
                    let options = self.screen_tools.pins[i].options.clone();
                    self.open_dialog_window(event_loop, Box::new(pin::PinOptionsWindow::new(id, options)));
                }
                None => {}
            }
            return true;
        }

        if let Some((p, _)) = self.screen_tools.picker.as_mut().filter(|(p, _)| p.owns(id)) {
            if let screen_picker::Outcome::Done(result) = p.window_event(id, event.clone())
                && let Some((picker, purpose)) = self.screen_tools.picker.take()
            {
                self.screen_tools.closing_picker = Some(picker);
                self.screen_color_picked(purpose, result);
                self.screen_tools.closing_picker = None;
            }
            return true;
        }
        false
    }

    /// `MouseHighlighterManager.Toggle` / the window's Start/Stop: on or
    /// off, ShareX's message when it can't start.
    pub(crate) fn highlighter_toggle(&mut self, event_loop: &ActiveEventLoop, options: sharex_tools::mouse_highlighter::MouseHighlighterOptions) {
        let active = !self.screen_tools.highlighter.is_manual();
        if let Err(e) = self.screen_tools.highlighter.set_manual(active, options) {
            eprintln!("Could not start mouse highlighting. {e}");
            let text = format!("{} {e}", sharex_strings::tools::MouseHighlighter_StartFailed());
            let message = crate::message_box::MessageBox::ok("ShareXrs", &text, crate::message_box::MessageBoxIcon::Error);
            if let Err(e) = self.open_dialog(event_loop, message, crate::DialogPurpose::Information) {
                eprintln!("couldn't open the message box: {e}");
            }
        }
        for d in &self.dialogs {
            d.window.window.request_redraw();
        }
    }

    /// The highlighter window changed its options: the default task
    /// settings take them, the running highlighter too, and they're saved.
    pub(crate) fn highlighter_options_changed(&mut self, options: sharex_tools::mouse_highlighter::MouseHighlighterOptions) {
        let t = self.settings.settings.DefaultTaskSettings.ToolsSettings.get_or_insert_default();
        options.write_json(&mut t.MouseHighlighterOptions);
        self.screen_tools.highlighter.refresh(options);
        if let Err(e) = self.settings.save_application_config(false) {
            eprintln!("couldn't save the settings: {e}");
        }
    }

    /// `MouseHighlighterManager.ActivateOnStartup`.
    pub(crate) fn highlighter_activate_on_startup(&mut self, event_loop: &ActiveEventLoop) {
        let o = highlighter::options_of(&self.settings.settings.DefaultTaskSettings);
        if o.auto_activate {
            self.highlighter_toggle(event_loop, o);
        }
    }

    /// One of the screen tools' windows, to own the clipboard when no
    /// other window is open.
    pub(crate) fn screen_tool_window(&self) -> Option<&winit::window::Window> {
        let t = &self.screen_tools;
        t.ruler
            .as_ref()
            .map(|r| r.window())
            .or(t.classic.as_ref().map(|r| r.window()))
            .or(t.closing_picker.as_ref().map(|p| p.window()))
            .or(t.pins.first().map(|p| &*p.window.window))
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
        at(self.screen_tools.highlighter.tick(now));
        if let Some(r) = self.screen_tools.ruler.as_mut() {
            at(r.wake_at(now));
        }
        if let Some(r) = self.screen_tools.classic.as_mut() {
            at(r.wake_at(now));
        }
        for p in &mut self.screen_tools.pins {
            match p.window.repaint_at {
                Some(t) if t <= now => {
                    p.window.window.request_redraw();
                    p.window.repaint_at = None;
                }
                t => at(t),
            }
        }
        next
    }
}

/// `PlayNotificationSoundAsync(NotificationSound.ActionCompleted, taskSettings)`.
fn play_action_sound(task: &TaskSettings) {
    if task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction) {
        crate::sound::play_action_completed();
    }
}

/// `AccentPalette.GetHaloColor`: black or white, whichever contrasts more
/// with the accent.
pub fn halo(accent: egui::Color32) -> egui::Color32 {
    let l = sharex_ui::accent::relative_luminance(accent);
    let ratio = |a: f64, b: f64| (a.max(b) + 0.05) / (a.min(b) + 0.05);
    if ratio(l, 0.0) >= ratio(l, 1.0) { egui::Color32::BLACK } else { egui::Color32::WHITE }
}
