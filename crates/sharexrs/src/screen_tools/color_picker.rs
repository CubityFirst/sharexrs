//! Tools › Color picker: `ShareX.HelpersLib/Presentation/ColorPicker/
//! ColorPickerWindow` (`.axaml` and code-behind) and its
//! `AdvancedColorPickerControl`. The colour box and slider in six modes,
//! the colour as name, hex, decimal, RGBA, CMYK and HSB (each editable but
//! the name), the current and previous colours, the Copy menu, recent or
//! standard colours, and picking from the screen or the clipboard. Closing
//! it adds the colour to the recent colours (`HelpersOptions.RecentColors`,
//! saved as `ApplicationConfig.RecentColors`).

use egui::{Color32, Id, Rect, Sense, TextureOptions, Ui, pos2, vec2};
use sharex_strings::helpers_lib as s;
use sharex_tools::color::{self, Cmyk, Color, ColorFormat, Hsb};
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{self, ComboBox, ComboItem, DecimalUpDown, RadioButton, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, MenuEntry, MenuStyle, Placement};
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools_ui;

pub const WIDTH: f32 = 790.0;
pub const MIN_WIDTH: f32 = 760.0;
/// The upper row's panels (`ColumnDefinitions="370,*"`), as tall as the
/// picker's: 300 + 10 + 40 inside 12px padding and a 1px border.
const TOP_HEIGHT: f32 = 376.0;
/// A row of the right panel: an 18px label, 4px, a 40px box (whose
/// `NumericUpDown` measures 41), and `RowSpacing="9"`.
const ROW_PITCH: f32 = 72.0;

/// `DrawStyle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawStyle {
    Hue,
    Saturation,
    Brightness,
    Red,
    Green,
    Blue,
}

impl DrawStyle {
    const ALL: [DrawStyle; 6] = [DrawStyle::Hue, DrawStyle::Saturation, DrawStyle::Brightness, DrawStyle::Red, DrawStyle::Green, DrawStyle::Blue];
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CopyCommand {
    All,
    Rgb,
    Hex,
    Hsb,
    Cmyk,
    Decimal,
}

/// `AdvancedColorPickerControl.FromHsv`.
fn from_hsv(hue: f64, saturation: f64, value: f64, alpha: u8) -> Color {
    let hue = ((hue % 1.0) + 1.0) % 1.0;
    let saturation = saturation.clamp(0.0, 1.0);
    let value = value.clamp(0.0, 1.0);
    let scaled = hue * 6.0;
    let sector = (scaled.floor() as i32) % 6;
    let fraction = scaled - scaled.floor();
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - fraction * saturation);
    let t = value * (1.0 - (1.0 - fraction) * saturation);
    let (r, g, b) = match sector {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        _ => (value, p, q),
    };
    Color::argb(alpha, to_byte(r), to_byte(g), to_byte(b))
}

/// `ToByte`: rounded to even, as `Math.Round`.
fn to_byte(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round_ties_even().clamp(0.0, 255.0) as u8
}

/// The picker control (`AdvancedColorPickerControl`), 342x300: a box and a
/// 30px slider 12px after it.
pub struct AdvancedPicker {
    pub style: DrawStyle,
    color: Color,
    dragging_slider: Option<bool>,
    textures: Option<(egui::TextureHandle, egui::TextureHandle, (u32, DrawStyle))>,
}

impl AdvancedPicker {
    const SLIDER: f32 = 30.0;
    const GAP: f32 = 12.0;

    pub fn new(color: Color) -> Self {
        Self { style: DrawStyle::Hue, color, dragging_slider: None, textures: None }
    }

    /// `SelectedColor`'s setter: a new colour rebuilds the bitmaps.
    pub fn set_color(&mut self, c: &Color) {
        self.color = c.clone();
    }

    fn regions(rect: Rect) -> (Rect, Rect) {
        let bw = (rect.width() - Self::SLIDER - Self::GAP).max(1.0);
        (Rect::from_min_size(rect.min, vec2(bw, rect.height())), Rect::from_min_size(pos2(rect.min.x + bw + Self::GAP, rect.min.y), vec2(Self::SLIDER, rect.height())))
    }

