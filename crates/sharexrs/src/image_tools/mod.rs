//! ShareX.Tools' Images windows (docs/TOOLS.md, "Window plan"): the image
//! combiner, splitter, thumbnailer, resizer, converter, watermark, comparer
//! and icon converter, each a [`Dialog`](crate::dialogs::Dialog) over its
//! engine in `sharex-tools`. This module holds what they share, ported from
//! `ShareX.Avalonia/Theming/ToolStyles.axaml` and the windows' common
//! `.axaml` blocks: the `tool-panel` borders and their text classes, the
//! toolbar buttons, the image file list (`ListBox` with `SelectionMode=
//! "Multiple"`), the preview pane (toolbar header, `ProgressBar`,
//! `ImageComparisonSlider`), and previews computed off the UI thread with
//! stale ones dropped (`_previewVersion`).

pub mod background_remover;
pub mod combiner;
pub mod comparer;
pub mod converter;
pub mod icon_converter;
pub mod open;
pub mod render;
pub mod resizer;
pub mod slider;
pub mod splitter;
pub mod thumbnailer;
pub mod watermark;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use egui::{Color32, FontId, Id, Rect, Response, Ui, Vec2, pos2, vec2};
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{self, Button};
use sharex_ui::icons;
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

/// The windows' `Margin="16"`.
pub const MARGIN: f32 = 16.0;
/// `Button`'s `Height="40"` (ControlStyles.axaml).
pub const BUTTON_HEIGHT: f32 = 40.0;
/// A `NumericUpDown`'s / `TextBox`'s / `ComboBox`'s height in these windows
/// (`MinHeight="40"`).
pub const CONTROL_HEIGHT: f32 = 40.0;

/// The image file types the pickers offer (`FilePickerFileTypes.ImageAll`).
pub const IMAGE_EXTENSIONS: [&str; 9] = ["png", "jpg", "jpeg", "gif", "bmp", "webp", "tif", "tiff", "ico"];

/// The window's background (`ShareX.Brush.Background.Main`); returns the
/// content box inside `Margin="16"`.
pub fn background(ui: &Ui) -> Rect {
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, palette(ui.ctx()).background_main);
    full.shrink(MARGIN)
}

/// `Border.tool-panel`: the panel background, a 1px border, 4px corners.
pub fn panel(ui: &Ui, rect: Rect) {
    let p = palette(ui.ctx());
    border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
}

/// `TextBlock.section-heading`: 15px, ShareX's SemiBold, the text colour.
pub fn heading_font() -> FontId {
    font(15.0, 600)
}

/// A `TextBlock` with `FontWeight="Normal"`.
pub fn normal_font() -> FontId {
    font(13.0, 400)
}

/// A plain `TextBlock` (ShareX's global SemiBold 13px).
pub fn semibold_font() -> FontId {
    font(13.0, 600)
}

/// A section heading's line height.
pub fn heading_height(ui: &Ui) -> f32 {
    line_height(ui, &heading_font())
}

/// Draws `text` at `pos` (the top of its line box) in `font` and `color`.
pub fn text_at(ui: &Ui, pos: egui::Pos2, text: &str, font: FontId, color: Color32) {
    ui.painter().galley(pos, galley(ui, text, font, color, f32::INFINITY), color);
}

/// `TextBlock.section-heading` centred vertically in `rect`.
pub fn heading(ui: &Ui, rect: Rect, text: &str) {
    let f = heading_font();
    let y = center_y(rect, line_height(ui, &f));
    text_at(ui, pos2(rect.min.x, y), text, f, palette(ui.ctx()).text);
}

/// `TextBlock.row-label` (`VerticalAlignment="Center"`) in `rect`.
pub fn row_label(ui: &Ui, rect: Rect, text: &str) {
    controls::label(ui, rect, text, normal_font(), palette(ui.ctx()).text_secondary);
}

