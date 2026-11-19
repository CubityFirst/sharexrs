//! Where the Data tools open from (`TaskHelpers.Open*`, `ExecuteJob`,
//! `ToolsIntegration.Show*Window`): the Tools menu (`MainMenuBuilder`'s
//! Data entries), their hotkeys and command lines (`-HashCheck <file>`,
//! `-Metadata <file>`, `-StripMetadata <file>`, `-AnalyzeImage`,
//! `-QRCode [file]`, `-IndexFolder`), the task list's and history
//! windows' Analyze image and the after capture task. OCR has its own
//! entry points (`crate::ocr`).

use sharex_settings::{HotkeyType, TaskSettings};
use winit::event_loop::ActiveEventLoop;

use super::{analyze_image, hash_checker, index_folder, metadata};
use crate::App;
use crate::dialogs::qr_code::{QrCodeWindow, ScanMode};

impl App {
    /// A Tools menu entry of the Data tools (its `MainMenuBuilder_*` key);
    /// false when it isn't one. OCR is handled before (it hides the main
    /// window).
    pub(crate) fn open_data_tool(&mut self, event_loop: &ActiveEventLoop, key: &str) -> bool {
        let defaults = &self.settings.settings.DefaultTaskSettings;
        let task = TaskSettings::default_task_settings(defaults);
        match key {
            "MainMenuBuilder_AnalyzeImage" => self.run_data_job(event_loop, &HotkeyType::AnalyzeImage, &task, None),
            "MainMenuBuilder_HashChecker" => self.run_data_job(event_loop, &HotkeyType::HashCheck, &task, None),
            // `OpenDirectoryIndexer()`: the window (the hotkey uploads instead).
            "MainMenuBuilder_IndexFolder" => {
                let binary = self.settings.settings.BinaryUnits;
                let defaults = self.settings.settings.DefaultTaskSettings.clone();
                let w = index_folder::DirectoryIndexerWindow::new(task, &defaults, binary);
                self.open_dialog_window(event_loop, Box::new(w));
                true
            }
            "MainMenuBuilder_Metadata" => self.run_data_job(event_loop, &HotkeyType::Metadata, &task, None),
            "MainMenuBuilder_QRCode" => self.run_data_job(event_loop, &HotkeyType::QRCode, &task, None),
            _ => false,
        }
    }

    /// `TaskHelpers.ExecuteJob` for the Data tools' jobs, with the
    /// command line's file; false for other jobs.
    pub(crate) fn run_data_job(&mut self, event_loop: &ActiveEventLoop, job: &HotkeyType, task: &TaskSettings, file: Option<String>) -> bool {
        let defaults = self.settings.settings.DefaultTaskSettings.clone();
        match job {
            // `OpenHashCheck(filePath, taskSettings)`.
            HotkeyType::HashCheck => {
                let w = hash_checker::HashCheckerWindow::new(file, task);
                self.open_dialog_window(event_loop, Box::new(w));
            }
            // `OpenMetadataWindow(filePath)`.
            HotkeyType::Metadata => self.open_dialog_window(event_loop, Box::new(metadata::MetadataWindow::new(file))),
            // `StripMetadata(filePath, taskSettings)`, a picked file without one.
            HotkeyType::StripMetadata => {
                let file = file.or_else(|| rfd::FileDialog::new().pick_file().map(|p| p.to_string_lossy().into_owned()));
                if let Some(f) = file.filter(|f| !f.is_empty()) {
                    match sharex_tools::metadata::strip(&f) {
                        Ok(()) => crate::video_tools::action_sound(task),
                        Err(e) => {
                            eprintln!("StripMetadata({f}): {e}");
                            self.show_error(event_loop, &e.to_string());
                        }
                    }
                }
            }
            // `AnalyzeImage(filePath, taskSettings)`.
            HotkeyType::AnalyzeImage => {
                let w = analyze_image::AnalyzeImageWindow::new(task.clone(), &defaults, file);
                self.open_dialog_window(event_loop, Box::new(w));
            }
            // `OpenQRCodeScanFromImageFile(filePath)` / `OpenQRCode()`.
            HotkeyType::QRCode => {
                let w = match file {
                    Some(f) if !f.is_empty() => QrCodeWindow::scan_image_file(std::path::Path::new(&f)),
                    _ => QrCodeWindow::new(None),
                };
                self.open_dialog_window(event_loop, Box::new(w));
            }
            HotkeyType::QRCodeDecodeFromScreen => self.open_dialog_window(event_loop, Box::new(QrCodeWindow::scan_mode(ScanMode::Screen))),
            HotkeyType::QRCodeScanRegion => self.open_dialog_window(event_loop, Box::new(QrCodeWindow::scan_mode(ScanMode::Region))),
            // `UploadManager.IndexFolder()`: a picked folder, indexed and uploaded.
            HotkeyType::IndexFolder => {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    let mut ctx = self.new_task_context();
                    let tasks: Vec<_> = crate::upload::manager::index_folder(&folder.to_string_lossy(), task, &mut ctx).into_iter().collect();
                    self.start_upload_tasks(tasks);
                }
            }
            _ => return false,
        }
        true
    }
}