    /// `GetPixelColor`.
    fn pixel(&self, x: f64, y: f64, slider: bool, hsb: &Hsb) -> Color {
        let c = &self.color;
        let b = |v: f64| to_byte(v);
        if slider {
            return match self.style {
                DrawStyle::Hue => from_hsv(1.0 - y, 1.0, 1.0, 255),
                DrawStyle::Saturation => from_hsv(hsb.hue, 1.0 - y, hsb.brightness, 255),
                DrawStyle::Brightness => from_hsv(hsb.hue, hsb.saturation, 1.0 - y, 255),
                DrawStyle::Red => Color::rgb(b(1.0 - y), c.g, c.b),
                DrawStyle::Green => Color::rgb(c.r, b(1.0 - y), c.b),
                DrawStyle::Blue => Color::rgb(c.r, c.g, b(1.0 - y)),
            };
        }
        match self.style {
            DrawStyle::Hue => from_hsv(hsb.hue, x, 1.0 - y, 255),
            DrawStyle::Saturation => from_hsv(x, hsb.saturation, 1.0 - y, 255),
            DrawStyle::Brightness => from_hsv(x, 1.0 - y, hsb.brightness, 255),
            DrawStyle::Red => Color::rgb(c.r, b(1.0 - y), b(x)),
            DrawStyle::Green => Color::rgb(b(1.0 - y), c.g, b(x)),
            DrawStyle::Blue => Color::rgb(b(x), b(1.0 - y), c.b),
        }
    }

    /// `CreateBitmap`.
    fn bitmap(&self, w: usize, h: usize, slider: bool) -> egui::ColorImage {
        let hsb = Hsb::from_color(&self.color);
        let mut px = Vec::with_capacity(w * h);
        for y in 0..h {
            let ny = y as f64 / (h.max(2) - 1) as f64;
            for x in 0..w {
                let nx = x as f64 / (w.max(2) - 1) as f64;
                let c = self.pixel(nx, ny, slider, &hsb);
                px.push(Color32::from_rgb(c.r, c.g, c.b));
            }
        }
        egui::ColorImage { size: [w, h], pixels: px, source_size: vec2(w as f32, h as f32) }
    }

    /// `UpdateFromPointer`.
    fn update_from_pointer(&mut self, rect: Rect, p: egui::Pos2) -> Color {
        let (b, s) = Self::regions(rect);
        let x = (((p.x - b.min.x) / (b.width() - 1.0).max(1.0)) as f64).clamp(0.0, 1.0);
        let y = (((p.y - b.min.y) / (b.height() - 1.0).max(1.0)) as f64).clamp(0.0, 1.0);
        let sy = (((p.y - s.min.y) / (s.height() - 1.0).max(1.0)) as f64).clamp(0.0, 1.0);
        let a = self.color.a;
        let hsb = Hsb::from_color(&self.color);
        let c = &self.color;
        let slider = self.dragging_slider == Some(true);
        let color = match (self.style, slider) {
            (DrawStyle::Hue, true) => from_hsv(1.0 - sy, hsb.saturation, hsb.brightness, a),
            (DrawStyle::Hue, false) => from_hsv(hsb.hue, x, 1.0 - y, a),
            (DrawStyle::Saturation, true) => from_hsv(hsb.hue, 1.0 - sy, hsb.brightness, a),
            (DrawStyle::Saturation, false) => from_hsv(x, hsb.saturation, 1.0 - y, a),
            (DrawStyle::Brightness, true) => from_hsv(hsb.hue, hsb.saturation, 1.0 - sy, a),
            (DrawStyle::Brightness, false) => from_hsv(x, 1.0 - y, hsb.brightness, a),
            (DrawStyle::Red, true) => Color::argb(a, to_byte(1.0 - sy), c.g, c.b),
            (DrawStyle::Red, false) => Color::argb(a, c.r, to_byte(1.0 - y), to_byte(x)),
            (DrawStyle::Green, true) => Color::argb(a, c.r, to_byte(1.0 - sy), c.b),
            (DrawStyle::Green, false) => Color::argb(a, to_byte(1.0 - y), c.g, to_byte(x)),
            (DrawStyle::Blue, true) => Color::argb(a, c.r, c.g, to_byte(1.0 - sy)),
            (DrawStyle::Blue, false) => Color::argb(a, to_byte(x), to_byte(1.0 - y), c.b),
        };
        self.color = color.clone();
        color
    }