/// `text` cut to `width` with `TextTrimming="CharacterEllipsis"`.
pub fn ellipsize(ui: &Ui, text: &str, font: &FontId, width: f32) -> String {
    if text_width(ui, text, font.clone()) <= width {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut n = chars.len();
    while n > 0 {
        n -= 1;
        let s: String = chars[..n].iter().collect::<String>() + "…";
        if text_width(ui, &s, font.clone()) <= width {
            return s;
        }
    }
    String::new()
}

/// `TextBlock.path-text`: Normal, secondary, trimmed with an ellipsis,
/// centred vertically in `rect`.
pub fn path_text(ui: &Ui, rect: Rect, text: &str) {
    let f = normal_font();
    let s = ellipsize(ui, text, &f, rect.width());
    controls::label(ui, rect, &s, f, palette(ui.ctx()).text_secondary);
}

/// A `Button` with string `Content` ("Browse..."): its width.
pub fn text_button_width(ui: &Ui, text: &str) -> f32 {
    Button::default().width(ui, text)
}

/// That button in `rect`.
pub fn text_button(ui: &Ui, rect: Rect, id: Id, text: &str, enabled: bool) -> Response {
    Button::default().show(ui, rect, id, text, enabled)
}

/// A `Button` with an `IconPresenter` and a `TextBlock` (`StackPanel
/// Spacing="8"`): its width.
pub fn icon_text_button_width(ui: &Ui, text: &str) -> f32 {
    crate::history_ui::common::icon_button_width(ui, text)
}

/// That button in `rect`.
pub fn icon_text_button(ui: &Ui, rect: Rect, id: Id, key: &str, text: &str, enabled: bool) -> Response {
    crate::history_ui::common::icon_button(ui, rect, id, key, text, false, enabled)
}

/// A `Button` with only an `IconPresenter` (the button's 13px font):
/// `Padding="10,6"`, so 35 wide.
pub const ICON_BUTTON_WIDTH: f32 = 1.0 + 10.0 + 13.0 + 10.0 + 1.0;

/// That button in `rect`, the icon in the text colour.
pub fn icon_button(ui: &Ui, rect: Rect, id: Id, key: &str, enabled: bool) -> Response {
    icon_button_sized(ui, rect, id, key, 13.0, enabled)
}

/// An icon-only button whose `IconPresenter` is `size` (`Button.picker-button`'s 16).
pub fn icon_button_sized(ui: &Ui, rect: Rect, id: Id, key: &str, size: f32, enabled: bool) -> Response {
    let p = palette(ui.ctx());
    let r = Button::compact(Thickness::hv(10.0, 6.0)).show(ui, rect, id, "", enabled);
    let opacity = if enabled { 1.0 } else { 0.35 };
    let inner = rect.shrink(1.0);
    let b = Rect::from_min_size(pos2(inner.min.x + ((inner.width() - size) / 2.0).round_ties_even(), center_y(inner, size)), Vec2::splat(size));
    let color = if enabled { p.text } else { border::group_content(p.text, p.control_background, opacity, p.background_panel) };
    icons::paint(ui.painter(), b, key, color);
    r
}

/// The centred empty-state `StackPanel` (`Spacing`): an icon of `icon_size`
/// over `lines` (each a `TextBlock` with its font and colour).
pub fn empty_state(ui: &Ui, area: Rect, key: &str, icon_size: f32, spacing: f32, lines: &[(&str, FontId, Color32)]) {
    let p = palette(ui.ctx());
    let heights: Vec<f32> = lines.iter().map(|(_, f, _)| line_height(ui, f)).collect();
    let total = icon_size + lines.len() as f32 * spacing + heights.iter().sum::<f32>();
    let widest = lines.iter().map(|(t, f, _)| text_width(ui, t, f.clone())).fold(icon_size, f32::max);
    let x0 = area.min.x + ((area.width() - widest) / 2.0).round_ties_even();
    let mut y = area.min.y + ((area.height() - total) / 2.0).round_ties_even();
    let icon_rect = Rect::from_min_size(pos2(x0 + ((widest - icon_size) / 2.0).round_ties_even(), y), Vec2::splat(icon_size));
    icons::paint(ui.painter(), icon_rect, key, p.text_secondary);
    y += icon_size + spacing;
    for (k, (text, f, color)) in lines.iter().enumerate() {
        let w = text_width(ui, text, f.clone());
        text_at(ui, pos2(x0 + ((widest - w) / 2.0).round_ties_even(), y), text, f.clone(), *color);
        y += heights[k] + spacing;
    }
}

/// `Path.GetDirectoryName`.
pub fn directory_name(path: &str) -> String {
    Path::new(path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The multi-select image picker (`OpenFilePickerAsync`, `AllowMultiple`).
pub fn pick_images(title: &str) -> Vec<String> {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("Images", &IMAGE_EXTENSIONS)
        .pick_files()
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// The single image picker.
pub fn pick_image(title: &str) -> Option<String> {
    rfd::FileDialog::new().set_title(title).add_filter("Images", &IMAGE_EXTENSIONS).pick_file().map(|p| p.to_string_lossy().into_owned())
}

/// `SkiaImageHelpers.OpenImageFileDialog`: the system's Open dialog with
/// ShareX's image filter ("Image files").
pub fn open_image_file_dialog() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter(sharex_strings::helpers_lib::ImageHelpers_Image_files(), &["png", "jpg", "jpeg", "jpe", "jfif", "gif", "bmp"])
        .pick_file()
        .map(|p| p.to_string_lossy().into_owned())
}

/// `SkiaImageHelpers.SaveImage(img, filePath)`: encoded by the extension
/// (PNG unless it's JPEG, WebP, GIF or BMP) at quality 100, the folder
/// created.
pub fn save_image(img: &image::RgbaImage, path: &Path) -> std::io::Result<()> {
    let bmp = sharex_tools::imaging::Bitmap::from_straight(img.width(), img.height(), img.as_raw());
    sharex_tools::imaging::save_image(&bmp, &path.to_string_lossy())
}

/// `SkiaImageHelpers.SaveImageFilter`'s types, in its order.
pub const SAVE_IMAGE_FILTERS: [(&str, &[&str]); 5] = [
    ("PNG (*.png)", &["png"]),
    ("JPEG (*.jpg, *.jpeg, *.jpe, *.jfif)", &["jpg", "jpeg", "jpe", "jfif"]),
    ("WebP (*.webp)", &["webp"]),
    ("GIF (*.gif)", &["gif"]),
    ("BMP (*.bmp)", &["bmp"]),
];

/// `SkiaImageHelpers.SaveImageFileDialog(img, filePath)`: the save dialog
/// in the file's folder with its name and ShareX's five image types (`png`
/// by default), then [`save_image`]; the path saved, or `None`. (The
/// dialog can't preselect the file's type as ShareX's `filterIndex` does.)
pub fn save_image_file_dialog(img: &image::RgbaImage, path: &Path) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new();
    for (name, exts) in SAVE_IMAGE_FILTERS {
        d = d.add_filter(name, exts);
    }
    if let Some(dir) = path.parent().filter(|d| d.is_dir()) {
        d = d.set_directory(dir);
    }
    if let Some(name) = path.file_name() {
        d = d.set_file_name(name.to_string_lossy());
    }
    let out = d.save_file()?;
    // `DefaultExtension = "png"`.
    let out = if out.extension().is_none() { out.with_extension("png") } else { out };
    match save_image(img, &out) {
        Ok(()) => Some(out),
        Err(e) => {
            eprintln!("couldn't save {}: {e}", out.display());
            None
        }
    }
}

/// The folder picker, starting in `start` when it exists.
pub fn pick_folder(title: &str, start: &str) -> Option<String> {
    let mut d = rfd::FileDialog::new().set_title(title);
    if !start.is_empty() && Path::new(start).is_dir() {
        d = d.set_directory(start);
    }
    d.pick_folder().map(|p| p.to_string_lossy().into_owned())
}

/// `File.Exists`.
pub fn file_exists(path: &str) -> bool {
    !path.is_empty() && Path::new(path).is_file()
}

/// `Directory.Exists`.
pub fn dir_exists(path: &str) -> bool {
    !path.is_empty() && Path::new(path).is_dir()
}

/// `string.Format(count == 1 ? one : many, count)`.
pub fn count_text(one: &str, many: &str, count: usize) -> String {
    (if count == 1 { one } else { many }).replace("{0}", &count.to_string())
}

/// The `ListBox` of image files (`SelectionMode="Multiple"`): rows with the
/// `AppIcons.Image` icon and the path, Ctrl/Shift extend the selection.
/// `SelectedItem` is the first item selected (the anchor kept while others
/// are added), as Avalonia's `SelectionModel.SelectedIndex`.
#[derive(Default)]
pub struct FileList {
    pub items: Vec<String>,
    /// Selected indexes.
    pub selected: Vec<usize>,
    /// `SelectedIndex`.
    pub primary: Option<usize>,
    /// The anchor for Shift ranges.
    anchor: Option<usize>,
    scroll: f32,
    /// The item `TextBlock` keeps ShareX's SemiBold (the resizer, converter
    /// and watermark windows don't set `FontWeight="Normal"`).
    pub semibold: bool,
}

impl FileList {
    pub fn semibold() -> Self {
        Self { semibold: true, ..Self::default() }
    }

    /// `SelectedItem`.
    pub fn selected_item(&self) -> Option<&String> {
        self.primary.and_then(|i| self.items.get(i))
    }

    /// Selects only `index` (`SelectedImage = …`), or nothing.
    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index.into_iter().collect();
        self.primary = index;
        self.anchor = index;
    }

    /// `SelectedItems` (in list order).
    pub fn selected_items(&self) -> Vec<String> {
        let mut s = self.selected.clone();
        s.sort_unstable();
        s.into_iter().filter_map(|i| self.items.get(i).cloned()).collect()
    }

    /// `Remove`: the selected files (`_selectedImages`, else `SelectedImage`)
    /// out of the list, as `Images.Remove(file)` (the first equal entry).
    pub fn remove_selected(&mut self) {
        let files = if self.selected.is_empty() { self.selected_item().cloned().into_iter().collect() } else { self.selected_items() };
        for f in files {
            if let Some(i) = self.items.iter().position(|x| *x == f) {
                self.items.remove(i);
            }
        }
        self.select(None);
    }

    /// `Images.Move(index, index + delta)` for the selected item, which stays selected.
    pub fn move_selected(&mut self, delta: isize) -> bool {
        let Some(i) = self.primary else { return false };
        let j = i as isize + delta;
        if j < 0 || j as usize >= self.items.len() {
            return false;
        }
        let j = j as usize;
        self.items.swap(i, j);
        let map = |k: usize| if k == i { j } else if k == j { i } else { k };
        self.selected = self.selected.iter().map(|&k| map(k)).collect();
        self.primary = Some(j);
        self.anchor = self.anchor.map(map);
        true
    }

    /// A row's height: `ListBoxItem` `Padding="12,9,12,12"` around the
    /// template's `Margin="4,3"` and its line.
    pub fn row_height(ui: &Ui) -> f32 {
        9.0 + 3.0 + line_height(ui, &normal_font()).max(13.0) + 3.0 + 12.0
    }

    /// Draws the list in `rect`; true when the selection changed.
    pub fn ui(&mut self, ui: &Ui, rect: Rect, id: Id, live: bool) -> bool {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        // `SystemControlBackgroundChromeMediumLowBrush`, no border.
        ui.painter().rect_filled(rect, 0.0, f.chrome_medium_low);
        let row_h = Self::row_height(ui);
        let content_h = row_h * self.items.len() as f32;
        let max_scroll = (content_h - rect.height()).max(0.0);
        if live && ui.rect_contains_pointer(rect) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        let text_font = if self.semibold { semibold_font() } else { normal_font() };
        let lh = line_height(ui, &text_font);
        let mut changed = false;
        let mut clicked: Option<(usize, egui::Modifiers)> = None;
        for (i, item) in self.items.iter().enumerate() {
            let y = rect.min.y + i as f32 * row_h - self.scroll;
            if y + row_h < rect.min.y || y > rect.max.y {
                continue;
            }
            let row = Rect::from_min_size(pos2(rect.min.x, y), vec2(rect.width(), row_h));
            let selected = self.selected.contains(&i);
            let (hovered, pressed) = if live {
                let r = ui.interact(row.intersect(rect), id.with(("row", i)), egui::Sense::click());
                if r.clicked() {
                    clicked = Some((i, ui.input(|inp| inp.modifiers)));
                }
                (r.hovered(), r.is_pointer_button_down_on())
            } else {
                (false, false)
            };
            controls::list_item_background(&painter, row, 0.0, hovered, pressed, selected);
            let fg = if selected { p.accent_foreground } else { p.text_secondary };
            // The template: `Grid ColumnDefinitions="Auto,*" ColumnSpacing="10"
            // Margin="4,3"` inside the item's padding.
            let content = Rect::from_min_max(pos2(row.min.x + 12.0 + 4.0, row.min.y + 9.0 + 3.0), pos2(row.max.x - 12.0 - 4.0, row.max.y - 12.0 - 3.0));
            // The `IconPresenter` takes the item's 14px font size.
            let icon = 14.0;
            let ir = Rect::from_min_size(pos2(content.min.x, center_y(content, icon)), Vec2::splat(icon));
            icons::paint(&painter, ir, sharex_ui::keys::app_icons::Image, fg);
            let tx = ir.max.x + 10.0;
            let text = ellipsize(ui, item, &text_font, content.max.x - tx);
            painter.galley(pos2(tx, center_y(content, lh)), galley(ui, &text, text_font.clone(), fg, f32::INFINITY), fg);
        }
        if max_scroll > 0.0 {
            crate::dialogs::layout::scroll_bar(ui, rect, content_h, self.scroll, 0.0);
        }
        if let Some((i, m)) = clicked {
            changed = true;
            if m.shift && let Some(a) = self.anchor {
                let (lo, hi) = (a.min(i), a.max(i));
                self.selected = (lo..=hi).collect();
                self.primary = Some(lo.min(self.primary.unwrap_or(lo)));
                self.primary = Some(a.min(i));
            } else if m.command {
                if let Some(k) = self.selected.iter().position(|&s| s == i) {
                    self.selected.remove(k);
                    if self.primary == Some(i) {
                        self.primary = self.selected.iter().copied().min();
                    }
                } else {
                    self.selected.push(i);
                    if self.primary.is_none() {
                        self.primary = Some(i);
                    }
                }
                self.anchor = Some(i);
            } else {
                self.select(Some(i));
            }
        }
        changed
    }
}

/// Work done off the UI thread for the newest request only (the view
/// models' `_previewVersion` and `Task.Delay(100)` debounce): `queue`
/// bumps the version and arms the delay; `poll` starts the work when it's
/// due and hands back a result only if no newer request came since.
pub struct Job<T> {
    version: u64,
    due: Option<Instant>,
    delay: Duration,
    rx: Option<mpsc::Receiver<(u64, T)>>,
    /// `IsPreviewLoading`.
    pub busy: bool,
    /// Since when it's been busy (the progress bar's animation).
    pub busy_since: Instant,
    /// Runs the work at once on this thread (`--render`).
    pub sync: bool,
}

impl<T: Send + 'static> Job<T> {
    pub fn new(delay_ms: u64) -> Self {
        Self { version: 0, due: None, delay: Duration::from_millis(delay_ms), rx: None, busy: false, busy_since: Instant::now(), sync: false }
    }

    /// A new request: the older ones' results will be dropped.
    pub fn queue(&mut self) {
        self.version += 1;
        self.due = Some(Instant::now() + self.delay);
        if !self.busy {
            self.busy_since = Instant::now();
        }
        self.busy = true;
    }

    /// Makes a queued request due at once (`InitializeAsync`).
    pub fn due_now(&mut self) {
        if self.due.is_some() {
            self.due = Some(Instant::now());
        }
    }

    /// No work wanted (and none pending).
    pub fn cancel(&mut self) {
        self.version += 1;
        self.due = None;
        self.busy = false;
    }

    /// Starts the work when it's due (`make` snapshots the inputs and gives
    /// the work), and returns the current request's result when it's in.
    pub fn poll(&mut self, ctx: &egui::Context, make: impl FnOnce() -> Box<dyn FnOnce() -> T + Send>) -> Option<T> {
        if let Some(due) = self.due {
            let now = Instant::now();
            if self.sync || now >= due {
                self.due = None;
                let work = make();
                let version = self.version;
                if self.sync {
                    self.busy = false;
                    return Some(work());
                }
                let (tx, rx) = mpsc::channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let out = work();
                    let _ = tx.send((version, out));
                    ctx.request_repaint();
                });
                self.rx = Some(rx);
            } else {
                ctx.request_repaint_after(due - now);
            }
        }
        if let Some(rx) = &self.rx
            && let Ok((version, out)) = rx.try_recv()
            && version == self.version
        {
            self.rx = None;
            self.busy = false;
            return Some(out);
        }
        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        None
    }
}

