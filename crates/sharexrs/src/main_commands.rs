//! `MainWindowIntegration.Run(MainFormCommand)`'s commands that open a
//! window of their own or a page: About, the screenshots folder, the
//! Donate/X/Discord links.

use winit::event_loop::ActiveEventLoop;

use crate::App;
use crate::dialogs::about::{AboutWindow, links};

impl App {
    /// `AboutWindowIntegration.Show`: one window; opening it starts the
    /// update check (`AboutWindow.OnOpened`) unless `DisableUpdateCheck`.
    pub(crate) fn show_about(&mut self, event_loop: &ActiveEventLoop) {
        let disabled = self.settings.system.disable_update_check;
        let open = self.dialogs.iter().any(|d| d.dialog.single() == Some("about"));
        let check = (!disabled && !open).then(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            let channel = self.settings.settings.UpdateChannel.clone();
            let portable = self.settings.paths.portable;
            // `UpdateManager.CreateUpdateChecker()` and `CheckUpdateAsync`.
            let _ = std::thread::Builder::new().name("about-update-check".into()).spawn(move || {
                let mut checker = sharex_update::create_update_checker(&channel, portable);
                checker.check_update();
                let _ = tx.send(checker);
            });
            rx
        });
        let window = AboutWindow::new(self.state.base_title.clone(), disabled, check);
        self.open_dialog_window(event_loop, Box::new(window));
    }

    /// `TaskHelpers.OpenScreenshotsFolder`: the default task's screenshots
    /// folder, or its parent when it isn't there yet.
    pub(crate) fn open_screenshots_folder(&mut self) {
        let parent = self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings));
        let task = sharex_settings::TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
        let metadata = crate::capture_job::Metadata { image: image::RgbaImage::new(0, 0), window_title: None, process_name: None };
        let folder = crate::capture_job::screenshots_folder(&task, &self.settings.settings, &parent, &metadata);
        crate::shell::open_folder(if folder.is_dir() { &folder } else { &parent });
    }

    /// The tray's Recent items (`BuildTrayMenu`: shown with `RecentTasksSave`,
    /// `RecentTasksShowInTrayMenu` and some recent task).
    pub(crate) fn refresh_recent_items(&mut self) {
        let s = &self.settings.settings;
        let m = &mut self.state.menu_state;
        m.show_recent_items = s.RecentTasksSave && s.RecentTasksShowInTrayMenu;
        m.recent_items = crate::main_menu::recent_items(&self.state.tasks.manager.recent, s.RecentTasksTrayMenuMostRecentFirst);
    }

    /// `MainFormCommand.Donate`, `X`, `Discord`.
    pub(crate) fn open_link(&mut self, which: Link) {
        let url = match which {
            Link::Donate => links::DONATE,
            Link::X => links::X_FOLLOW,
            Link::Discord => links::DISCORD,
        };
        crate::upload::open_url(url);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Link {
    Donate,
    X,
    Discord,
}

impl App {
    /// `ApplicationState.DefaultTaskSettings.GeneralSettings`.
    pub(crate) fn default_general(&self) -> sharex_settings::TaskSettingsGeneral {
        self.settings.settings.DefaultTaskSettings.GeneralSettings.clone().unwrap_or_default()
    }

    /// `DisableNotificationsOnFullscreen` while the active window is
    /// fullscreen (`CaptureHelpers.IsActiveWindowFullscreen`).
    pub(crate) fn notifications_blocked(&self, report: &crate::tasks::TaskReport) -> bool {
        report.general.as_ref().is_some_and(|g| g.DisableNotificationsOnFullscreen) && sharex_capture::platform::is_active_window_fullscreen(&sharex_capture::screenshot::screens())
    }

    /// `NotificationWindow.Show(config)`: the open toast gives way to the
    /// new one.
    pub(crate) fn show_notification(&mut self, event_loop: &ActiveEventLoop, config: crate::dialogs::notification::NotificationConfig) {
        let Some(window) = crate::dialogs::notification::NotificationWindow::new(config) else { return };
        if let Some(i) = self.dialogs.iter().position(|d| d.dialog.single() == Some("notification")) {
            self.dialogs.remove(i);
        }
        self.open_dialog_window(event_loop, Box::new(window));
    }

    /// `ImageViewerWindowIntegration.ShowImage(image)` (encoded, with an
    /// optional display name).
    pub(crate) fn show_image(&mut self, event_loop: &ActiveEventLoop, image: image::RgbaImage, name: Option<String>) {
        let mut images = crate::dialogs::image_viewer::Images::default();
        images.load_image(image, name);
        self.open_dialog_window(event_loop, Box::new(crate::dialogs::image_viewer::ImageViewerWindow::new(images)));
    }

    /// `ImageViewerWindowIntegration.ShowImage(filePath)`: the image and its
    /// folder's others.
    pub(crate) fn show_image_file(&mut self, event_loop: &ActiveEventLoop, path: &std::path::Path) {
        let mut images = crate::dialogs::image_viewer::Images::default();
        if images.load_file(path) {
            self.open_dialog_window(event_loop, Box::new(crate::dialogs::image_viewer::ImageViewerWindow::new(images)));
        }
    }

    /// `ImageViewerWindowIntegration.ShowImage(filePaths, selectedIndex)`.
    pub(crate) fn show_image_files(&mut self, event_loop: &ActiveEventLoop, files: &[std::path::PathBuf], selected: usize) {
        let mut images = crate::dialogs::image_viewer::Images::default();
        if images.load_files(files, selected) {
            self.open_dialog_window(event_loop, Box::new(crate::dialogs::image_viewer::ImageViewerWindow::new(images)));
        }
    }

    /// `TaskHelpers.OpenImageViewer()`: an image picked first
    /// (`SelectImageAsync`); nothing picked, nothing shown.
    pub(crate) fn open_image_viewer(&mut self, event_loop: &ActiveEventLoop) {
        let exts = sharex_helpers::file_helpers::IMAGE_FILE_EXTENSIONS;
        let picked = rfd::FileDialog::new()
            .set_title(sharex_strings::helpers_lib::ImageViewerWindow_Open_image())
            .add_filter("Images", exts)
            .pick_file();
        if let Some(path) = picked {
            self.show_image_file(event_loop, &path);
        }
    }
}

/// The Actions toolbar's title menu, while it's open.
pub type ToolbarMenu = crate::tray_menu::TrayMenu<crate::dialogs::actions_toolbar::ToolbarCmd>;

impl App {
    /// `TaskHelpers.ExecuteJob(job)`.
    pub(crate) fn run_job_now(&mut self, event_loop: &ActiveEventLoop, job: sharex_settings::HotkeyType, task: sharex_settings::TaskSettings) {
        self.run_job(event_loop, job, task, None);
    }

    /// `ActionsToolbarWindowIntegration.Toggle`.
    pub(crate) fn toggle_actions_toolbar(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(i) = self.dialogs.iter().position(|d| d.dialog.single() == Some("actions-toolbar")) {
            let id = self.dialogs[i].window.window.id();
            self.dialog_effects(event_loop, id, vec![crate::dialogs::Effect::Close]);
        } else {
            self.open_actions_toolbar(event_loop);
        }
    }

    /// `ActionsToolbarWindowIntegration.Show`.
    pub(crate) fn open_actions_toolbar(&mut self, event_loop: &ActiveEventLoop) {
        let w = crate::dialogs::actions_toolbar::ActionsToolbarWindow::new(&self.settings.settings);
        self.open_dialog_window(event_loop, Box::new(w));
    }

    /// The title's context menu (`CreateToolbarMenu`) at the pointer.
    pub(crate) fn open_actions_toolbar_menu(&mut self, event_loop: &ActiveEventLoop) {
        self.toolbar_menu = None;
        let gpu = match &self.gpu {
            Some(g) => g,
            None => return,
        };
        let screens = sharex_capture::screenshot::screens();
        let cursor = sharex_capture::screenshot::cursor_position().unwrap_or((0, 0));
        let entries = crate::dialogs::actions_toolbar::toolbar_menu(&self.settings.settings);
        match crate::tray_menu::TrayMenu::open_context(event_loop, gpu, &screens, cursor, entries, self.current_theme) {
            Ok(m) => self.toolbar_menu = Some(m),
            Err(e) => eprintln!("couldn't open the menu: {e}"),
        }
    }

    /// Routes an event to the toolbar's menu; false when it's not its.
    pub(crate) fn toolbar_menu_event(&mut self, event_loop: &ActiveEventLoop, id: winit::window::WindowId, event: winit::event::WindowEvent) -> bool {
        use crate::dialogs::actions_toolbar::ToolbarCmd;
        let Some(m) = self.toolbar_menu.as_mut().filter(|m| m.owns(id)) else { return false };
        let out = m.window_event(event);
        if out.closed || out.command.is_some() {
            self.toolbar_menu = None;
        }
        let Some(command) = out.command else { return true };
        let toolbar = self.dialogs.iter().position(|d| d.dialog.single() == Some("actions-toolbar"));
        let s = &mut self.settings.settings;
        match command {
            ToolbarCmd::Close => {
                if let Some(i) = toolbar {
                    let id = self.dialogs[i].window.window.id();
                    self.dialog_effects(event_loop, id, vec![crate::dialogs::Effect::Close]);
                }
                return true;
            }
            ToolbarCmd::LockPosition => s.ActionsToolbarLockPosition = !s.ActionsToolbarLockPosition,
            ToolbarCmd::StayOnTop => {
                s.ActionsToolbarStayTopMost = !s.ActionsToolbarStayTopMost;
                let level = if s.ActionsToolbarStayTopMost { winit::window::WindowLevel::AlwaysOnTop } else { winit::window::WindowLevel::Normal };
                if let Some(i) = toolbar {
                    self.dialogs[i].window.window.set_window_level(level);
                }
            }
            ToolbarCmd::OpenAtStartup => s.ActionsToolbarRunAtStartup = !s.ActionsToolbarRunAtStartup,
            ToolbarCmd::Edit => {
                self.open_dialog_window(event_loop, Box::new(crate::dialogs::actions_toolbar::ActionsToolbarEditorWindow::new()));
                return true;
            }
        }
        if let Err(e) = self.settings.save_application_config(false) {
            eprintln!("couldn't save the settings: {e}");
        }
        true
    }
}
