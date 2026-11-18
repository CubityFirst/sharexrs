//! `InsertImageDialogView` with its `InsertImageDialogViewModel`: where a
//! picked image goes (centred as an annotation, or the canvas grown below
//! or to the right for it), after the Image tool's file picker.

use egui::{Id, Rect, Ui, Vec2, pos2, vec2};
use image::RgbaImage;
use sharex_strings::image_editor as ie;
use sharex_ui::controls::Button;
use sharex_ui::keys::app_icons;
use sharex_ui::theme::Thickness;
use sharex_ui::{Palette, icons, widgets};

use super::modal::{self, text_font};

const WIDTH: f32 = 380.0;
const BUTTON_MIN_H: f32 = 58.0;

/// `InsertImagePlacement`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    Center,
    CanvasExpandDown,
    CanvasExpandRight,
}

/// The dialog, holding the picked image until it's placed.
pub struct InsertImage {
    pub image: RgbaImage,
}

pub enum Outcome {
    Place(Placement),
    Cancel,
}

impl InsertImage {
    fn button_height(ui: &Ui) -> f32 {
        let content = widgets::line_height(ui, &text_font(13.0, 400)) + 2.0 + widgets::line_height(ui, &text_font(11.0, 400));
        (2.0 + 20.0 + content).max(BUTTON_MIN_H)
    }

    pub fn size(&self, ui: &Ui) -> Vec2 {
        let h = 2.0 + modal::title_height(ui, true) + 20.0 + 3.0 * Self::button_height(ui) + 2.0 * 12.0 + 20.0;
        vec2(WIDTH, h)
    }

    pub fn show(&mut self, ui: &mut Ui, p: &Palette, rect: Rect) -> Option<Outcome> {
        let mut out = None;
        let inner = modal::frame(ui, p, rect, modal::SHADOW_LARGE);
        let (title, close) = modal::title_bar(ui, p, inner, ie::InsertImageDialogView_Title(), p.text, Some(Id::new("insert-close")));
        if close.is_some_and(|r| r.clicked()) {
            out = Some(Outcome::Cancel);
        }
        let x = inner.min.x + 20.0;
        let w = inner.width() - 40.0;
        let mut y = title.max.y + 20.0;
        let bh = Self::button_height(ui);
        let rows = [
            (Placement::Center, app_icons::Locate, ie::InsertImageDialogView_Center(), ie::InsertImageDialogView_CenterDescription()),
            (Placement::CanvasExpandDown, app_icons::ArrowDownToLine, ie::InsertImageDialogView_Below(), ie::InsertImageDialogView_BelowDescription()),
            (Placement::CanvasExpandRight, app_icons::ArrowRightToLine, ie::InsertImageDialogView_Right(), ie::InsertImageDialogView_RightDescription()),
        ];
        let f13 = text_font(13.0, 400);
        let f11 = text_font(11.0, 400);
        let (lh13, lh11) = (widgets::line_height(ui, &f13), widgets::line_height(ui, &f11));
        for (i, (placement, icon, head, desc)) in rows.into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(x, y), vec2(w, bh));
            y += bh + 12.0;
            let pad = Thickness::hv(14.0, 10.0);
            let resp = Button { padding: pad, ..Default::default() }.show(ui, r, Id::new(("insert-placement", i)), "", true);
            let fg = if resp.hovered() || resp.is_pointer_button_down_on() { sharex_ui::theme::fluent(ui.ctx()).base_high } else { p.text };
            let content = Rect::from_min_max(pos2(r.min.x + 15.0, r.min.y + 11.0), pos2(r.max.x - 15.0, r.max.y - 11.0));
            let text_h = lh13 + 2.0 + lh11;
            let row_h = text_h.max(18.0);
            let top = widgets::center_y(content, row_h);
            icons::paint(ui.painter(), Rect::from_min_size(pos2(content.min.x, top + ((row_h - 18.0) / 2.0).round_ties_even()), vec2(18.0, 18.0)), icon, fg);
            let tx = content.min.x + 18.0 + 12.0;
            let ty = top + ((row_h - text_h) / 2.0).round_ties_even();
            let g = widgets::galley(ui, head, f13.clone(), p.text_secondary, f32::INFINITY);
            ui.painter().galley(pos2(tx, ty), g, p.text_secondary);
            let g = widgets::galley(ui, desc, f11.clone(), p.text_secondary, f32::INFINITY);
            ui.painter().galley(pos2(tx, ty + lh13 + 2.0), g, p.text_secondary);
            if resp.clicked() {
                out = Some(Outcome::Place(placement));
            }
        }
        out
    }
}
