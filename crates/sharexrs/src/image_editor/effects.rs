//! The image editor's image effects: the effect browser
//! (`EffectBrowserPanel` in `EditorView`'s overlay: the `overlay-scrim`
//! over the whole view and the 800px `floating-panel` 125px from the top)
//! and an effect's dialog (`SchemaDrivenEffectDialog` in the right sidebar,
//! `Border.editor-sidebar.right`) with its live preview
//! (`MainViewModel.EffectPreview`). The catalogue, the parameters and the
//! effects themselves are `sharex_image_editor::effects`; the browser's
//! search, recent and favourite effects `effects::browser`.
//!
//! As `EditorView.EffectsHost` routes them: editor operations with
//! parameters (crop, resize, resize canvas, rotate, auto crop) open their
//! dialog and apply through `EditorCore` (annotations moved with the
//! image), those without (rotate 90/180, flips) and the `ApplyImmediately`
//! effects apply at once, the others open their dialog and commit their
//! latest preview.

use std::path::PathBuf;

use egui::{Color32, CornerRadius, Id, Pos2, Rect, Sense, Ui, pos2, vec2};
use sharex_capture::editor_popups::{ColorDropdown, PopupResult};
use sharex_image_editor::editor::EditorCore;
use sharex_image_editor::effects::browser::{Browser, Item, Section, VisibleSection};
use sharex_image_editor::effects::{self as fx, EffectCategory, EffectDefinition, EffectPreview, EffectValues, ParamKind, ParamValue, SkColor};
use sharex_image_editor::options::ImageEditorOptions;
use sharex_skia::Pixmap;
use sharex_strings::image_editor as ie;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, ComboBox, ComboItem, DecimalUpDown, Slider, TextBox, ToggleSwitch};
use sharex_ui::keys::app_icons;
use sharex_ui::theme::Thickness;
use sharex_ui::{Palette, icons, widgets};

/// `Border.editor-sidebar.open`: 282 wide (a 1px left border).
pub const SIDEBAR: f32 = 282.0;
/// The sidebar's `StackPanel`: `Width="250"`, `Margin="16"`.
const CONTENT: f32 = 250.0;
const MARGIN: f32 = 16.0;
/// The browser's `floating-panel`: `Width="800"`, `Margin="16,125,16,125"`.
const PANEL_WIDTH: f32 = 800.0;
const PANEL_TOP: f32 = 125.0;
/// `Button.effect-item`: `Height="40"` (ShareX's `Button`), `Margin="4"`.
const ITEM: f32 = 40.0;

/// `EffectBrowserLocalization.GetEffectBrowserLabel`.
pub fn browser_label(def: &EffectDefinition) -> String {
    let dialog = def.browser_label.ends_with("...");
    let name = effect_name(def.id, def.browser_label.strip_suffix("...").unwrap_or(&def.browser_label));
    if dialog { format!("{name}...") } else { name.to_string() }
}

/// `EffectBrowserLocalization.GetEffectName`.
pub fn effect_name<'a>(id: &str, fallback: &'a str) -> &'a str {
    ie::lookup(&format!("EffectBrowserPanel_Effect_{id}")).unwrap_or(fallback)
}

/// `EffectBrowserLocalization.GetCategoryName`.
fn category_name(c: EffectCategory) -> &'static str {
    ie::lookup(&format!("EffectBrowserPanel_Category_{}", c.name())).unwrap_or(c.name())
}