/// An image shown as a texture, made again only when its pixels change.
pub struct Picture {
    pub image: image::RgbaImage,
    texture: Option<((u32, u32), egui::TextureHandle)>,
    serial: u64,
}

impl Picture {
    pub fn new(image: image::RgbaImage) -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self { image, texture: None, serial: SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }
    }

    /// Decodes PNG/JPEG/BMP… bytes (`new Bitmap(stream)`).
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        image::load_from_memory(bytes).ok().map(|i| Self::new(i.to_rgba8()))
    }

    pub fn size(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    /// The image resampled to `size` (whole pixels) as a texture: shrinking
    /// goes through `thumbnail::resize` (Skia's linear + mipmaps, as
    /// `ImageComparisonSlider`'s render cache), enlarging keeps the
    /// bilinear filter of the draw.
    pub fn texture(&mut self, ctx: &egui::Context, size: (u32, u32)) -> egui::TextureId {
        let (w, h) = self.image.dimensions();
        let shrink = size.0 < w || size.1 < h;
        let want = if shrink { size } else { (w, h) };
        if self.texture.as_ref().is_none_or(|(s, _)| *s != want) {
            let img = if shrink { crate::thumbnail::resize(&self.image, want.0.max(1), want.1.max(1)) } else { self.image.clone() };
            let options = if shrink { egui::TextureOptions::NEAREST } else { egui::TextureOptions::LINEAR };
            let tex = ctx.load_texture(format!("tool-picture-{}", self.serial), crate::thumbnail::to_color_image(&img), options);
            self.texture = Some((want, tex));
        }
        self.texture.as_ref().map(|(_, t)| t.id()).unwrap_or_default()
    }
}