    /// Draws the control in `rect`; returns a colour the pointer chose.
    pub fn show(&mut self, ui: &Ui, rect: Rect, id: Id) -> Option<Color> {
        let (b, s) = Self::regions(rect);
        let key = {
            let c = &self.color;
            (u32::from_be_bytes([c.a, c.r, c.g, c.b]), self.style)
        };
        if self.textures.as_ref().is_none_or(|t| t.2 != key) {
            let bi = self.bitmap(b.width().round() as usize, b.height().round() as usize, false);
            let si = self.bitmap(s.width().round() as usize, s.height().round() as usize, true);
            match &mut self.textures {
                Some((bt, st, k)) => {
                    bt.set(bi, TextureOptions::NEAREST);
                    st.set(si, TextureOptions::NEAREST);
                    *k = key;
                }
                None => {
                    let bt = ui.ctx().load_texture("color picker box", bi, TextureOptions::NEAREST);
                    let st = ui.ctx().load_texture("color picker slider", si, TextureOptions::NEAREST);
                    self.textures = Some((bt, st, key));
                }
            }
        }
        let painter = ui.painter();
        if let Some((bt, st, _)) = &self.textures {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            painter.image(bt.id(), b, uv, Color32::WHITE);
            painter.image(st.id(), s, uv, Color32::WHITE);
        }
        // A 1px (95, 95, 95) pen on each region's edge.
        let grey = Color32::from_rgb(95, 95, 95);
        pen_rect(painter, b, 1.0, grey);
        pen_rect(painter, s, 1.0, grey);
        let hsb = Hsb::from_color(&self.color);
        let c = &self.color;
        let (px, py) = match self.style {
            DrawStyle::Hue => (hsb.saturation, 1.0 - hsb.brightness),
            DrawStyle::Saturation => (hsb.hue, 1.0 - hsb.brightness),
            DrawStyle::Brightness => (hsb.hue, 1.0 - hsb.saturation),
            DrawStyle::Red => (c.b as f64 / 255.0, 1.0 - c.g as f64 / 255.0),
            DrawStyle::Green => (c.b as f64 / 255.0, 1.0 - c.r as f64 / 255.0),
            DrawStyle::Blue => (c.r as f64 / 255.0, 1.0 - c.g as f64 / 255.0),
        };
        let point = pos2(b.min.x + px as f32 * b.width(), b.min.y + py as f32 * b.height());
        pen_ellipse(painter, point, 6.0, 3.0, Color32::BLACK);
        pen_ellipse(painter, point, 6.0, 1.5, Color32::WHITE);
        let value = match self.style {
            DrawStyle::Hue => hsb.hue,
            DrawStyle::Saturation => hsb.saturation,
            DrawStyle::Brightness => hsb.brightness,
            DrawStyle::Red => c.r as f64 / 255.0,
            DrawStyle::Green => c.g as f64 / 255.0,
            DrawStyle::Blue => c.b as f64 / 255.0,
        };
        let sy = s.min.y + (1.0 - value as f32) * s.height();
        let marker = Rect::from_min_size(pos2(s.min.x - 3.0, sy - 5.0), vec2(s.width() + 6.0, 10.0));
        pen_rect(painter, marker, 3.0, Color32::BLACK);
        pen_rect(painter, marker, 1.0, Color32::WHITE);
        let r = ui.interact(rect, id, Sense::click_and_drag());
        let mut out = None;
        if r.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down()) {
            if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                if self.dragging_slider.is_none() {
                    // `OnPointerPressed`: only inside the box or the slider.
                    self.dragging_slider = if s.contains(p) { Some(true) } else if b.contains(p) { Some(false) } else { None };
                }
                if self.dragging_slider.is_some() {
                    out = Some(self.update_from_pointer(rect, p));
                }
            }
        } else if self.dragging_slider.is_some() {
            self.dragging_slider = None;
        }
        out
    }
}

/// `ColorPickerWindow` in its tool mode (Tools › Color picker).
pub struct ColorPickerWindow {
    current: Color,
    previous: Color,
    has_previous: bool,
    picker: AdvancedPicker,
    /// `ColorPickerOptions.RecentColorsSelected`.
    recent_selected: bool,
    recent: Vec<Color>,
    /// Text being typed in the hex box.
    hex: String,
    copy_menu: Option<Flyout<CopyCommand>>,
    opened: bool,
    /// The window's height (`SizeToContent`).
    height: f32,
}

