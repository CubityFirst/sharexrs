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
        crate::app_windows::open_path(if folder.is_dir() { &folder } else { &parent });
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
