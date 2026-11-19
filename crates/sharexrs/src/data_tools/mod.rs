//! ShareX.Tools' Data windows (docs/TOOLS.md, "The Data and System windows
//! in ShareXrs"): the hash checker, index folder, metadata and Analyze
//! image (and its options), each a [`Dialog`](crate::dialogs::Dialog) over
//! its engine in `sharex-tools`. The QR code window is
//! `crate::dialogs::qr_code`, OCR `crate::ocr_window`. They share the
//! Images windows' blocks (`crate::image_tools`); what's theirs is here.

pub mod analyze_image;
pub mod analyze_options;
pub mod hash_checker;
pub mod index_folder;
pub mod metadata;
pub mod open;
pub mod render;

use egui::{Id, Ui};
use sharex_ui::controls::{ComboBox, ComboItem};

use crate::dialogs::{Cx, Dialog, Effect, Spec};

/// An open combo box's dropdown on its own, as the reference shots of
/// ShareX's popup windows show it (`--render <window> --state <x>-menu`):
/// the combo box's items, its selection and its width in the window.
pub struct ComboPopupShot {
    pub items: Vec<ComboItem>,
    pub selected: Option<usize>,
    /// The combo box's width (or how to measure it: `None` for a
    /// `HorizontalAlignment="Left"` combo as wide as its selected item).
    pub width: Option<f32>,
    pub combo: ComboBox,
    /// An editable combo box's text (its width comes from it).
    pub editable_text: Option<String>,
    /// The combo box's `PlaceholderText`, nothing selected (its width).
    pub placeholder: Option<&'static str>,
}

impl ComboPopupShot {
    pub fn new(items: Vec<ComboItem>, selected: Option<usize>, width: Option<f32>) -> Self {
        Self { items, selected, width, combo: ComboBox { padding: sharex_ui::theme::Thickness::hv(8.0, 6.0), enabled: true }, editable_text: None, placeholder: None }
    }
}

impl Dialog for ComboPopupShot {
    fn spec(&self) -> Spec {
        Spec::sized("", 200.0, 200.0, 0.0, 0.0)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        // ShareX's popup is a window of its own whose rounded corners come
        // out black in its capture.
        ui.painter().rect_filled(ui.max_rect(), 0.0, egui::Color32::BLACK);
        let width = match (&self.width, &self.editable_text) {
            (Some(w), _) => *w,
            (None, Some(t)) => self.combo.editable_width(ui, t),
            (None, None) if self.placeholder.is_some() && self.selected.is_none() => self.combo.placeholder_width(ui, self.placeholder.unwrap_or("")),
            (None, None) => self.combo.auto_width(ui, &self.items, self.selected),
        };
        sharex_ui::controls::dropdown_alone(ui, Id::new("popup-shot"), width, &self.items, self.selected);
        Vec::new()
    }
}

/// The centred notice `StackPanel` (`MaxWidth`, `Spacing="10"`): an
/// `IconPresenter` of `icon_size` in the secondary text colour over a
/// `TextBlock` (Normal, secondary, `TextAlignment="Center"`,
/// `TextWrapping="Wrap"`), centred in `area`.
pub fn notice(ui: &Ui, area: egui::Rect, key: &str, icon_size: f32, max_width: f32, text: &str) {
    use egui::{Vec2, pos2};
    use sharex_ui::widgets::{galley, text_width};
    let p = sharex_ui::theme::palette(ui.ctx());
    let f = crate::image_tools::normal_font();
    let width = max_width.min(area.width());
    let lines = sharex_ui::controls::wrap_lines(ui, text, &f, width);
    let block_w = lines.iter().map(|l| text_width(ui, l, f.clone())).fold(0.0, f32::max);
    let text_h = crate::video_tools::wrapped_text(ui, pos2(0.0, 0.0), width, text, f.clone(), p.text_secondary, false);
    let stack_w = block_w.max(icon_size);
    let total = icon_size + 10.0 + text_h;
    let x0 = area.min.x + ((area.width() - stack_w) / 2.0).round_ties_even();
    let y0 = area.min.y + ((area.height() - total) / 2.0).round_ties_even();
    let ir = egui::Rect::from_min_size(pos2(x0 + ((stack_w - icon_size) / 2.0).round_ties_even(), y0), Vec2::splat(icon_size));
    sharex_ui::icons::paint(ui.painter(), ir, key, p.text_secondary);
    let exact = ui.fonts_mut(|fo| fo.row_height(&f));
    let bx = x0 + ((stack_w - block_w) / 2.0).round_ties_even();
    for (i, line) in lines.iter().enumerate() {
        let w = text_width(ui, line, f.clone());
        let g = galley(ui, line, f.clone(), p.text_secondary, f32::INFINITY);
        let y = y0 + icon_size + 10.0 + (exact * i as f32).round();
        ui.painter().galley(pos2(bx + ((block_w - w) / 2.0).round_ties_even(), y), g, p.text_secondary);
    }
}