impl ColorPickerWindow {
    /// `new ColorPickerWindow(options, screenColorPickerOptions)`: red.
    pub fn new(recent_selected: bool, recent: Vec<Color>) -> Self {
        let red = Color::named("Red");
        let mut me = Self {
            current: red.clone(),
            previous: red.clone(),
            has_previous: false,
            picker: AdvancedPicker::new(red),
            recent_selected,
            recent,
            hex: String::new(),
            copy_menu: None,
            opened: false,
            height: 0.0,
        };
        me.set_color(me.current.clone(), true);
        me
    }

    /// `--render`: the clipboard isn't read.
    pub fn skip_clipboard(&mut self) {
        self.opened = true;
    }

    /// A palette swatch clicked (`OnPaletteColorClick`).
    pub fn choose(&mut self, c: Color) {
        self.change_to(c);
    }

    /// `SetColor`.
    fn set_color(&mut self, c: Color, update_picker: bool) {
        self.current = c.clone();
        if update_picker {
            self.picker.set_color(&c);
        }
        self.hex = format!("#{}", color::color_to_hex(&c, ColorFormat::Rgb));
    }

    /// A new colour with the current one kept as the previous.
    fn change_to(&mut self, c: Color) {
        self.previous = self.current.clone();
        self.has_previous = true;
        self.set_color(c, true);
    }

    fn palette(&self) -> Vec<Color> {
        let colors = if self.recent_selected { self.recent.clone() } else { color::standard_colors() };
        colors.into_iter().take(color::RECENT_COLORS_MAX).collect()
    }

    /// The palette's rows (a `WrapPanel` of 21px items, 344 wide).
    fn palette_rows(&self) -> usize {
        self.palette().len().div_ceil(16)
    }

    fn total_height(&self) -> f32 {
        let palette = 1.0 + 12.0 + 32.0 + 10.0 + self.palette_rows() as f32 * 21.0 + 12.0 + 1.0;
        tools_ui::MARGIN + TOP_HEIGHT + 12.0 + palette.max(40.0) + tools_ui::MARGIN
    }

    fn add_recent(&mut self) {
        color::add_recent_color(&mut self.recent, &self.current.clone());
    }

    /// `CopyAsync`.
    fn copy(&mut self, cmd: CopyCommand, cx: &mut Cx) -> Effect {
        let c = &self.current;
        let text = match cmd {
            CopyCommand::All => color::my_color_to_string(c),
            CopyCommand::Rgb => color::copy_rgb(c),
            CopyCommand::Hex => color::copy_hex(c),
            CopyCommand::Hsb => color::copy_hsb(c),
            CopyCommand::Cmyk => color::copy_cmyk(c),
            CopyCommand::Decimal => color::copy_decimal(c),
        };
        self.add_recent();
        self.save_recent(cx);
        Effect::CopyText(text)
    }

    fn save_recent(&self, cx: &mut Cx) {
        cx.settings.RecentColors = color::recent_colors_to_json(&self.recent);
    }

    /// The colour the screen colour picker came back with (`OnPickScreenClick`).
    pub fn screen_color(&mut self, c: Option<Color>, cx: &mut Cx) {
        if let Some(c) = c {
            self.change_to(c);
            self.add_recent();
            self.save_recent(cx);
        }
    }

    /// `ColumnDefinitions` of `n` stars with `ColumnSpacing="8"` across
    /// `x0..x0 + width`, edges rounded as layout rounding does.
    fn columns(x0: f32, width: f32, n: usize) -> Vec<(f32, f32)> {
        let w = (width - 8.0 * (n as f32 - 1.0)) / n as f32;
        (0..n)
            .map(|k| {
                let a = (x0 + k as f32 * (w + 8.0)).round_ties_even();
                let b = (x0 + k as f32 * (w + 8.0) + w).round_ties_even();
                (a, b - a)
            })
            .collect()
    }

    fn label(ui: &Ui, col: (f32, f32), y: f32, text: &str) {
        let f = font(13.0, 400);
        let tw = text_width(ui, text, f.clone());
        let x = col.0 + ((col.1 - tw) / 2.0).round_ties_even();
        tools_ui::text_at(ui, pos2(x, y), text, f, palette(ui.ctx()).text);
    }

    /// A centred number box without spinner buttons.
    fn number(ui: &mut Ui, rect: Rect, id: &str, value: i64, max: i64) -> Option<i64> {
        let mut v = value as f64;
        let d = DecimalUpDown { minimum: 0.0, maximum: max as f64, increment: 1.0, decimals: 0, enabled: true };
        d.show_with(ui, rect, Id::new(("color-picker", id)), &mut v, &|v| format!("{}", v.round() as i64), false)
            .then_some(v.round() as i64)
    }
}

