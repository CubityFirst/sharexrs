//! `ShareX/Presentation/IconPackPrompt`: `IconPackPromptWindow`, which
//! asks once (`ShowIconPackPrompt`) which icon pack to use, showing a few
//! icons from each. Picking a card applies the pack at once; closing the
//! window clears the flag and saves the settings. 620 wide, as tall as its
//! content.

use egui::{Id, Rect, Vec2, pos2, vec2};
use sharex_settings::IconPack;
use sharex_strings::sharex as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, wrap_lines};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height};
use sharex_ui::{focus, icons};

use super::{Cx, Dialog, Effect, Spec};

const WIDTH: f32 = 620.0;
/// The header's `Padding="18,14"`, its 40px tile (`CornerRadius="8"`) and
/// `ColumnSpacing="12"`.
const HEADER_PADDING: (f32, f32) = (18.0, 14.0);
const TILE: f32 = 40.0;
const TILE_SPACING: f32 = 12.0;
/// The cards' grid: `Margin="18,16,18,0"`, `ColumnSpacing="12"`.
const MARGIN: f32 = 18.0;
const CARDS_TOP: f32 = 16.0;
const CARD_SPACING: f32 = 12.0;
/// `Button.pack-card`: `Padding="12"`, `CornerRadius="10"`; ShareX's
/// `Button /template/ ContentPresenter` style keeps its border at 1px.
const CARD_PADDING: f32 = 12.0;
const CARD_RADIUS: f32 = 10.0;
/// The card's `StackPanel Spacing="10"`.
const STACK_SPACING: f32 = 10.0;
/// `Border.preview`: `Padding="10,12"`, `CornerRadius="8"`; its icons 22px
/// with `Margin="0,6"`, four to a row.
const PREVIEW_PADDING: (f32, f32) = (10.0, 12.0);
const PREVIEW_ICON: f32 = 22.0;
const CHECK: f32 = 18.0;
/// The footer: `Margin="18,16,18,18"`, `ColumnSpacing="16"`.
const FOOTER_TOP: f32 = 16.0;
const FOOTER_SPACING: f32 = 16.0;

/// `PreviewIcons`: main menu commands that look clearly different in the
/// two packs.
const PREVIEW_ICONS: [&str; 8] = [
    icon::Region,
    icon::ScreenRecording,
    icon::ImageUpload,
    icon::ImageEditor,
    icon::Clipboard,
    icon::History,
    icon::ScreenColorPicker,
    icon::Settings,
];

#[derive(Default)]
pub struct IconPackPromptWindow;

impl IconPackPromptWindow {
    fn title_font() -> egui::FontId {
        font(19.0, 600)
    }

    fn subtitle_font() -> egui::FontId {
        font(13.0, 400)
    }

    fn pack_title_font() -> egui::FontId {
        font(15.0, 600)
    }

    fn description_font() -> egui::FontId {
        font(12.0, 400)
    }

    fn change_later_font() -> egui::FontId {
        font(11.0, 400)
    }

    fn continue_button() -> Button {
        Button::primary(Thickness::hv(20.0, 6.0), 8.0)
    }

    fn header_text_width() -> f32 {
        WIDTH - 2.0 * HEADER_PADDING.0 - TILE - TILE_SPACING
    }

    fn header_height(ui: &egui::Ui) -> f32 {
        let lines = wrap_lines(ui, s::IconPackPrompt_Subtitle(), &Self::subtitle_font(), Self::header_text_width()).len().max(1);
        let text = line_height(ui, &Self::title_font()) + 2.0 + line_height(ui, &Self::subtitle_font()) * lines as f32;
        2.0 * HEADER_PADDING.1 + text.max(TILE)
    }

    fn card_width() -> f32 {
        ((WIDTH - 2.0 * MARGIN - CARD_SPACING) / 2.0).floor()
    }

    fn content_width() -> f32 {
        Self::card_width() - 2.0 * (CARD_PADDING + 1.0)
    }

    fn preview_height() -> f32 {
        2.0 * PREVIEW_PADDING.1 + 2.0 * (PREVIEW_ICON + 12.0)
    }