/// The preview pane of the resizer, converter and watermark windows: a
/// `tool-panel` with `Padding="0"`, a toolbar-coloured header (`Padding="12,10"`,
/// a 1px bottom border) with the file path and the size text, the 4px
/// `ProgressBar` while loading, and the `ImageComparisonSlider` (only a
/// `RightImage`, `StretchDirection="DownOnly"`) inside `Margin="12"`; the
/// 54px image icon when there are no images.
pub fn preview_pane(ui: &Ui, rect: Rect, path: &str, size_text: &str, preview: Option<&mut Picture>, loading: Option<f64>, has_images: bool) {
    let p = palette(ui.ctx());
    panel(ui, rect);
    let inner = rect.shrink(1.0);
    let nf = normal_font();
    let sf = semibold_font();
    let lh = line_height(ui, &nf).max(line_height(ui, &sf));
    let header = Rect::from_min_size(inner.min, vec2(inner.width(), 10.0 + lh + 10.0 + 1.0));
    let radius = egui::CornerRadius { nw: 3, ne: 3, ..Default::default() };
    ui.painter().rect_filled(header, radius, p.background_toolbar);
    ui.painter().rect_filled(Rect::from_min_size(pos2(header.min.x, header.max.y - 1.0), vec2(header.width(), 1.0)), 0.0, p.border);
    let row = Rect::from_min_max(pos2(header.min.x + 12.0, header.min.y + 10.0), pos2(header.max.x - 12.0, header.max.y - 1.0 - 10.0));
    let size_w = if size_text.is_empty() { 0.0 } else { text_width(ui, size_text, sf.clone()) };
    let spacing = if size_text.is_empty() { 0.0 } else { 10.0 };
    path_text(ui, Rect::from_min_max(row.min, pos2(row.max.x - size_w - spacing, row.max.y)), path);
    if !size_text.is_empty() {
        text_at(ui, pos2(row.max.x - size_w, center_y(row, line_height(ui, &sf))), size_text, sf, p.text_secondary);
    }
    let body = Rect::from_min_max(pos2(inner.min.x, header.max.y), inner.max);
    if let Some(pic) = preview {
        slider::ComparisonSlider::draw_single(ui, body.shrink(12.0), pic, slider::Stretch::DownOnly);
    } else if !has_images {
        let s = 54.0;
        let ir = Rect::from_min_size(pos2(body.min.x + ((body.width() - s) / 2.0).round_ties_even(), body.min.y + ((body.height() - s) / 2.0).round_ties_even()), Vec2::splat(s));
        icons::paint(ui.painter(), ir, sharex_ui::keys::app_icons::Image, p.text_secondary);
    }
    if let Some(t) = loading {
        let bar = Rect::from_min_size(body.min, vec2(body.width(), 4.0));
        controls::indeterminate_progress(ui.painter(), bar, t);
        ui.ctx().request_repaint();
    }
}