impl Dialog for ColorPickerWindow {
    fn spec(&self) -> Spec {
        let mut spec = Spec::fit_height(crate::rs_title(s::ColorPickerWindow_Title()), WIDTH, MIN_WIDTH);
        spec.maximizable = false;
        spec
    }

    fn content_height(&mut self, _ui: &mut Ui) -> f32 {
        self.height = self.total_height();
        self.height
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        let mut effects = Vec::new();
        if !self.opened {
            // `OnOpened`: a colour on the clipboard becomes the colour.
            self.opened = true;
            if let Some(c) = sharex_capture::clipboard::get_text().and_then(|t| color::parse_color(&t)) {
                self.change_to(c);
            }
        }
        let p = palette(ui.ctx());
        let c = tools_ui::background(ui);
        // Left panel: the picker and the colour mode.
        let left = Rect::from_min_size(c.min, vec2(370.0, TOP_HEIGHT));
        tools_ui::panel(ui, left);
        let lc = left.shrink(13.0);
        let picker_rect = Rect::from_min_size(pos2(lc.min.x + ((lc.width() - 342.0) / 2.0).round_ties_even(), lc.min.y), vec2(342.0, 300.0));
        if let Some(chosen) = self.picker.show(ui, picker_rect, Id::new("color-picker-control")) {
            // `OnPickerColorChanged`: the picker keeps its own colour.
            self.set_color(chosen, false);
        }
        let mode_row = Rect::from_min_size(pos2(lc.min.x, picker_rect.max.y + 10.0), vec2(lc.width(), 40.0));
        let ml = s::ColorPickerWindow_Color_mode();
        let mf = font(13.0, 400);
        let mw = text_width(ui, ml, mf.clone());
        controls::label(ui, Rect::from_min_size(mode_row.min, vec2(mw, 40.0)), ml, mf, p.text);
        let items: Vec<ComboItem> = [
            s::ColorPickerWindow_Hue(),
            s::ColorPickerWindow_Saturation(),
            s::ColorPickerWindow_Brightness(),
            s::ColorPickerWindow_Red(),
            s::ColorPickerWindow_Green(),
            s::ColorPickerWindow_Blue(),
        ]
        .into_iter()
        .map(ComboItem::text)
        .collect();
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let selected = DrawStyle::ALL.iter().position(|d| *d == self.picker.style);
        let cw = combo.auto_width(ui, &items, selected);
        if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(mode_row.min.x + mw + 10.0, mode_row.min.y), vec2(cw, 40.0)), Id::new("color-picker-mode"), &items, selected) {
            self.picker.style = DrawStyle::ALL[i.min(5)];
        }

        // Right panel: the values.
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), pos2(c.max.x, c.min.y + TOP_HEIGHT));
        tools_ui::panel(ui, right);
        let rc = right.shrink(13.0);
        let cur = self.current.clone();
        let hsb = Hsb::from_color(&cur);
        let cmyk = Cmyk::from_color(&cur);
        let row_y = |k: usize| rc.min.y + k as f32 * ROW_PITCH;
        let lh = 18.0;
        let boxr = |col: (f32, f32), k: usize| Rect::from_min_size(pos2(col.0, row_y(k) + lh + 4.0), vec2(col.1, 40.0));
        let mut next: Option<Color> = None;
        // Row 0: name, hex, decimal.
        let cols = Self::columns(rc.min.x, rc.width(), 3);
        Self::label(ui, cols[0], row_y(0), s::ColorPickerWindow_Color_name());
        Self::label(ui, cols[1], row_y(0), s::ColorPickerWindow_Hex());
        Self::label(ui, cols[2], row_y(0), s::ColorPickerWindow_Decimal());
        let mut name = color::get_color_name(&cur);
        centered_text_box(ui, true, boxr(cols[0], 0), Id::new("color-picker-name"), &mut name);
        let mut hex = self.hex.clone();
        if centered_text_box(ui, false, boxr(cols[1], 0), Id::new("color-picker-hex"), &mut hex).changed() {
            // `OnHexTextChanged`: eight digits are RGBA.
            self.hex = hex.clone();
            let t = hex.trim();
            let parsed = if t.trim_start_matches('#').len() == 8 { color::hex_to_color(t, ColorFormat::Rgba) } else { color::hex_to_color(t, ColorFormat::Rgb) };
            if let Some(c) = parsed {
                self.current = c.clone();
                self.picker.set_color(&c);
            }
        }
        if let Some(v) = Self::number(ui, boxr(cols[2], 0), "decimal", color::color_to_decimal(&cur, ColorFormat::Rgb) as i64, 16_777_215) {
            next = Some(color::decimal_to_color(v as i32, ColorFormat::Rgb));
        }
        // Row 1: RGBA.
        let cols = Self::columns(rc.min.x, rc.width(), 4);
        let rgba = [cur.r as i64, cur.g as i64, cur.b as i64, cur.a as i64];
        let labels = [s::ColorPickerWindow_Red(), s::ColorPickerWindow_Green(), s::ColorPickerWindow_Blue(), s::ColorPickerWindow_Alpha()];
        let mut values = rgba;
        for k in 0..4 {
            Self::label(ui, cols[k], row_y(1), labels[k]);
            if let Some(v) = Self::number(ui, boxr(cols[k], 1), ["r", "g", "b", "a"][k], rgba[k], 255) {
                values[k] = v;
                next = Some(Color::argb(values[3] as u8, values[0] as u8, values[1] as u8, values[2] as u8));
            }
        }
        // Row 2: CMYK (`Grid.Row="2"`).
        let ck = [cmyk.cyan100(), cmyk.magenta100(), cmyk.yellow100(), cmyk.key100()].map(|v| v.round_ties_even() as i64);
        let labels = [s::ColorPickerWindow_Cyan_percent(), s::ColorPickerWindow_Magenta_percent(), s::ColorPickerWindow_Yellow_percent(), s::ColorPickerWindow_Key_percent()];
        let mut cv = ck;
        for k in 0..4 {
            Self::label(ui, cols[k], row_y(2), labels[k]);
            if let Some(v) = Self::number(ui, boxr(cols[k], 2), ["c", "m", "y", "k"][k], ck[k], 100) {
                cv[k] = v;
                next = Some(Cmyk::from_ints(cv[0] as i32, cv[1] as i32, cv[2] as i32, cv[3] as i32, cur.a as i32).to_color());
            }
        }
        // Row 3: HSB.
        let cols = Self::columns(rc.min.x, rc.width(), 3);
        let hv = [hsb.hue360(), hsb.saturation100(), hsb.brightness100()].map(|v| v.round_ties_even() as i64);
        let labels = [s::ColorPickerWindow_Hue_degrees(), s::ColorPickerWindow_Saturation_percent(), s::ColorPickerWindow_Brightness_percent()];
        let mut h = hv;
        for k in 0..3 {
            Self::label(ui, cols[k], row_y(3), labels[k]);
            if let Some(v) = Self::number(ui, boxr(cols[k], 3), ["h", "s", "v"][k], hv[k], if k == 0 { 360 } else { 100 }) {
                h[k] = v;
                next = Some(Hsb::from_ints(h[0] as i32, h[1] as i32, h[2] as i32, cur.a as i32).to_color());
            }
        }
        // Row 4: current, previous, Copy.
        Self::label(ui, cols[0], row_y(4), s::ColorPickerWindow_Current());
        Self::label(ui, cols[1], row_y(4), s::ColorPickerWindow_Previous());
        let checkered = Brush::Checkered { colors: p.checkered, cell: 4.0 };
        let current_box = boxr(cols[0], 4);
        border::border(ui.painter(), current_box, 4.0, 1.0, Some(checkered), Some(p.border));
        let fill = |c: &Color| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
        border::border(ui.painter(), current_box.shrink(1.0), 3.0, 0.0, Some(fill(&cur).into()), None);
        let prev_box = boxr(cols[1], 4);
        border::border(ui.painter(), prev_box, 4.0, 1.0, Some(checkered), Some(p.border));
        if self.has_previous {
            border::border(ui.painter(), prev_box.shrink(1.0), 3.0, 0.0, Some(fill(&self.previous).into()), None);
            // `OnPreviousColorPressed`: the two swap.
            if ui.interact(prev_box, Id::new("color-picker-previous"), Sense::click()).is_pointer_button_down_on()
                && ui.input(|i| i.pointer.primary_pressed())
            {
                let old = self.current.clone();
                self.set_color(self.previous.clone(), true);
                self.previous = old;
            }
        }
        let copy_rect = boxr(cols[2], 4);
        if copy_button(ui, copy_rect, s::ColorPickerWindow_Copy()) {
            let entries = vec![
                MenuEntry::item(s::ColorPickerWindow_Copy_all_details(), "", CopyCommand::All),
                MenuEntry::item(s::ColorPickerWindow_Copy_RGB(), "", CopyCommand::Rgb),
                MenuEntry::item(s::ColorPickerWindow_Copy_hexadecimal(), "", CopyCommand::Hex),
                MenuEntry::item(s::ColorPickerWindow_Copy_HSB(), "", CopyCommand::Hsb),
                MenuEntry::item(s::ColorPickerWindow_Copy_CMYK(), "", CopyCommand::Cmyk),
                MenuEntry::item(s::ColorPickerWindow_Copy_decimal(), "", CopyCommand::Decimal),
            ];
            let mut f = Flyout::menu(Id::new("color-picker-copy-menu"), entries, copy_rect, Placement::BottomEdgeAlignedLeft);
            f.style = MenuStyle::Standard;
            self.copy_menu = Some(f);
        }

        // The palette panel.
        let rows = self.palette_rows() as f32;
        let ph = 1.0 + 12.0 + 32.0 + 10.0 + rows * 21.0 + 12.0 + 1.0;
        let bottom_top = c.min.y + TOP_HEIGHT + 12.0;
        let pal = Rect::from_min_size(pos2(c.min.x, bottom_top), vec2(370.0, ph));
        tools_ui::panel(ui, pal);
        let pc = pal.shrink(13.0);
        let rf = controls::content_font(14.0, 400);
        let mut x = pc.min.x;
        for (k, text) in [s::ColorPickerWindow_Recent_colors(), s::ColorPickerWindow_Standard_colors()].into_iter().enumerate() {
            let checked = (k == 0) == self.recent_selected;
            let w = 20.0 + 8.0 + text_width(ui, text, rf.clone());
            let rb = RadioButton { text, text_font: rf.clone(), text_color: Some(p.text), padding_left: 8.0, enabled: true };
            if rb.show(ui, Rect::from_min_size(pos2(x, pc.min.y), vec2(w, 32.0)), Id::new(("color-picker-palette", k)), checked).clicked() && !checked {
                // `OnPaletteChanged`.
                self.recent_selected = k == 0;
                set_recent_selected(cx, self.recent_selected);
            }
            x += w + 8.0;
        }
        let swatches = self.palette();
        let mut chosen = None;
        for (i, sw) in swatches.iter().enumerate() {
            let (col, row) = (i % 16, i / 16);
            let b = Rect::from_min_size(pos2(pc.min.x + col as f32 * 21.0, pc.min.y + 32.0 + 10.0 + row as f32 * 21.0), vec2(21.0, 21.0));
            let r = ui.interact(b, Id::new(("color-picker-swatch", i)), Sense::click());
            if r.hovered() {
                border::border(ui.painter(), b, 4.0, 0.0, Some(p.control_background_hover.into()), None);
            }
            border::border(ui.painter(), b.shrink(1.0), 3.0, 0.0, Some(fill(sw).into()), None);
            if r.clicked() {
                chosen = Some(sw.clone());
            }
        }
        if let Some(sw) = chosen {
            self.change_to(sw);
        }
        // Pick from screen and from the clipboard, at the bottom right.
        let by = bottom_top + ph.max(40.0) - 40.0;
        let clip = Rect::from_min_size(pos2(c.max.x - 38.0, by), vec2(38.0, 40.0));
        let screen = Rect::from_min_size(pos2(clip.min.x - 8.0 - 38.0, by), vec2(38.0, 40.0));
        if tools_ui::icon_button_sized(ui, screen, Id::new("color-picker-screen"), icon::ScreenColorPicker, 14.0, true)
            .on_hover_text(s::ColorPickerWindow_Pick_from_screen())
            .clicked()
        {
            effects.push(Effect::PickScreenColor);
        }
        if tools_ui::icon_button_sized(ui, clip, Id::new("color-picker-clipboard"), icon::ClipboardPaste, 14.0, true)
            .on_hover_text(s::ColorPickerWindow_Pick_from_clipboard())
            .clicked()
            && let Some(c) = sharex_capture::clipboard::get_text().and_then(|t| color::parse_color(&t))
        {
            self.change_to(c);
        }
        if let Some(n) = next {
            self.set_color(n, true);
        }
        if let Some(menu) = &mut self.copy_menu {
            let out = menu.show(ui, ui.max_rect());
            if let Some(cmd) = out.command {
                effects.push(self.copy(cmd, cx));
            }
            if out.closed {
                self.copy_menu = None;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && self.copy_menu.is_none() {
            effects.push(Effect::Close);
        }
        let h = self.total_height();
        if (h - self.height).abs() > 0.5 {
            self.height = h;
            effects.push(Effect::SetHeight(h));
        }
        effects
    }

    /// `Closed`: the colour goes into the recent colours.
    fn closed(&mut self, cx: &mut Cx) -> Vec<Effect> {
        self.add_recent();
        self.save_recent(cx);
        Vec::new()
    }

    fn screen_color(&mut self, c: Option<Color>, cx: &mut Cx) {
        ColorPickerWindow::screen_color(self, c, cx);
    }
}