    fn description(pack: IconPack) -> &'static str {
        match pack {
            IconPack::Legacy => s::IconPackPrompt_LegacyDescription(),
            _ => s::IconPackPrompt_ModernDescription(),
        }
    }

    /// The card's content: preview, title row, description.
    fn card_content_height(ui: &egui::Ui, pack: IconPack) -> f32 {
        let title = line_height(ui, &Self::pack_title_font()).max(CHECK);
        let df = Self::description_font();
        let lines = wrap_lines(ui, Self::description(pack), &df, Self::content_width()).len().max(1);
        Self::preview_height() + STACK_SPACING + title + STACK_SPACING + line_height(ui, &df) * lines as f32
    }

    /// Both cards share their grid row: as tall as the taller one.
    fn card_height(ui: &egui::Ui) -> f32 {
        let content = Self::card_content_height(ui, IconPack::Modern).max(Self::card_content_height(ui, IconPack::Legacy));
        content + 2.0 * (CARD_PADDING + 1.0)
    }

    fn footer_text_width(ui: &egui::Ui) -> f32 {
        let b = Self::continue_button();
        WIDTH - 2.0 * MARGIN - FOOTER_SPACING - b.width(ui, s::AfterCaptureWindow_Continue())
    }

    fn footer_height(ui: &egui::Ui) -> f32 {
        let f = Self::change_later_font();
        let lines = wrap_lines(ui, s::IconPackPrompt_ChangeLater(), &f, Self::footer_text_width(ui)).len().max(1);
        (line_height(ui, &f) * lines as f32).max(40.0)
    }

    /// A pack card (`Button.pack-card`, `.selected` on the chosen pack).
    fn card(ui: &egui::Ui, rect: Rect, id: Id, pack: IconPack, selected: bool) -> egui::Response {
        let p = palette(ui.ctx());
        let r = ui.interact(rect, id, egui::Sense::click());
        let over = r.hovered() || r.is_pointer_button_down_on();
        let fill = if over && !r.is_pointer_button_down_on() { p.control_background_hover } else { p.control_background };
        let stroke = if selected { p.accent_border } else { p.border };
        border::border(ui.painter(), rect, CARD_RADIUS, 1.0, Some(Brush::Solid(fill)), Some(stroke));
        let inner = rect.shrink(CARD_PADDING + 1.0);
        let ui_pack = if pack == IconPack::Legacy { icons::IconPack::Legacy } else { icons::IconPack::Modern };
        // The preview.
        let preview = Rect::from_min_size(inner.min, vec2(inner.width(), Self::preview_height()));
        border::border(ui.painter(), preview, 8.0, 0.0, Some(Brush::Solid(p.background_main)), None);
        let cells = Rect::from_min_max(
            pos2(preview.min.x + PREVIEW_PADDING.0, preview.min.y + PREVIEW_PADDING.1),
            pos2(preview.max.x - PREVIEW_PADDING.0, preview.max.y - PREVIEW_PADDING.1),
        );
        let cell_w = cells.width() / 4.0;
        for (i, key) in PREVIEW_ICONS.iter().enumerate() {
            let (col, row) = ((i % 4) as f32, (i / 4) as f32);
            let x = cells.min.x + cell_w * col + ((cell_w - PREVIEW_ICON) / 2.0).round_ties_even();
            let y = cells.min.y + (PREVIEW_ICON + 12.0) * row + 6.0;
            icons::paint_in_pack(ui.painter(), Rect::from_min_size(pos2(x, y), Vec2::splat(PREVIEW_ICON)), key, p.text, ui_pack);
        }
        // The title row.
        let tf = Self::pack_title_font();
        let row_h = line_height(ui, &tf).max(CHECK);
        let row = Rect::from_min_size(pos2(inner.min.x, preview.max.y + STACK_SPACING), vec2(inner.width(), row_h));
        let title = match pack {
            IconPack::Legacy => s::ApplicationSettingsWindow_IconPackLegacy(),
            _ => s::ApplicationSettingsWindow_IconPackModern(),
        };
        let g = galley(ui, title, tf.clone(), p.text_secondary, row.width() - CHECK);
        ui.painter().galley(pos2(row.min.x, center_y(row, line_height(ui, &tf))), g, p.text_secondary);
        if selected {
            let check = Rect::from_min_size(pos2(row.max.x - CHECK, center_y(row, CHECK)), Vec2::splat(CHECK));
            icons::paint(ui.painter(), check, icon::CircleCheck, p.accent_icon);
        }
        // The description.
        let df = Self::description_font();
        let lh = line_height(ui, &df);
        for (i, line) in wrap_lines(ui, Self::description(pack), &df, inner.width()).iter().enumerate() {
            let g = galley(ui, line, df.clone(), p.text_secondary, f32::INFINITY);
            ui.painter().galley(pos2(inner.min.x, row.max.y + STACK_SPACING + lh * i as f32), g, p.text_secondary);
        }
        if focus::visible(ui.ctx(), id) {
            focus::paint_adorner(ui.painter(), rect);
        }
        r
    }
}

