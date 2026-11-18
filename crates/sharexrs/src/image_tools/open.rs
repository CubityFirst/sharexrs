//! Where the Images tools open from (`TaskHelpers.Open*`,
//! `ToolsIntegration.Show*Window`): the Tools menu, their hotkeys
//! (`TaskHelpers.ExecuteJob`) and the task list's Resize image and Combine
//! images (`UploadInfoManager.ResizeImages`, `CombineImages(orientation)`).
//! Each `Show` makes a new window, as ShareX's.

use sharex_settings::{HotkeyType, TaskSettings};
use winit::event_loop::ActiveEventLoop;

use super::{combiner, comparer, converter, icon_converter, resizer, splitter, thumbnailer, watermark};
use crate::App;
use crate::dialogs::Dialog;

/// The Images tools ported, by hotkey.
pub fn tool_for_hotkey(job: &HotkeyType) -> Option<&'static str> {
    Some(match job {
        HotkeyType::ImageCombiner => "MainMenuBuilder_ImageCombiner",
        HotkeyType::ImageSplitter => "MainMenuBuilder_ImageSplitter",
        HotkeyType::ImageThumbnailer => "MainMenuBuilder_ImageThumbnailer",
        HotkeyType::ImageResizer => "MainMenuBuilder_ImageResizer",
        HotkeyType::ImageConverter => "MainMenuBuilder_ImageConverter",
        HotkeyType::ImageWatermark => "MainMenuBuilder_ImageWatermark",
        HotkeyType::ImageComparer => "MainMenuBuilder_ImageComparer",
        HotkeyType::IconConverter => "MainMenuBuilder_IconConverter",
        HotkeyType::ImageEffects => "MainMenuBuilder_ImageEffects",
        HotkeyType::ImageEditor => "MainMenuBuilder_ImageEditor",
        _ => return None,
    })
}

impl App {
    /// A Tools menu entry of the Images tools (its `MainMenuBuilder_*`
    /// key), with `task` (the hotkey's, or the default task settings).
    /// False when it isn't one of them.
    pub(crate) fn open_image_tool(&mut self, event_loop: &ActiveEventLoop, key: &str, task: Option<TaskSettings>) -> bool {
        let defaults = &self.settings.settings.DefaultTaskSettings;
        let task = task.unwrap_or_else(|| TaskSettings::default_task_settings(defaults));
        let window: Box<dyn Dialog> = match key {
            // `OpenImageCombiner(null, taskSettings)`.
            "MainMenuBuilder_ImageCombiner" => Box::new(combiner::ImageCombinerWindow::new(task, defaults, Vec::new())),
            "MainMenuBuilder_ImageSplitter" => Box::new(splitter::ImageSplitterWindow::default()),
            "MainMenuBuilder_ImageThumbnailer" => Box::new(thumbnailer::ImageThumbnailerWindow::default()),
            "MainMenuBuilder_ImageResizer" => Box::new(resizer::ImageResizerWindow::default()),
            "MainMenuBuilder_ImageConverter" => Box::new(converter::ImageConverterWindow::default()),
            "MainMenuBuilder_ImageWatermark" => Box::new(watermark::ImageWatermarkWindow::default()),
            "MainMenuBuilder_ImageComparer" => Box::new(comparer::ImageComparerWindow::default()),
            "MainMenuBuilder_IconConverter" => Box::new(icon_converter::IconConverterWindow::default()),
            "MainMenuBuilder_ImageEffects" => {
                self.open_image_effects_tool(event_loop, None, task);
                return true;
            }
            // `TaskHelpers.OpenImageEditor(taskSettings)`: no image.
            "MainMenuBuilder_ImageEditor" => {
                self.open_image_editor(event_loop, None, None, task, None);
                return true;
            }
            _ => return false,
        };
        self.open_dialog_window(event_loop, window);
        true
    }

    /// `UploadInfoManager.ResizeImages` → `TaskHelpers.OpenImageResizer(imageFiles)`.
    pub(crate) fn open_image_resizer(&mut self, event_loop: &ActiveEventLoop, files: Vec<String>) {
        if !files.is_empty() {
            self.open_dialog_window(event_loop, Box::new(resizer::ImageResizerWindow::new(files)));
        }
    }

    /// `TaskHelpers.CombineImages(imageFiles, orientation)`: the default
    /// task settings' combiner options in that orientation, the result
    /// through `RunImageTask`.
    pub(crate) fn combine_images(&mut self, files: Vec<String>, orientation: sharex_tools::combiner::Orientation) {
        if files.len() < 2 {
            return;
        }
        let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
        let mut options = task.ToolsSettings.as_ref().map(|t| sharex_tools::combiner::ImageCombinerOptions::from_json(&t.ImageCombinerOptions)).unwrap_or_default();
        options.orientation = orientation;
        if let Some(bmp) = sharex_tools::combiner::combine_files(&files, &options) {
            let image = image::RgbaImage::from_raw(bmp.width(), bmp.height(), bmp.to_straight()).unwrap_or_default();
            self.run_image_task_for(image, task);
        }
    }
}