/// `DrawRectangle(null, pen, rect)`: a `width` pen centred on the edge,
/// with Skia's area coverage.
fn pen_rect(painter: &egui::Painter, rect: Rect, width: f32, color: Color32) {
    border::border(painter, rect.expand(width / 2.0), 0.0, width, None, Some(color));
}

/// `DrawEllipse(null, pen, center, r, r)`.
fn pen_ellipse(painter: &egui::Painter, center: egui::Pos2, r: f32, width: f32, color: Color32) {
    let outer = r + width / 2.0;
    border::border(painter, Rect::from_center_size(center, vec2(outer * 2.0, outer * 2.0)), outer, width, None, Some(color));
}

/// `TextAlignment="Center"` in a text box: the shared box with its text
/// centred.
fn centered_text_box(ui: &mut Ui, read_only: bool, rect: Rect, id: Id, text: &mut String) -> egui::Response {
    let f = TextBox::font();
    let w = text_width(ui, text, f.clone());
    let pad = ((rect.width() - 2.0 - w) / 2.0).max(8.0).floor();
    let tb = TextBox { padding: Thickness::new(pad, 6.0, 8.0, 6.0), read_only, ..Default::default() };
    tb.show(ui, rect, id, text)
}

/// The Copy button: a copy icon, "Copy" and an 11px chevron (`Spacing="8"`).
fn copy_button(ui: &Ui, rect: Rect, text: &str) -> bool {
    let p = palette(ui.ctx());
    let r = controls::Button::default().show(ui, rect, Id::new("color-picker-copy"), "", true);
    let f = font(13.0, 400);
    let tw = text_width(ui, text, f.clone());
    let w = 13.0 + 8.0 + tw + 8.0 + 11.0;
    let x = rect.min.x + ((rect.width() - w) / 2.0).round_ties_even();
    let lh = line_height(ui, &f);
    let ty = center_y(rect, lh);
    sharex_ui::icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(rect, 13.0)), vec2(13.0, 13.0)), icon::Copy, p.text);
    tools_ui::text_at(ui, pos2(x + 21.0, ty), text, f, p.text);
    sharex_ui::icons::paint(ui.painter(), Rect::from_min_size(pos2(x + 21.0 + tw + 8.0, center_y(rect, 11.0)), vec2(11.0, 11.0)), icon::ChevronDown, p.text);
    r.clicked()
}

/// `ColorPickerOptions.RecentColorsSelected` on the default task settings'
/// tools settings (`ToolsSettingsReference`).
fn set_recent_selected(cx: &mut Cx, v: bool) {
    let t = cx.settings.DefaultTaskSettings.ToolsSettings.get_or_insert_default();
    sharex_tools::pin::set_json(&mut t.ColorPickerOptions, "RecentColorsSelected", serde_json::json!(v));
}

/// `ColorPickerOptions.RecentColorsSelected` of the default task settings.
pub fn recent_selected(settings: &sharex_settings::ApplicationConfig) -> bool {
    settings
        .DefaultTaskSettings
        .ToolsSettings
        .as_ref()
        .and_then(|t| t.ColorPickerOptions.get("RecentColorsSelected"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_of_the_hue_slider() {
        assert_eq!(from_hsv(0.0, 1.0, 1.0, 255), Color::rgb(255, 0, 0));
        assert_eq!(from_hsv(1.0 / 3.0, 1.0, 1.0, 255), Color::rgb(0, 255, 0));
        assert_eq!(to_byte(0.5), 128);
    }
}