/// `Border.tool-panel` with the message (`TextWrapping="Wrap"`, secondary):
/// its height for `width`, drawn when `paint`.
pub fn message_panel(ui: &Ui, pos: egui::Pos2, width: f32, message: &str, paint: bool) -> f32 {
    let f = semibold_font();
    let h = controls::wrapped(ui, pos, width - 26.0, message, f.clone(), palette(ui.ctx()).text_secondary, false) + 26.0;
    if paint {
        panel(ui, Rect::from_min_size(pos, vec2(width, h)));
        controls::wrapped(ui, pos2(pos.x + 13.0, pos.y + 13.0), width - 26.0, message, f, palette(ui.ctx()).text_secondary, true);
    }
    h
}

/// `FileHelpers.OpenFolderWithFile` for the first output, when there is one.
pub fn open_folder_with_first(files: &[String]) -> Vec<crate::dialogs::Effect> {
    files.first().map(|f| vec![crate::dialogs::Effect::OpenFolderWithFile(PathBuf::from(f))]).unwrap_or_default()
}

/// A label + control row (`Grid ColumnDefinitions="<label>,*"
/// ColumnSpacing="10"`): the control's rect.
pub fn label_row(ui: &Ui, rect: Rect, label_w: f32, text: &str) -> Rect {
    row_label(ui, Rect::from_min_size(rect.min, vec2(label_w, rect.height())), text);
    Rect::from_min_max(pos2(rect.min.x + label_w + 10.0, rect.min.y), rect.max)
}