/// What the browser or a dialog asks the editor for.
pub enum Action {
    /// `ExecuteCommand` of an effect item (or a favourites menu item).
    Run(&'static str),
    /// A right click changed Recent or Favorites (persisted).
    Changed,
    /// `CloseEffectsPanel` (the scrim pressed).
    Close,
    /// The dialog's Apply.
    Apply,
    /// The dialog's Cancel.
    Cancel,
    /// The colour popup's screen colour picker for parameter `i`.
    PickScreenColor(usize),
}

/// `SchemaDrivenEffectDialog` with its preview session.
pub struct EffectDialog {
    pub def: &'static EffectDefinition,
    pub values: EffectValues,
    preview: Option<EffectPreview>,
    /// The latest preview as the canvas shows it, and its sequence number.
    pub shown: Option<(u64, Pixmap)>,
    seq: u64,
    /// A value changed: the preview runs again.
    pending: bool,
    scroll: f32,
    /// The open colour popup: the parameter, its dropdown, its button.
    color: Option<(usize, ColorDropdown, Rect)>,
    /// The text boxes' text.
    texts: Vec<String>,
}

impl EffectDialog {
    /// `new SchemaDrivenEffectDialog(definition)` and `StartEffectPreview`
    /// on the editor's image; crop and resize start at the image's size
    /// (`ShowEditorOperationDialog`).
    pub fn new(def: &'static EffectDefinition, core: &EditorCore) -> Self {
        let mut values = def.default_values();
        if matches!(def.editor_operation(), Some(fx::EditorOperation::CropImage | fx::EditorOperation::ResizeImage)) {
            let (w, h) = (core.source().width(), core.source().height());
            for p in &def.parameters {
                if matches!(p.kind, ParamKind::Numeric { .. }) {
                    if p.key.eq_ignore_ascii_case("width") {
                        values.set(p.key, ParamValue::Number(w as f64));
                    } else if p.key.eq_ignore_ascii_case("height") {
                        values.set(p.key, ParamValue::Number(h as f64));
                    }
                }
            }
        }
        let texts = def
            .parameters
            .iter()
            .map(|p| match values.get(p.key) {
                Some(ParamValue::Text(t)) => t.clone(),
                _ => String::new(),
            })
            .collect();
        // `OnAttachedToVisualTree`: the first preview.
        Self { def, values, preview: Some(EffectPreview::start(core)), shown: None, seq: 0, pending: true, scroll: 0.0, color: None, texts }
    }

    /// `PreviewEffect`: the effect on the image before it; an effect that
    /// throws keeps the last preview (`ReportError`).
    pub fn update_preview(&mut self) {
        if !std::mem::take(&mut self.pending) {
            return;
        }
        let Some(p) = self.preview.as_mut() else { return };
        match p.preview(self.def, &self.values) {
            Ok(b) => {
                self.seq += 1;
                self.shown = Some((self.seq, fx::to_pixmap(b)));
            }
            Err(e) => eprintln!("Image editor: failed to render live effect preview ({}): {e}", self.def.id),
        }
    }

    /// A change waits for its preview.
    pub fn is_pending(&self) -> bool {
        self.pending && self.preview.is_some()
    }

    /// The preview session, for Apply.
    pub fn take_preview(&mut self) -> Option<EffectPreview> {
        self.preview.take()
    }

    /// `Title`: the localised effect name.
    pub fn title(&self) -> &'static str {
        effect_name(self.def.id, self.def.name)
    }

    /// Sets a parameter (by key) as its control would.
    pub fn set(&mut self, key: &str, v: ParamValue) {
        if self.values.get(key) != Some(&v) {
            self.values.set(key, v.clone());
            self.pending = true;
            if let (Some(i), ParamValue::Text(t)) = (self.def.parameters.iter().position(|p| p.key == key), &v) {
                self.texts[i] = t.clone();
            }
        }
    }

    /// The screen colour picker's colour for parameter `i`.
    pub fn set_color(&mut self, i: usize, c: [u8; 4]) {
        if let Some(p) = self.def.parameters.get(i) {
            let [r, g, b, a] = c;
            self.set(p.key, ParamValue::Color(SkColor::rgba(r, g, b, a)));
        }
    }

    /// Whether `p` is on the dialog's colour popup.
    pub fn popup_contains(&self, p: Pos2) -> bool {
        self.color.as_ref().is_some_and(|(_, d, _)| d.contains(p))
    }

    /// For `--render`: the colour popup of parameter `i` open.
    pub fn open_color(&mut self, i: usize) {
        let mut d = ColorDropdown::default();
        let c = match self.def.parameters.get(i).and_then(|p| self.values.get(p.key)) {
            Some(ParamValue::Color(c)) => c.rgba_array(),
            _ => return,
        };
        d.toggle(c);
        self.color = Some((i, d, Rect::NOTHING));
    }

