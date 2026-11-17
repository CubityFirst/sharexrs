//! `ShareX/Presentation/DragDropUpload/DragDropUploadWindow.axaml(.cs)`: a
//! small borderless accent square in a corner of the screen; files dropped
//! on it are uploaded. Left drag moves it, right click closes it.

use std::path::PathBuf;

use egui::{Rect, Vec2, pos2};
use sharex_settings::{ApplicationConfig, ImageContentAlignment, TaskSettings};
use sharex_strings::sharex as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{font, line_height};

use super::{Cx, Dialog, Effect, Spec, UploadRequest};

pub struct DropWindow {
    size: f32,
    offset: i32,
    alignment: ImageContentAlignment,
    normal_opacity: f32,
    hover_opacity: f32,
    hovered: bool,
    /// `TaskSettings` (`DragDropUploadWindowIntegration.Show` sets it each time).
    pub task: TaskSettings,
    /// Files dropped since the last frame (winit gives them one by one).
    dropped: Vec<PathBuf>,
}

impl DropWindow {
    /// `new DragDropUploadWindow(size, offset, alignment, opacity, hoverOpacity)`
    /// from the settings (`TaskHelpers.OpenDropWindow`).
    pub fn new(settings: &ApplicationConfig, task: TaskSettings) -> Self {
        Self {
            size: settings.DropSize.clamp(10, 300) as f32,
            offset: settings.DropOffset.max(0),
            alignment: settings.DropAlignment.clone(),
            normal_opacity: settings.DropOpacity.clamp(1, 255) as f32 / 255.0,
            hover_opacity: settings.DropHoverOpacity.clamp(1, 255) as f32 / 255.0,
            hovered: false,
            task,
            dropped: Vec::new(),
        }
    }
}

impl Dialog for DropWindow {
    fn spec(&self) -> Spec {
        Spec {
            borderless: true,
            topmost: true,
            skip_taskbar: true,
            maximizable: false,
            resizable: false,
            ..Spec::sized(crate::rs_title(s::DragDropUploadWindow_Title()), self.size, self.size, 0.0, 0.0)
        }
    }

    fn single(&self) -> Option<&'static str> {
        Some("drop-window")
    }

    fn set_task(&mut self, task: TaskSettings) {
        self.task = task;
    }

    /// `PositionWindow`: on the primary screen's working area, by the
    /// alignment and offset.
    fn position(&self, area: sharex_capture::Rect, size: (u32, u32)) -> Option<(i32, i32)> {
        use ImageContentAlignment as A;
        let (w, h) = (size.0 as i32, size.1 as i32);
        let a = &self.alignment;
        let x = if [A::TopLeft, A::MiddleLeft, A::BottomLeft].contains(a) {
            area.x + self.offset
        } else if [A::TopCenter, A::MiddleCenter, A::BottomCenter].contains(a) {
            area.x + (area.w - w) / 2
        } else {
            area.x + area.w - w - self.offset
        };
        let y = if [A::TopLeft, A::TopCenter, A::TopRight].contains(a) {
            area.y + self.offset
        } else if [A::MiddleLeft, A::MiddleCenter, A::MiddleRight].contains(a) {
            area.y + (area.h - h) / 2
        } else {
            area.y + area.h - h - self.offset
        };
        Some((x, y))
    }

    fn opacity(&self) -> f32 {
        if self.hovered { self.hover_opacity } else { self.normal_opacity }
    }

    fn drag(&mut self, hover: bool) {
        self.hovered = hover;
    }

    fn dropped(&mut self, files: Vec<PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        self.dropped.extend(files);
        Vec::new()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        // `Border.drop-surface`: the accent with a 2px (3px while dragging
        // over) border in the accent's foreground, 10px corners.
        let thickness = if self.hovered { 3.0 } else { 2.0 };
        border::border(ui.painter(), full, 10.0, thickness, Some(Brush::accent(ui.ctx())), Some(p.accent_foreground));
        // The stack (`Spacing="5"`, `Margin="6"`), centred: the icon at
        // `size / 3` (16..42) from 35px, the text at `size / 7` (10..20)
        // from 55px.
        let icon_size = (self.size / 3.0).clamp(16.0, 42.0);
        let text_font = font((self.size / 7.0).clamp(10.0, 20.0), 600);
        let text = s::DragDropUploadWindow_DropHere();
        let show_icon = self.size >= 35.0;
        let show_text = self.size >= 55.0;
        let text_h = line_height(ui, &text_font);
        let mut stack = 0.0;
        if show_icon {
            stack += icon_size;
        }
        if show_text {
            stack += text_h + if show_icon { 5.0 } else { 0.0 };
        }
        let mut y = full.min.y + ((full.height() - stack) / 2.0).round_ties_even();
        if show_icon {
            let x = full.min.x + ((full.width() - icon_size) / 2.0).round_ties_even();
            icons::paint(ui.painter(), Rect::from_min_size(pos2(x, y), Vec2::splat(icon_size)), icon::Upload, p.accent_foreground);
            y += icon_size + 5.0;
        }
        if show_text {
            sharex_ui::controls::text_centered_at(ui, full.center().x, y, text, text_font, p.accent_foreground);
        }
        let r = ui.interact(full, ui.id().with("surface"), egui::Sense::click_and_drag());
        let r = r.on_hover_cursor(egui::CursorIcon::Move);
        let mut out = Vec::new();
        if !self.dropped.is_empty() {
            // `HandleDrop`: `UploadManager.UploadFile(files, taskSettings)`.
            let files = std::mem::take(&mut self.dropped);
            out.push(Effect::Upload(Box::new(UploadRequest::Files { files, task: self.task.clone(), confirmed: false })));
        }
        if r.hovered() && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
            out.push(Effect::DragWindow);
        }
        if r.hovered() && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary)) {
            out.push(Effect::Close);
        }
        out
    }
}
