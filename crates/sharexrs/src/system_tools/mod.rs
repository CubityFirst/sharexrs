//! ShareX.Tools' System windows (docs/TOOLS.md, "The Data and System
//! windows in ShareXrs"): the clipboard viewer, inspect window (and its
//! picker overlay), monitor test, network monitor and borderless window,
//! each a [`Dialog`](crate::dialogs::Dialog) over its engine in
//! `sharex_tools::system` / `sharex_tools::monitor_test`, with the native
//! side behind the traits in [`native`].

pub mod borderless_window;
pub mod clipboard_viewer;
pub mod inspect_window;
pub mod monitor_test;
pub mod native;
pub mod network_monitor;
pub mod open;
pub mod render;

use sharex_ui::controls::ComboItem;
use sharex_ui::keys::app_icons as icon;

use native::ListedWindow;

/// The window lists' `InspectWindowListItem`s with their icons as textures.
#[derive(Default)]
pub struct WindowList {
    pub windows: Vec<ListedWindow>,
    textures: Vec<Option<egui::TextureHandle>>,
}

impl WindowList {
    pub fn set(&mut self, windows: Vec<ListedWindow>) {
        self.windows = windows;
        self.textures.clear();
    }

    /// The combo box's items: the icon (or `AppIcons.Window`) in a `cell`
    /// px column and `DisplayName` (Normal).
    pub fn items(&mut self, ctx: &egui::Context, cell: f32) -> Vec<ComboItem> {
        if self.textures.len() != self.windows.len() {
            self.textures = self
                .windows
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    w.icon.as_ref().map(|img| {
                        let shown = if img.width() as f32 > cell || img.height() as f32 > cell {
                            crate::thumbnail::resize(img, cell as u32, cell as u32)
                        } else {
                            img.clone()
                        };
                        ctx.load_texture(format!("window-list-icon-{i}"), crate::thumbnail::to_color_image(&shown), egui::TextureOptions::LINEAR)
                    })
                })
                .collect();
        }
        self.windows
            .iter()
            .zip(&self.textures)
            .map(|(w, t)| ComboItem { icon: Some(icon::Window), cell: Some((cell, t.as_ref().map(|t| t.id()))), ..ComboItem::normal(w.entry.display_name()) })
            .collect()
    }
}