    /// The content's height (the scroll viewer's extent).
    fn content_height(&self, ui: &Ui) -> f32 {
        let label_h = widgets::line_height(ui, &label_font());
        let head = widgets::line_height(ui, &widgets::font(20.0, 400)) + 16.0 + 8.0;
        let params: f32 = self
            .def
            .parameters
            .iter()
            .map(|p| match p.kind {
                ParamKind::Slider { .. } => 24.0 + 4.0 + 50.0 + 10.0,
                ParamKind::Checkbox { .. } => 32.0 + 10.0,
                _ => label_h + 4.0 + 40.0 + 10.0,
            })
            .sum();
        MARGIN + head + params + 16.0 + 40.0 + MARGIN
    }

    /// Draws the sidebar at the right of `view`.
    pub fn show(&mut self, ui: &mut Ui, p: &Palette, view: Rect, modal: bool) -> Option<Action> {
        let mut out = None;
        let rect = Rect::from_min_max(pos2(view.max.x - SIDEBAR, view.min.y), view.max);
        ui.painter().rect_filled(rect, 0.0, p.background_panel);
        ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(1.0, rect.height())), 0.0, p.border);
        let viewport = Rect::from_min_max(pos2(rect.min.x + 1.0, rect.min.y), rect.max);
        let extent = self.content_height(ui);
        let max_scroll = (extent - viewport.height()).max(0.0);
        let combo_open = (0..self.def.parameters.len()).any(|i| ComboBox::is_open(ui.ctx(), Id::new(("fx-enum", self.def.id, i))));
        if !modal && !combo_open && ui.rect_contains_pointer(viewport) {
            self.scroll -= ui.ctx().input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(viewport));
        child.set_clip_rect(viewport.intersect(ui.clip_rect()));
        if modal {
            child.disable();
        }
        let ui = &mut child;
        let x = viewport.min.x + MARGIN;
        let top = (viewport.min.y + MARGIN - self.scroll).round();
        let painter = ui.painter().clone();
        let lf = label_font();
        let lh = widgets::line_height(ui, &lf);

        // The title row: the effect's icon and name (20px, Normal, accent).
        let head_font = widgets::font(20.0, 400);
        let head_h = widgets::line_height(ui, &head_font);
        let icon_r = Rect::from_min_size(pos2(x, top + ((head_h + 16.0 - 2.0 - 20.0) / 2.0).round_ties_even() + 2.0), vec2(20.0, 20.0));
        icons::paint(&painter, icon_r, self.def.icon, p.text_secondary);
        let g = widgets::galley(ui, self.title(), head_font, p.accent_text, f32::INFINITY);
        painter.galley(pos2(x + 20.0 + 8.0, top), g, p.accent_text);
        let mut y = top + head_h + 16.0 + 8.0;

        let label = |ui: &Ui, y: f32, text: &str| {
            let g = widgets::galley(ui, text, lf.clone(), p.text_secondary, f32::INFINITY);
            ui.painter().galley(pos2(x, y), g, p.text_secondary);
        };
        let def = self.def;
        for (i, param) in def.parameters.iter().enumerate() {
            let value = self.values.get(param.key).cloned().unwrap_or_else(|| param.default_value());
            let id = Id::new(("fx-param", def.id, i));
            match &param.kind {
                ParamKind::Slider { min, max, tick, snap, .. } => {
                    let row = Rect::from_min_size(pos2(x, y), vec2(CONTENT, 24.0));
                    label(ui, y, param.label);
                    let v = value.as_number().unwrap_or(0.0);
                    super::background::capsule(ui, p, row, &format_0_hash(v));
                    let r = Rect::from_min_size(pos2(x, y + 24.0 + 4.0), vec2(CONTENT, 50.0));
                    let mut f = v as f32;
                    let s = Slider { minimum: *min as f32, maximum: *max as f32, step: if *snap { *tick as f32 } else { 0.0 }, enabled: true };
                    if s.show(ui, r, id, &mut f) {
                        self.set(param.key, ParamValue::Number(f as f64));
                    }
                    y += 24.0 + 4.0 + 50.0 + 10.0;
                }
                ParamKind::Checkbox { .. } => {
                    // `LabeledToggleSwitch`: the label and the switch, the
                    // whole row a toggle button.
                    let row = Rect::from_min_size(pos2(x, y), vec2(CONTENT, 32.0));
                    let on = matches!(value, ParamValue::Bool(true));
                    let resp = ui.interact(row, id, Sense::click());
                    let g = widgets::galley(ui, param.label, lf.clone(), p.text_secondary, f32::INFINITY);
                    painter.galley(pos2(x, widgets::center_y(row, lh)), g, p.text_secondary);
                    let _ = ToggleSwitch { enabled: true }.show(ui, pos2(row.max.x + 12.0 - ToggleSwitch::SIZE.x, row.min.y), id.with("switch"), &mut on.clone());
                    if resp.clicked() {
                        self.set(param.key, ParamValue::Bool(!on));
                    }
                    y += 32.0 + 10.0;
                }
                kind => {
                    label(ui, y, param.label);
                    let r = Rect::from_min_size(pos2(x, y + lh + 4.0), vec2(CONTENT, 40.0));
                    match kind {
                        ParamKind::Enum { options, .. } => {
                            let items: Vec<ComboItem> = options.iter().map(|o| ComboItem::text(*o)).collect();
                            let sel = match value {
                                ParamValue::Choice(c) => Some(c),
                                _ => None,
                            };
                            if let Some(c) = (ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: !modal }).show(ui, r, Id::new(("fx-enum", def.id, i)), &items, sel) {
                                self.set(param.key, ParamValue::Choice(c));
                            }
                        }
                        ParamKind::Color { .. } => {
                            let c = match value {
                                ParamValue::Color(c) => color32(c),
                                _ => Color32::TRANSPARENT,
                            };
                            // `ColorPickerDropdown`'s button, as wide as its
                            // content: the 24px swatch, 6, the 10px chevron in
                            // `Padding="10,6"` and the border.
                            let br = Rect::from_min_size(r.min, vec2(1.0 + 10.0 + 24.0 + 6.0 + 10.0 + 10.0 + 1.0, 40.0));
                            let resp = ui.interact(br, id, Sense::click());
                            let open = self.color.as_ref().is_some_and(|(k, d, _)| *k == i && d.open);
                            sharex_capture::region::options_row::color_dropdown_button(ui, &painter, p, br, c, resp.hovered(), open);
                            if let Some((k, _, anchor)) = self.color.as_mut()
                                && *k == i
                            {
                                *anchor = br;
                            }
                            if resp.clicked() {
                                if open {
                                    self.color = None;
                                } else {
                                    let mut d = ColorDropdown::default();
                                    d.toggle(match value {
                                        ParamValue::Color(c) => c.rgba_array(),
                                        _ => [0; 4],
                                    });
                                    self.color = Some((i, d, br));
                                }
                            }
                        }
                        ParamKind::Numeric { min, max, increment, format, .. } => {
                            let decimals = format.split('.').nth(1).map_or(0, str::len);
                            let mut v = value.as_number().unwrap_or(0.0);
                            let d = DecimalUpDown { minimum: *min, maximum: *max, increment: *increment, decimals, enabled: !modal };
                            if d.show(ui, r, id, &mut v) {
                                self.set(param.key, ParamValue::Number(v));
                            }
                        }
                        ParamKind::Text { .. } => {
                            let mut t = std::mem::take(&mut self.texts[i]);
                            TextBox { enabled: !modal, ..Default::default() }.show(ui, r, id, &mut t);
                            self.texts[i] = t.clone();
                            self.set(param.key, ParamValue::Text(t));
                        }
                        ParamKind::FilePath { filter, .. } => {
                            // The path box, 8, the Browse button.
                            let browse = ie::SchemaDrivenEffectDialog_BrowseEllipsis();
                            let b = Button::default();
                            let bw = b.width(ui, browse).ceil();
                            let tr = Rect::from_min_max(r.min, pos2(r.max.x - bw - 8.0, r.max.y));
                            let mut t = std::mem::take(&mut self.texts[i]);
                            TextBox { enabled: !modal, ..Default::default() }.show(ui, tr, id, &mut t);
                            if b.show(ui, Rect::from_min_max(pos2(r.max.x - bw, r.min.y), r.max), id.with("browse"), browse, !modal).clicked()
                                && let Some(path) = pick_file(param.label, *filter)
                            {
                                t = path.display().to_string();
                            }
                            self.texts[i] = t.clone();
                            self.set(param.key, ParamValue::Text(t));
                        }
                        _ => {}
                    }
                    y += lh + 4.0 + 40.0 + 10.0;
                }
            }
        }
        // Cancel and Apply (`Classes="active"`), `ColumnDefinitions="*,*"`
        // 16 below.
        y += 16.0;
        let half = CONTENT / 2.0;
        let cancel = Rect::from_min_size(pos2(x, y), vec2(half - 4.0, 40.0));
        let apply = Rect::from_min_max(pos2(x + half + 4.0, y), pos2(x + CONTENT, y + 40.0));
        if Button::default().show(ui, cancel, Id::new(("fx-cancel", def.id)), ie::SchemaDrivenEffectDialog_Cancel(), !modal).clicked() {
            out = Some(Action::Cancel);
        }
        if super::background::active_button(ui, p, apply, Id::new(("fx-apply", def.id)), ie::SchemaDrivenEffectDialog_Apply()) && !modal {
            out = Some(Action::Apply);
        }
        scroll_bar(ui, viewport, extent, self.scroll, max_scroll);
        out
    }

    /// The open colour popup (over everything, within `bounds`).
    pub fn show_popup(&mut self, ui: &Ui, p: &Palette, bounds: Rect) -> Option<Action> {
        let (i, mut d, anchor) = self.color.take()?;
        let param = &self.def.parameters[i];
        let before = match self.values.get(param.key) {
            Some(ParamValue::Color(c)) => c.rgba_array(),
            _ => [0; 4],
        };
        let mut c = before;
        let r: PopupResult = d.show(ui, p, anchor, bounds, &format!("fx-color-{i}"), &mut c);
        if c != before {
            self.set_color(i, c);
        }
        let out = r.pick_screen_color.then_some(Action::PickScreenColor(i));
        if d.open {
            self.color = Some((i, d, anchor));
        }
        out
    }
}