/// The `Slider` + value `TextBlock` row (`ColumnDefinitions="<label>,*,Auto"`,
/// `ColumnSpacing="10"`, the text `MinWidth="40"` right-aligned in
/// `text_font`, the secondary text colour): returns whether the value
/// changed. `format` makes the text from the value (`{0:0}%`).
#[allow(clippy::too_many_arguments)]
pub fn slider_row(ui: &Ui, rect: Rect, label_w: f32, label: &str, id: Id, value: &mut f32, range: (f32, f32), text_font: FontId, format: &dyn Fn(f32) -> String, enabled: bool) -> bool {
    let p = palette(ui.ctx());
    let control = if label_w > 0.0 { label_row(ui, rect, label_w, label) } else { rect };
    // The `Auto` column is as wide as the text it shows (at least 40).
    let text = format(*value);
    let tw = text_width(ui, &text, text_font.clone()).max(40.0);
    // Fluent's horizontal template: 15px rows above and below the 20px
    // track row; the shared control draws its thumb 13px down a 32px rect.
    let track_center = control.min.y + (control.height() / 2.0).floor();
    let slider_rect = Rect::from_min_max(pos2(control.min.x, track_center - 13.0), pos2(control.max.x - tw - 10.0, track_center + 19.0));
    let mut v = *value;
    let changed = controls::Slider { minimum: range.0, maximum: range.1, step: 0.0, enabled }.show(ui, slider_rect, id, &mut v);
    if changed {
        *value = v;
    }
    // `MinWidth="40" HorizontalAlignment="Right"`: the block is at least 40
    // wide at the column's right, its text at the block's left.
    let text = format(*value);
    let w = text_width(ui, &text, text_font.clone()).max(40.0);
    text_at(ui, pos2(control.max.x - w, center_y(control, line_height(ui, &text_font))), &text, text_font, p.text_secondary);
    changed
}