/// `TaskSettings.ToolsSettingsReference`: the task's own tools settings,
/// or the default task's when it uses those (`UseDefaultToolsSettings`).
pub fn tools_settings(task: &sharex_settings::TaskSettings, defaults: &sharex_settings::TaskSettings) -> sharex_settings::TaskSettingsTools {
    let tools = if task.UseDefaultToolsSettings { defaults.ToolsSettings.as_ref() } else { task.ToolsSettings.as_ref() };
    tools.cloned().unwrap_or_default()
}

/// Writes one of the tools settings back where [`tools_settings`] read it
/// (the window's own task too, so it reads it back the same).
pub fn save_tools_setting(cx: &mut Cx, task: &mut sharex_settings::TaskSettings, set: impl Fn(&mut sharex_settings::TaskSettingsTools)) {
    if task.UseDefaultToolsSettings {
        set(cx.settings.DefaultTaskSettings.ToolsSettings.get_or_insert_default());
    }
    if let Some(t) = task.ToolsSettings.as_mut() {
        set(t);
    }
}

/// `string.Format(format, args)`: `{0}`, `{1:N0}`, `{2:0.##}`… . An
/// argument that is a whole or decimal number (written as Rust prints it)
/// formats as that number, so a resource's format specifier applies
/// (`MetadataViewModel_Tag_count`'s `{0:N0} tags`); other arguments are
/// strings.
pub fn format(format: &str, args: &[&str]) -> String {
    use sharex_tools::text::Arg;
    let args: Vec<Arg> = args
        .iter()
        .map(|a| match (a.parse::<i64>(), a.parse::<f64>()) {
            (Ok(n), _) if n.to_string() == *a => Arg::I64(n),
            (_, Ok(v)) if v.is_finite() && v.to_string() == *a => Arg::F64(v),
            _ => Arg::Str(a.to_string()),
        })
        .collect();
    sharex_tools::text::format_dotnet(format, &args)
}

#[cfg(test)]
mod format_tests {
    use super::format;

    fn culture(s: &str) -> String {
        let c = sharex_helpers::date_format::CultureFormat::current();
        sharex_tools::text::with_separators(s, &c.number_group_separator, &c.number_decimal_separator)
    }

    #[test]
    fn format_specifiers_apply_to_numbers() {
        // DAT-02: the Metadata header showed `{0:N0} tags` literally.
        assert_eq!(format(sharex_strings::tools::MetadataViewModel_Tag_count(), &["22"]), "22 tags");
        assert_eq!(format("{0:N0} tags", &["12345"]), culture("12,345 tags"));
        assert_eq!(format(sharex_strings::tools::AnalyzeImageViewModel_Time_ms(), &["1500"]), culture("Time: 1,500 ms"));
        assert_eq!(format("{0:0.##}%", &["12.345"]), culture("12.35%"));
        // Strings that aren't plain numbers stay as written.
        assert_eq!(format("{0} / {1}", &["007", "a.b"]), "007 / a.b");
        assert_eq!(format("{{{0}}}", &["x"]), "{x}");
    }
}