fn color32(c: SkColor) -> Color32 {
    let [r, g, b, a] = c.rgba_array();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// The dialog's `TextBlock`s: ShareX's style (Segoe UI 13, SemiBold,
/// `Text.Secondary`).
fn label_font() -> egui::FontId {
    widgets::font(13.0, 600)
}

/// `StringFormat={}{0:0.##}`: at most two decimals.
pub fn format_0_hash(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// `OnBrowseFilePathClick`: "Select {Label}", the parameter's filter
/// (`Name|*.a;*.b`) or every file.
fn pick_file(label: &str, filter: Option<&str>) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new().set_title(format!("Select {label}"));
    if let Some(f) = filter {
        let parts: Vec<&str> = f.split('|').map(str::trim).filter(|s| !s.is_empty()).collect();
        if parts.len() >= 2 {
            let exts: Vec<&str> = parts[1].split(';').map(|s| s.trim().trim_start_matches("*.")).filter(|s| !s.is_empty()).collect();
            if !exts.is_empty() {
                d = d.add_filter(parts[0], &exts);
            }
        }
    }
    d.pick_file()
}

/// Fluent's collapsed vertical scroll bar: a 2px thumb 2px in from the
/// right, between the 16px line buttons.
fn scroll_bar(ui: &Ui, viewport: Rect, extent: f32, scroll: f32, max_scroll: f32) {
    if max_scroll <= 0.0 {
        return;
    }
    let thumb = sharex_ui::theme::fluent(ui.ctx()).base_medium;
    let track = (viewport.min.y + 16.0, viewport.max.y - 16.0);
    let len = track.1 - track.0;
    let th = (len * viewport.height() / extent).max(16.0);
    let ty = track.0 + (len - th) * scroll / max_scroll;
    ui.painter().rect_filled(Rect::from_min_size(pos2(viewport.max.x - 4.0, ty), vec2(2.0, th)), CornerRadius::same(1), thumb);
}

/// `IsEffectsPanelOpen` with its browser and dialog
/// (`EffectsPanelContent`).
pub struct Effects {
    /// `IsEffectsPanelOpen`.
    pub open: bool,
    pub browser: Browser,
    /// The options' recent and favourite effects are loaded (`SetOptions`).
    loaded: bool,
    scroll: f32,
    /// `FocusSearchBox`: focused (and cleared) when the browser opens.
    focus_search: bool,
    /// The browser opened this frame: the press that opened it isn't one
    /// on its scrim (it wasn't there yet).
    fresh: bool,
    /// `EffectsPanelContent`.
    pub dialog: Option<EffectDialog>,
    /// The floating panel as last drawn.
    pub panel: Option<Rect>,
    /// The favourites menu's effects, in its order.
    pub menu: Vec<&'static str>,
}

impl Default for Effects {
    fn default() -> Self {
        Self { open: false, browser: Browser::new(browser_label), loaded: false, scroll: 0.0, focus_search: false, fresh: false, dialog: None, panel: None, menu: Vec::new() }
    }
}

impl Effects {
    /// `SetOptions`: the recent and favourite effects from the options
    /// (written back resolved).
    pub fn load(&mut self, o: &mut ImageEditorOptions) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let (recent, favorites) = self.browser.load(&o.RecentEffects, &o.FavoriteEffects);
        o.RecentEffects = recent;
        o.FavoriteEffects = favorites;
    }

    /// `PersistRecentToOptions` / `PersistFavoritesToOptions`.
    pub fn persist(&self, o: &mut ImageEditorOptions) {
        o.RecentEffects = self.browser.recent_ids();
        o.FavoriteEffects = self.browser.favorite_ids();
    }

    /// `IsEffectBrowserVisible`.
    pub fn browser_visible(&self) -> bool {
        self.open && self.dialog.is_none()
    }

    /// `IsRightEffectsSidebarVisible`.
    pub fn sidebar_visible(&self) -> bool {
        self.open && self.dialog.is_some()
    }

    /// The browser shown (`EffectsPanelContent = null`), its search
    /// cleared and focused (`FocusSearchBox`).
    pub fn show_browser(&mut self) {
        self.dialog = None;
        self.open = true;
        self.browser.search.clear();
        self.focus_search = true;
        self.fresh = true;
    }

    /// `CloseEffectsPanel` (a dialog's preview dropped).
    pub fn close(&mut self) {
        self.open = false;
        self.dialog = None;
        self.panel = None;
    }

    /// The preview the canvas shows, if a dialog has one.
    pub fn preview(&self) -> Option<&(u64, Pixmap)> {
        self.dialog.as_ref().filter(|_| self.open).and_then(|d| d.shown.as_ref())
    }

    /// For `--render`: the browser scrolled, its search box not focused
    /// (no caret: ShareX's shots catch it blinking off).
    pub fn set_scroll(&mut self, y: f32) {
        self.scroll = y;
        self.focus_search = false;
    }

    /// The browser over `view` (the `overlay-scrim` and the panel).
    pub fn show_browser_ui(&mut self, ui: &mut Ui, p: &Palette, view: Rect, modal: bool) -> Option<Action> {
        let mut out = None;
        ui.painter().rect_filled(view, 0.0, p.overlay_soft);
        let sections = self.browser.visible();
        let width = PANEL_WIDTH.min(view.width() - 32.0);
        let left = view.min.x + 16.0 + ((view.width() - 32.0 - width) / 2.0).round_ties_even();
        let inner_w = width - 2.0;
        let item_area_w = inner_w - 32.0;
        let item_font = widgets::font(13.0, 400);
        let item_lh = widgets::line_height(ui, &item_font);
        let head_font = widgets::font(18.0, 600);
        let head_h = widgets::line_height(ui, &head_font);
        // Each section's rows of items (a `WrapPanel` of 4px-margined
        // buttons) and its header row's height (the hint's 22px box).
        let laid: Vec<LaidSection> = sections
            .into_iter()
            .map(|s| {
                let mut rows: Vec<Vec<(Item, f32)>> = vec![Vec::new()];
                let mut x = 0.0;
                for it in &s.items {
                    let w = 10.0 + 20.0 + 8.0 + widgets::text_width(ui, &it.name, item_font.clone()).ceil() + 10.0 + 8.0;
                    if x + w > item_area_w && x > 0.0 {
                        rows.push(Vec::new());
                        x = 0.0;
                    }
                    rows.last_mut().unwrap().push((it.clone(), w));
                    x += w;
                }
                let hrow = if matches!(s.section, Section::Category(_)) { head_h } else { head_h.max(22.0) };
                (s, hrow, rows)
            })
            .collect();
        let section_h = |hrow: f32, rows: usize| 8.0 + hrow + 8.0 + rows as f32 * (ITEM + 8.0) + 8.0;
        let list_h: f32 = 8.0 + laid.iter().map(|(_, h, r)| section_h(*h, r.len())).sum::<f32>() + 16.0;
        let max_h = (view.height() - 2.0 * PANEL_TOP).max(100.0);
        let height = (2.0 + 16.0 + 40.0 + list_h).min(max_h);
        let rect = Rect::from_min_size(pos2(left, view.min.y + PANEL_TOP), vec2(width, height));
        self.panel = Some(rect);
        // `BoxShadow="0 22 40 -22 #66000000"`.
        let shadow = egui::epaint::Shadow { offset: [0, 22], blur: 40, spread: 0, color: Color32::from_black_alpha(0x66) };
        ui.painter().add(shadow.as_shape(rect.shrink(22.0), CornerRadius::same(8)));
        border::border(ui.painter(), rect, 8.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
        let inner = rect.shrink(1.0);
        // The search box (`Margin="16,16,16,0"`, 40 high, `Padding="8,0"`).
        let search_r = Rect::from_min_size(pos2(inner.min.x + 16.0, inner.min.y + 16.0), vec2(inner_w - 32.0, 40.0));
        let placeholder = ie::EffectBrowserPanel_SearchCount().replace("{0}", &self.browser.total().to_string());
        let tb = TextBox { placeholder: &placeholder, padding: Thickness::new(8.0, 0.0, 8.0, 0.0), enabled: !modal, ..Default::default() };
        let before = self.browser.search.clone();
        let resp = tb.show(ui, search_r, Id::new("fx-search"), &mut self.browser.search);
        if std::mem::take(&mut self.focus_search) {
            resp.request_focus();
        }
        if self.browser.search != before {
            self.scroll = 0.0;
        }
        // The categories, scrolled.
        let viewport = Rect::from_min_max(pos2(inner.min.x, search_r.max.y), inner.max);
        let max_scroll = (list_h - viewport.height()).max(0.0);
        if !modal && ui.rect_contains_pointer(viewport) {
            self.scroll -= ui.ctx().input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let clip = ui.painter().with_clip_rect(viewport.intersect(ui.clip_rect()));
        let mut y = viewport.min.y + 8.0 - self.scroll;
        let x0 = inner.min.x + 16.0;
        let hover = ui.ctx().pointer_hover_pos().unwrap_or(Pos2::new(-1.0, -1.0));
        let mut changed = false;
        for (s, hrow, rows) in &laid {
            // The header (`Margin="8,8,0,8"`): the name and count, and for
            // Recent and Favorites the hint's light bulb 6 after it.
            let hy = y + 8.0;
            let hrect = Rect::from_min_size(pos2(x0 + 8.0, hy), vec2(1.0, *hrow));
            let name = match s.section {
                Section::Recent => ie::EffectBrowserPanel_Recent(),
                Section::Favorites => ie::EffectBrowserPanel_Favorites(),
                Section::Category(c) => category_name(c),
            };
            let header = format!("{name} ({})", s.count);
            let hw = widgets::text_width(ui, &header, head_font.clone()).ceil();
            let g = widgets::galley(ui, &header, head_font.clone(), p.text_secondary, f32::INFINITY);
            clip.galley(pos2(x0 + 8.0, widgets::center_y(hrect, head_h)), g, p.text_secondary);
            if !matches!(s.section, Section::Category(_)) {
                let tip = Rect::from_min_size(pos2(x0 + 8.0 + hw + 6.0, widgets::center_y(hrect, 22.0)), vec2(22.0, 22.0));
                // The 18px light bulb, `Margin="0,1,0,0"`, centred in the
                // box; ShareX's lands 1px right of the 2px centring offset
                // (measured on Recent's and Favorites' headers in both
                // themes: the header text is identical, the bulb isn't).
                let ir = Rect::from_min_size(pos2(tip.min.x + 3.0, tip.min.y + 2.0), vec2(18.0, 18.0));
                icons::paint(&clip, ir, app_icons::Lightbulb, p.text_secondary);
                let hint = if s.section == Section::Recent { ie::EffectBrowserPanel_RecentHint() } else { ie::EffectBrowserPanel_FavoritesHint() };
                if !modal && viewport.contains(hover) && tip.intersect(viewport).is_positive() {
                    let _ = ui.interact(tip.intersect(viewport), Id::new(("fx-tip", name)), Sense::hover()).on_hover_text(hint);
                }
            }
            let mut iy = hy + hrow + 8.0;
            for row in rows {
                let mut ix = x0;
                for (it, w) in row {
                    let b = Rect::from_min_size(pos2(ix + 4.0, iy + 4.0), vec2(w - 8.0, ITEM));
                    ix += w;
                    if b.max.y < viewport.min.y || b.min.y > viewport.max.y {
                        continue;
                    }
                    let hit = b.intersect(viewport);
                    let id = Id::new(("fx-item", s.section_key(), it.id));
                    let resp = ui.interact(hit, id, if modal { Sense::hover() } else { Sense::click() });
                    let over = !modal && (resp.hovered() || resp.is_pointer_button_down_on());
                    clip.rect_filled(b, CornerRadius::same(4), if over { p.control_background_hover } else { p.control_background });
                    // The 16px icon in its 20px column, 8, the name.
                    let ic = Rect::from_min_size(pos2(b.min.x + 10.0 + 2.0, b.min.y + ((ITEM - 16.0) / 2.0)), vec2(16.0, 16.0));
                    icons::paint(&clip, ic, it.icon, p.text);
                    let g = widgets::galley(ui, &it.name, item_font.clone(), p.text, f32::INFINITY);
                    clip.galley(pos2(b.min.x + 10.0 + 20.0 + 8.0, widgets::center_y(b, item_lh)), g, p.text);
                    let resp = if modal { resp } else { resp.on_hover_text(it.description) };
                    if resp.clicked() {
                        out = Some(Action::Run(it.id));
                    }
                    if resp.secondary_clicked() {
                        self.browser.right_click(s.section, it.id);
                        changed = true;
                    }
                }
                iy += ITEM + 8.0;
            }
            y += section_h(*hrow, rows.len());
        }
        scroll_bar(ui, viewport, list_h, self.scroll, max_scroll);
        if changed && out.is_none() {
            out = Some(Action::Changed);
        }
        // The scrim pressed (outside the panel) closes the browser.
        if !std::mem::take(&mut self.fresh)
            && !modal
            && ui.ctx().input(|i| i.pointer.any_pressed())
            && let Some(at) = ui.ctx().input(|i| i.pointer.interact_pos())
            && view.contains(at)
            && !rect.contains(at)
        {
            out = Some(Action::Close);
        }
        out
    }
}

/// A section laid out: its header row's height and its rows of items
/// (each with its width, margins included).
type LaidSection = (VisibleSection, f32, Vec<Vec<(Item, f32)>>);

trait SectionKey {
    fn section_key(&self) -> u8;
}

impl SectionKey for VisibleSection {
    fn section_key(&self) -> u8 {
        match self.section {
            Section::Recent => 0,
            Section::Favorites => 1,
            Section::Category(c) => 2 + c as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_dotnet() {
        assert_eq!(format_0_hash(15.0), "15");
        assert_eq!(format_0_hash(0.5), "0.5");
        assert_eq!(format_0_hash(1.234), "1.23");
        assert_eq!(format_0_hash(-0.001), "0");
    }

    #[test]
    fn labels_are_localised_names() {
        let d = fx::find("resize_image").unwrap();
        assert_eq!(browser_label(d), "Resize image...");
        assert_eq!(effect_name("invert", "x"), "Invert");
    }
}