/// `{0:0}`: a decimal rounded half away from zero, as .NET formats it.
pub fn format_0(v: f32) -> String {
    format!("{}", (v as f64).round() as i64)
}

/// What the image list panel's toolbar asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListAction {
    None,
    Add,
    Remove,
    Up,
    Down,
}

/// The image list panel (`Border.tool-panel`, `Padding="12"`): Add, Remove
/// (and Move up/down when `moves`), the count text right-aligned (`count_font`),
/// the list, and the drop hint over an empty list.
#[allow(clippy::too_many_arguments)]
pub fn list_panel(ui: &Ui, rect: Rect, key: &str, list: &mut FileList, add_text: &str, count: &str, count_font: FontId, hint: &str, hint_font: FontId, moves: bool, live: bool) -> (ListAction, bool) {
    let p = palette(ui.ctx());
    panel(ui, rect);
    let c = rect.shrink(13.0);
    let bar = Rect::from_min_size(c.min, vec2(c.width(), BUTTON_HEIGHT));
    let mut action = ListAction::None;
    let aw = icon_text_button_width(ui, add_text);
    let mut x = bar.min.x;
    if icon_text_button(ui, Rect::from_min_size(pos2(x, bar.min.y), vec2(aw, BUTTON_HEIGHT)), Id::new((key, "add")), sharex_ui::keys::app_icons::Plus, add_text, live).clicked() {
        action = ListAction::Add;
    }
    x += aw + 8.0;
    let can_remove = !list.selected.is_empty() || list.primary.is_some();
    let can_up = list.primary.is_some_and(|i| i > 0);
    let can_down = list.primary.is_some_and(|i| i + 1 < list.items.len());
    use sharex_ui::keys::app_icons as icon;
    let mut buttons = vec![(icon::Trash, can_remove, ListAction::Remove)];
    if moves {
        buttons.push((icon::ArrowUp, can_up, ListAction::Up));
        buttons.push((icon::ArrowDown, can_down, ListAction::Down));
    }
    for (k, (ikey, enabled, a)) in buttons.into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, bar.min.y), vec2(ICON_BUTTON_WIDTH, BUTTON_HEIGHT));
        if icon_button(ui, r, Id::new((key, "btn", k)), ikey, enabled && live).clicked() {
            action = a;
        }
        x += ICON_BUTTON_WIDTH + 8.0;
    }
    let cw = text_width(ui, count, count_font.clone());
    text_at(ui, pos2(bar.max.x - cw, center_y(bar, line_height(ui, &count_font))), count, count_font, p.text_secondary);
    let list_rect = Rect::from_min_max(pos2(c.min.x, bar.max.y + 10.0), c.max);
    let changed = list.ui(ui, list_rect, Id::new((key, "list")), live);
    if list.items.is_empty() {
        empty_state(ui, list_rect, icon::DragDropUpload, 42.0, 8.0, &[(hint, hint_font, p.text_secondary)]);
    }
    (action, changed)
}