impl Dialog for IconPackPromptWindow {
    fn spec(&self) -> Spec {
        Spec { maximizable: false, ..Spec::fit_height(s::IconPackPrompt_Title(), WIDTH, 0.0) }
    }

    fn content_height(&mut self, ui: &mut egui::Ui) -> f32 {
        Self::header_height(ui) + CARDS_TOP + Self::card_height(ui) + FOOTER_TOP + Self::footer_height(ui) + MARGIN
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = super::layout::background(ui);
        let mut out = Vec::new();

        // The header.
        let header_h = Self::header_height(ui);
        let band = Rect::from_min_size(full.min, vec2(full.width(), header_h));
        ui.painter().rect_filled(band, 0.0, p.background_toolbar);
        let row = band.shrink2(vec2(HEADER_PADDING.0, HEADER_PADDING.1));
        let tile = Rect::from_min_size(pos2(row.min.x, center_y(row, TILE)), Vec2::splat(TILE));
        border::border(ui.painter(), tile, 8.0, 0.0, Some(Brush::Solid(p.background_panel)), None);
        icons::paint(ui.painter(), Rect::from_center_size(tile.center(), Vec2::splat(20.0)), icon::Theme, p.accent_icon);
        let text_x = tile.max.x + TILE_SPACING;
        let (tf, sf) = (Self::title_font(), Self::subtitle_font());
        let sub = wrap_lines(ui, s::IconPackPrompt_Subtitle(), &sf, Self::header_text_width());
        let (th, sh) = (line_height(ui, &tf), line_height(ui, &sf));
        let mut y = center_y(row, th + 2.0 + sh * sub.len().max(1) as f32);
        ui.painter().galley(pos2(text_x, y), galley(ui, s::IconPackPrompt_Title(), tf, p.text, f32::INFINITY), p.text);
        y += th + 2.0;
        for line in &sub {
            ui.painter().galley(pos2(text_x, y), galley(ui, line, sf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            y += sh;
        }

        // The cards.
        let top = band.max.y + CARDS_TOP;
        let (cw, ch) = (Self::card_width(), Self::card_height(ui));
        let chosen = cx.settings.theme_options().IconPack.clone();
        for (i, pack) in [IconPack::Modern, IconPack::Legacy].into_iter().enumerate() {
            let rect = Rect::from_min_size(pos2(full.min.x + MARGIN + (cw + CARD_SPACING) * i as f32, top), vec2(cw, ch));
            if Self::card(ui, rect, ui.id().with(("pack", i)), pack.clone(), chosen == pack).clicked() && chosen != pack {
                // `SelectPack`: applied at once.
                cx.settings.theme_options().IconPack = pack;
                out.push(Effect::ApplyAppearance);
            }
        }

        // The footer.
        let footer = Rect::from_min_size(pos2(full.min.x + MARGIN, top + ch + FOOTER_TOP), vec2(full.width() - 2.0 * MARGIN, Self::footer_height(ui)));
        let lf = Self::change_later_font();
        let lines = wrap_lines(ui, s::IconPackPrompt_ChangeLater(), &lf, Self::footer_text_width(ui));
        let lh = line_height(ui, &lf);
        let mut y = center_y(footer, lh * lines.len().max(1) as f32);
        for line in &lines {
            ui.painter().galley(pos2(footer.min.x, y), galley(ui, line, lf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            y += lh;
        }
        let b = Self::continue_button();
        let text = s::AfterCaptureWindow_Continue();
        let bw = b.width(ui, text);
        let rect = Rect::from_min_size(pos2(footer.max.x - bw, center_y(footer, 40.0)), vec2(bw, 40.0));
        // `IsDefault="True"`: Enter is Continue.
        if b.show(ui, rect, ui.id().with("continue"), text, true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            out.push(Effect::Close);
        }
        out
    }

    /// `OnClosed`: not asked again, and saved.
    fn closed(&mut self, cx: &mut Cx) -> Vec<Effect> {
        cx.settings.ShowIconPackPrompt = false;
        vec![Effect::SaveSettings]
    }
}