/// A Fluent `Slider`'s height: `SliderPreContentMargin` (15), the 20px
/// track row, `SliderPostContentMargin` (15).
pub const SLIDER_HEIGHT: f32 = 50.0;

/// A `Grid`'s height: its rows and `RowSpacing` between every two row
/// definitions (Avalonia keeps the spacing of rows whose content is hidden).
pub fn grid_height(rows: &[f32], spacing: f32) -> f32 {
    rows.iter().sum::<f32>() + spacing * rows.len().saturating_sub(1) as f32
}

/// A `StackPanel`'s height: its visible children and `Spacing` between them.
pub fn stack_height(children: &[f32], spacing: f32) -> f32 {
    let visible: Vec<f32> = children.iter().copied().filter(|h| *h > 0.0).collect();
    visible.iter().sum::<f32>() + spacing * visible.len().saturating_sub(1) as f32
}

/// Converts an `egui` colour to the engine's.
pub fn rgba(c: Color32) -> sharex_tools::imaging::Rgba {
    [c.r(), c.g(), c.b(), 255]
}

/// Avalonia's `ColorPicker` (`IsAlphaEnabled="False"`): the 64x32 button at
/// `rect.min`, centred vertically in `rect`, and its flyout (the shared
/// colour popup) while open. Returns whether the colour changed.
pub fn color_picker(ui: &Ui, rect: Rect, id: Id, color: &mut Color32, enabled: bool) -> bool {
    let size = controls::ColorPickerButton::SIZE;
    let r = Rect::from_min_size(pos2(rect.min.x, center_y(rect, size.y)), size);
    let resp = controls::ColorPickerButton { enabled }.show(ui, r, id, *color);
    let open_id = id.with("open");
    let mut open: bool = ui.ctx().data(|d| d.get_temp(open_id)).unwrap_or(false);
    let just_opened = resp.clicked() && !open;
    if resp.clicked() {
        open = !open;
    }
    let mut changed = false;
    if open {
        let mut c = *color;
        let (ch, keep) = controls::color_popup(ui, id, r, &mut c, just_opened);
        if ch {
            *color = Color32::from_rgb(c.r(), c.g(), c.b());
            changed = true;
        }
        open = keep;
    }
    ui.ctx().data_mut(|d| d.insert_temp(open_id, open));
    changed
}
