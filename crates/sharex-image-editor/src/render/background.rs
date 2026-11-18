//! The Background panel's export: `EditorView.GetSnapshot()` while the
//! panel is open (`MainViewModel.AreBackgroundEffectsActive`). The editor
//! renders its `PreviewFrame`:
//!
//! ```text
//! Border PreviewFrame       Background = the background brush, Padding = CanvasPadding
//!   Border                  BoxShadow = CanvasShadow, CornerRadius
//!     Border                Background = SmartPaddingColor, Padding = SmartPaddingThickness,
//!                           CornerRadius, ClipToBounds
//!       Canvas (viewport)   the image, cropped by smart padding, with its annotations
//! ```
//!
//! through Avalonia's `ImmediateRenderer` into a 96-DPI
//! `RenderTargetBitmap` (Skia, N32, legacy blitters): the background
//! filled over the whole frame (`DrawRect`; a gradient through the raster
//! pipeline, an image through `ConfigureTileBrush`'s intermediate), the
//! canvas shadow (`DrawingContextImpl.DrawRectangle` with a `BoxShadow`: the
//! rounded rect drawn 10px lower through an `SkImageFilters::Blur` layer,
//! under an anti-aliased difference clip of the rect), then, inside an
//! anti-aliased rounded clip, the padding colour and the image.
//! (`MainViewModel.BackgroundState`/`CanvasState`, `BackgroundSettingsPanel`,
//! `EditorView.CoreBridge.GetSnapshot`.)

use sharex_skia::Pixmap;
use sharex_skia::raster::gradient::LinearGradient;
use sharex_skia::raster::{
    Bitmap, Canvas, Matrix as RMatrix, Paint, Rect as RRect, Sampling, Shader, TileMode,
};

/// `DesktopWallpaperLayout`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Fill,
    Fit,
    Stretch,
    Center,
    Tile,
    Span,
}

/// The background brush (`MainViewModel.CanvasBackgroundMode`).
#[derive(Clone, Debug)]
pub enum Fill {
    /// `CreateGradientBrush`: straight colours, top left to bottom right.
    Gradient([u8; 4], [u8; 4]),
    /// A straight colour.
    Color([u8; 4]),
    Transparent,
    /// An image (`CreateImageBrush` with its layout): the image mode's
    /// file (`Fill`) or the desktop wallpaper.
    Image(Pixmap, Layout),
}

/// The panel's settings.
#[derive(Clone, Debug)]
pub struct Background {
    pub margin: f64,
    pub padding: f64,
    pub smart_padding: bool,
    pub corner: f64,
    pub shadow: f64,
    pub fill: Fill,
    /// `TargetOutputAspectRatio` (`None`: Auto).
    pub ratio: Option<f64>,
    /// `ImageEditorOptions.ShadowColor`, straight RGBA.
    pub shadow_color: [u8; 4],
}

/// Where everything lands in the exported frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The export's size (`GetSnapshot`'s pixel size).
    pub width: u32,
    pub height: u32,
    /// `CanvasPadding` (left, top, right, bottom).
    pub canvas_padding: (f64, f64, f64, f64),
    /// The shadow/padding border's rect (layout-rounded).
    pub card: (i32, i32, i32, i32),
    /// The viewport (`SmartPaddingViewportWidth/Height`) and where it sits.
    pub viewport: (i32, i32, i32, i32),
    /// The image pixel at the viewport's top left (`-SmartPaddingOffsetX/Y`).
    pub crop: (i32, i32),
}

/// `CanvasBackgroundMode`, as `ImageEditorOptions.BackgroundType` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Gradient,
    Color,
    Transparent,
    Image,
    Wallpaper,
}

impl Mode {
    pub const ALL: [Mode; 5] = [
        Mode::Gradient,
        Mode::Color,
        Mode::Transparent,
        Mode::Image,
        Mode::Wallpaper,
    ];

    /// `ParseBackgroundMode` (`Enum.TryParse`, ignoring case; else
    /// Transparent).
    pub fn parse(s: &str) -> Mode {
        let s = s.trim();
        Mode::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(s))
            .unwrap_or(Mode::Transparent)
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Gradient => "Gradient",
            Mode::Color => "Color",
            Mode::Transparent => "Transparent",
            Mode::Image => "Image",
            Mode::Wallpaper => "Wallpaper",
        }
    }
}

/// `NormalizeBackgroundColorHex`: `#AARRGGBB`, or the fallback when the
/// value isn't a colour.
pub fn normalize_hex(hex: &str, fallback: [u8; 4]) -> String {
    let c = if hex.trim().is_empty() {
        None
    } else {
        super::color::try_parse(hex)
    };
    let c = c.map_or(fallback, |c| [c.r, c.g, c.b, c.a]);
    format!("#{:02X}{:02X}{:02X}{:02X}", c[3], c[0], c[1], c[2])
}

/// A straight RGBA colour from `#AARRGGBB` (transparent when invalid).
pub fn rgba(hex: &str) -> [u8; 4] {
    let c = super::color::parse_color(hex);
    [c.r, c.g, c.b, c.a]
}

/// The fallbacks `InitializeBackgroundSettingsFromOptions` uses.
pub const GRADIENT1_FALLBACK: [u8; 4] = [255, 126, 95, 255];
pub const GRADIENT2_FALLBACK: [u8; 4] = [254, 180, 123, 255];
pub const COLOR_FALLBACK: [u8; 4] = [34, 34, 34, 255];

/// The background from `ImageEditorOptions` as the editor applies it
/// (`InitializeBackgroundSettingsFromOptions`, `ApplySelectedBackgroundMode`):
/// `wallpaper` is the desktop wallpaper service's answer (`None` when it
/// isn't supported: a saved Wallpaper mode then falls back to the first
/// option, Gradient), `load` reads an image file.
pub fn from_options(
    o: &crate::options::ImageEditorOptions,
    ratio: Option<f64>,
    wallpaper: Option<Option<(std::path::PathBuf, Layout)>>,
    load: impl Fn(&std::path::Path) -> Option<Pixmap>,
) -> Background {
    let mut mode = Mode::parse(&o.BackgroundType);
    if mode == Mode::Wallpaper && wallpaper.is_none() {
        mode = Mode::Gradient;
    }
    let image = |path: &std::path::Path, layout| match load(path) {
        Some(pm) => Fill::Image(pm, layout),
        None => Fill::Transparent,
    };
    let fill = match mode {
        Mode::Gradient => Fill::Gradient(
            rgba(&normalize_hex(
                &o.BackgroundGradientColor1Hex,
                GRADIENT1_FALLBACK,
            )),
            rgba(&normalize_hex(
                &o.BackgroundGradientColor2Hex,
                GRADIENT2_FALLBACK,
            )),
        ),
        Mode::Color => Fill::Color(rgba(&normalize_hex(&o.BackgroundColorHex, COLOR_FALLBACK))),
        Mode::Transparent => Fill::Transparent,
        Mode::Image if o.BackgroundImagePath.trim().is_empty() => Fill::Transparent,
        Mode::Image => image(std::path::Path::new(&o.BackgroundImagePath), Layout::Fill),
        Mode::Wallpaper => match wallpaper.flatten() {
            Some((path, layout)) => image(&path, layout),
            None => Fill::Transparent,
        },
    };
    let shadow =
        super::color::try_parse(&o.ShadowColorHex).map_or([0, 0, 0, 128], |c| [c.r, c.g, c.b, c.a]);
    Background {
        margin: o.BackgroundMargin.max(0.0),
        padding: o.BackgroundPadding.max(0.0),
        smart_padding: o.BackgroundSmartPadding,
        corner: o.BackgroundRoundedCorner.max(0.0),
        shadow: o.BackgroundShadowRadius.max(0.0),
        fill,
        ratio,
        shadow_color: shadow,
    }
}

/// `ParseAspectRatio`: "16:9" -> 16/9, "Auto" (or anything else) -> None.
pub fn parse_ratio(ratio: &str) -> Option<f64> {
    if ratio.trim().is_empty() || ratio.eq_ignore_ascii_case("Auto") {
        return None;
    }
    let parts: Vec<&str> = ratio.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let (w, h) = (
        parts[0].trim().parse::<f64>().ok()?,
        parts[1].trim().parse::<f64>().ok()?,
    );
    (w > 0.0 && h > 0.0).then_some(w / h)
}

/// `EnsureSmartPaddingCache`: the uniform border round the image (within
/// 30 of the top-left pixel in every channel) as crop insets (left, top,
/// right, bottom); all zero when there's none. The target colour is
/// `GetPixel` (unpremultiplied), the pixels the bitmap's premultiplied
/// bytes, as ShareX compares them.
pub fn smart_padding_insets(source: &Pixmap) -> (i32, i32, i32, i32) {
    let (w, h) = (source.width() as i32, source.height() as i32);
    if w == 0 || h == 0 {
        return (0, 0, 0, 0);
    }
    let target = unpremultiply(source.pixel(0, 0));
    const TOLERANCE: i32 = 30;
    let differs = |c: u32| {
        let px = [
            (c >> 16 & 0xFF) as i32,
            (c >> 8 & 0xFF) as i32,
            (c & 0xFF) as i32,
            (c >> 24) as i32,
        ];
        (0..4).any(|i| (px[i] - target[i] as i32).abs() > TOLERANCE)
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0, 0);
    for y in 0..h {
        let mut row_has_content = false;
        for x in 0..w {
            if differs(source.pixel(x, y)) {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
                row_has_content = true;
                break;
            }
        }
        if row_has_content {
            for x in (0..w).rev() {
                if differs(source.pixel(x, y)) {
                    max_x = max_x.max(x);
                    break;
                }
            }
        }
    }
    if min_x > max_x || min_y > max_y {
        return (0, 0, 0, 0);
    }
    (min_x, min_y, (w - max_x - 1).max(0), (h - max_y - 1).max(0))
}

/// Whether the insets crop anything (`HasDetectedSmartPadding`).
pub fn has_smart_padding(insets: (i32, i32, i32, i32)) -> bool {
    insets.0 > 0 || insets.1 > 0 || insets.2 > 0 || insets.3 > 0
}

/// `SKColor` of a premultiplied pixel (`SKBitmap.GetPixel`), as RGBA.
fn unpremultiply(c: u32) -> [u8; 4] {
    let a = c >> 24;
    if a == 0 {
        return [0, 0, 0, 0];
    }
    if a == 255 {
        return [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255];
    }
    // SkUnPreMultiply::PMColorToColor (the reciprocal table).
    let scale = (255 << 24) / a;
    let un = |v: u32| ((v * scale + (1 << 23)) >> 24) as u8;
    [un(c >> 16 & 0xFF), un(c >> 8 & 0xFF), un(c & 0xFF), a as u8]
}

/// `CalculateOutputPadding(BackgroundMargin, TargetOutputAspectRatio)`.
fn output_padding(margin: f64, ratio: Option<f64>, vw: f64, vh: f64) -> (f64, f64, f64, f64) {
    let Some(target) = ratio.filter(|_| vw > 0.0 && vh > 0.0) else {
        return (margin, margin, margin, margin);
    };
    let total_w = vw + margin * 2.0;
    let total_h = vh + margin * 2.0;
    let current = total_w / total_h;
    let (mut extra_x, mut extra_y) = (0.0, 0.0);
    const EPSILON: f64 = 0.0001;
    if current > target + EPSILON {
        extra_y = (total_w / target - total_h).max(0.0) / 2.0;
    } else if current + EPSILON < target {
        extra_x = (total_h * target - total_w).max(0.0) / 2.0;
    }
    (
        margin + extra_x,
        margin + extra_y,
        margin + extra_x,
        margin + extra_y,
    )
}

/// `LayoutHelper.RoundLayoutValue` at scale 1 (`Math.Round`, to even).
fn round_layout(v: f64) -> f64 {
    v.round_ties_even()
}

/// The frame for an image of `size` with smart padding's `insets` (when
/// smart padding is on and found something).
pub fn frame(size: (u32, u32), insets: Option<(i32, i32, i32, i32)>, bg: &Background) -> Frame {
    let (iw, ih) = (size.0 as i32, size.1 as i32);
    let ins = insets
        .filter(|i| has_smart_padding(*i))
        .unwrap_or((0, 0, 0, 0));
    // SmartPaddingViewportWidth/Height.
    let vw = (iw - ins.0 - ins.2).max(0);
    let vh = (ih - ins.1 - ins.3).max(0);
    let cp = output_padding(bg.margin, bg.ratio, vw as f64, vh as f64);
    let p = bg.padding;
    // GetSnapshot's pixel size (MidpointRounding.AwayFromZero).
    let width = ((vw as f64 + p + p + cp.0 + cp.2).round() as i64).max(1) as u32;
    let height = ((vh as f64 + p + p + cp.1 + cp.3).round() as i64).max(1) as u32;
    // PreviewFrame's Padding, rounded (LayoutHelper.ArrangeChild).
    let (l, t, r, b) = (
        round_layout(cp.0),
        round_layout(cp.1),
        round_layout(cp.2),
        round_layout(cp.3),
    );
    let card_w = (width as f64 - l - r).max(0.0);
    let card_h = (height as f64 - t - b).max(0.0);
    // The padding border's child: the viewport Canvas (its Width/Height),
    // centred in the slot its rounded Padding leaves.
    let rp = round_layout(p);
    let cx = round_layout(rp + (card_w - rp - rp - vw as f64) / 2.0);
    let cy = round_layout(rp + (card_h - rp - rp - vh as f64) / 2.0);
    Frame {
        width,
        height,
        canvas_padding: cp,
        card: (l as i32, t as i32, card_w as i32, card_h as i32),
        viewport: (l as i32 + cx as i32, t as i32 + cy as i32, vw, vh),
        crop: (ins.0, ins.1),
    }
}

/// `CreateCanvasShadowColor`: the shadow colour's alpha scaled from the
/// default shadow's 128 to 80.
fn canvas_shadow_color(c: [u8; 4]) -> [u8; 4] {
    let a = ((c[3] as f64 * (80.0 / 128.0)).round_ties_even() as i64).clamp(0, 255) as u8;
    [c[0], c[1], c[2], a]
}

/// `SkBlurRadiusToSigma`.
fn sigma(radius: f64) -> f32 {
    if radius <= 0.0 {
        0.0
    } else {
        0.288675f32 * radius as f32 + 0.5
    }
}

fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> sharex_skia::Path {
    let mut p = sharex_skia::Path::new();
    let rect = sharex_skia::Rect::from_ltrb(x, y, x + w, y + h);
    if r > 0.0 {
        p.add_round_rect(rect, r, r, sharex_skia::Direction::Cw);
    } else {
        p.add_rect(rect, sharex_skia::Direction::Cw, 0);
    }
    p
}

/// Renders the export: `content` is the image with its annotations
/// (`render::render`), `top_left` the image's own top-left pixel
/// (`SmartPaddingColor`, sampled from the image without annotations).
pub fn render(
    content: &Pixmap,
    top_left: u32,
    insets: Option<(i32, i32, i32, i32)>,
    bg: &Background,
) -> Pixmap {
    let f = frame((content.width(), content.height()), insets, bg);
    let (w, h) = (f.width, f.height);
    let mut out = Bitmap::new(w, h);
    let device = sharex_skia::IRect::from_ltrb(0, 0, w as i32, h as i32);
    // 1. PreviewFrame's background over the whole frame.
    {
        let mut canvas = Canvas::new(&mut out);
        fill_background(&mut canvas, &bg.fill, w, h);
    }
    let (cx, cy, cw, ch) = f.card;
    let radius = bg.corner.max(0.0) as f32;
    let rounded = radius > 0.0;
    // 2. The canvas shadow (DrawRectangle with a BoxShadow).
    let color = canvas_shadow_color(bg.shadow_color);
    if cw > 0 && ch > 0 && color[3] > 0 {
        draw_box_shadow(&mut out, device, (cx, cy, cw, ch), radius, bg.shadow, color);
    }
    // 3. The padding border: its rounded clip, the padding colour, then
    // the viewport (clipped to itself) with the image.
    if cw > 0 && ch > 0 {
        let clip_path = rrect_path(cx as f32, cy as f32, cw as f32, ch as f32, radius);
        let mut canvas = Canvas::new(&mut out);
        canvas.aa_clip = Some(sharex_skia::aaclip::AAClip::from_path(&clip_path, device));
        let pad = unpremultiply(top_left);
        if pad[3] > 0 {
            let paint = Paint::fill(pad);
            if rounded {
                canvas.fill_sk_path(&clip_path, &paint);
            } else {
                canvas.draw_rect(
                    RRect::xywh(cx as f32, cy as f32, cw as f32, ch as f32),
                    &paint,
                );
            }
        }
        let (vx, vy, vw, vh) = f.viewport;
        if vw > 0 && vh > 0 {
            let (sx, sy) = f.crop;
            let mut view = Bitmap::new(vw as u32, vh as u32);
            for y in 0..vh {
                for x in 0..vw {
                    let c = content.pixel(x + sx, y + sy);
                    view.set_pixel(
                        x as u32,
                        y as u32,
                        [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8],
                    );
                }
            }
            // SKCanvasControl: DrawImage(image, rect, rect, sampling, white).
            let mut paint = Paint::shader(Shader::Color([255, 255, 255, 255]));
            paint.anti_alias = true;
            canvas.draw_bitmap(&view, vx as f32, vy as f32, Some(&paint));
        }
    }
    Pixmap::from_premul_rgba(w, h, out.data())
}

/// The background brush over `(0, 0, w, h)` (`DrawRectangle(brush, null,
/// rect)`: `DrawRect` with the brush's paint, anti-aliased).
fn fill_background(canvas: &mut Canvas, fill: &Fill, w: u32, h: u32) {
    let rect = RRect::xywh(0.0, 0.0, w as f32, h as f32);
    match fill {
        Fill::Transparent => {}
        Fill::Color(c) => {
            if c[3] > 0 {
                canvas.draw_rect(rect, &Paint::fill(*c));
            }
        }
        Fill::Gradient(c1, c2) => {
            // LinearGradientBrush (0,0) -> (1,1) relative, clamped.
            let mut paint = Paint::shader(Shader::Gradient(LinearGradient {
                start: (0.0, 0.0),
                end: (w as f32, h as f32),
                colors: vec![*c1, *c2],
                positions: Some(vec![0.0, 1.0]),
                local: RMatrix::IDENTITY,
            }));
            paint.anti_alias = true;
            canvas.draw_rect(rect, &paint);
        }
        Fill::Image(image, layout) => fill_image(canvas, image, *layout, w, h),
    }
}

/// `CreateImageBrush` + `ConfigureTileBrush`: the bitmap drawn into an
/// intermediate surface (the brush's stretch and centring, linear
/// sampling), then that surface as an image shader over the rect (decal,
/// or repeated for a tiled wallpaper).
fn fill_image(canvas: &mut Canvas, image: &Pixmap, layout: Layout, w: u32, h: u32) {
    let (bw, bh) = (image.width() as f64, image.height() as f64);
    if bw <= 0.0 || bh <= 0.0 {
        return;
    }
    let tile = layout == Layout::Tile;
    // Stretch.CalculateScaling(destination, source).
    let (tw, th) = if tile { (bw, bh) } else { (w as f64, h as f64) };
    let (sx, sy) = (tw / bw, th / bh);
    let scale = match layout {
        Layout::Fill => (sx.max(sy), sx.max(sy)),
        Layout::Fit => (sx.min(sy), sx.min(sy)),
        Layout::Stretch | Layout::Span => (sx, sy),
        Layout::Center | Layout::Tile => (1.0, 1.0),
    };
    // CalculateTranslate (centred).
    let tx = (tw - bw * scale.0) / 2.0;
    let ty = (th - bh * scale.1) / 2.0;
    let (iw, ih) = (tw.ceil() as u32, th.ceil() as u32);
    let mut inter = Bitmap::new(iw, ih);
    {
        let source =
            Bitmap::from_premultiplied(image.width(), image.height(), image.to_premul_rgba());
        let mut c = Canvas::new(&mut inter);
        c.matrix = RMatrix([
            scale.0 as f32,
            0.0,
            tx as f32,
            0.0,
            scale.1 as f32,
            ty as f32,
            0.0,
            0.0,
            1.0,
        ]);
        let mut paint = Paint::shader(Shader::Color([255, 255, 255, 255]));
        paint.anti_alias = true;
        let r = RRect::xywh(0.0, 0.0, bw as f32, bh as f32);
        c.draw_image_rect(&source, Some(r), r, Sampling::Linear, Some(&paint));
    }
    let mode = if tile {
        TileMode::Repeat
    } else {
        TileMode::Decal
    };
    let mut paint = Paint::shader(Shader::Image {
        image: &inter,
        matrix: RMatrix::IDENTITY,
        sampling: Sampling::Nearest,
        tile_x: mode,
        tile_y: mode,
    });
    paint.anti_alias = true;
    canvas.draw_rect(RRect::xywh(0.0, 0.0, w as f32, h as f32), &paint);
}

/// `DrawingContextImpl.DrawRectangle(null, null, rect, boxShadows)` for
/// the canvas's outer shadow: the clip is the device minus the rect
/// (anti-aliased when rounded), and the rect, 10px lower, is drawn with a
/// blur image filter (a layer, blurred, drawn back through the clip).
fn draw_box_shadow(
    out: &mut Bitmap,
    device: sharex_skia::IRect,
    rect: (i32, i32, i32, i32),
    radius: f32,
    blur: f64,
    color: [u8; 4],
) {
    let (x, y, w, h) = rect;
    let shape = rrect_path(x as f32, y as f32, w as f32, h as f32, radius);
    let clip = sharex_skia::aaclip::AAClip::from_path(&shape, device).difference_from(device);
    let s = sigma(blur.max(0.0));
    // BoxShadow's OffsetY, a whole number of pixels.
    let offset = 10;
    let (dw, dh) = (device.width(), device.height());
    let mut cropped = Bitmap::new(dw as u32, dh as u32);
    if s > 0.0 {
        // AutoLayerForImageFilter: the layer is the shape's bounds (rounded
        // out) cut to what the blur needs for the visible output (the clip
        // grown by ceil(3 sigma)), and the shape is drawn in that layer's
        // coordinates. Skia's analytic AA depends on where a path sits, so
        // the layer's origin matters to the pixels.
        let pad = (s * 3.0).ceil() as i32;
        let (sl, st, sr, sb) = (x, y + offset, x + w, y + offset + h);
        let (ll, lt) = (sl.max(-pad), st.max(-pad));
        let (lr, lb) = (sr.min(dw + pad), sb.min(dh + pad));
        if ll >= lr || lt >= lb {
            return;
        }
        let (lw, lh) = (lr - ll, lb - lt);
        let mut layer = Bitmap::new(lw as u32, lh as u32);
        {
            let moved = rrect_path((sl - ll) as f32, (st - lt) as f32, w as f32, h as f32, radius);
            Canvas::new(&mut layer).fill_sk_path(&moved, &Paint::fill(color));
        }
        // The blur reads transparent pixels beyond the layer and writes the
        // layer grown by its reach.
        let (pw, ph) = (lw + 2 * pad, lh + 2 * pad);
        let mut padded = vec![0u8; (pw * ph * 4) as usize];
        for yy in 0..lh {
            let src = &layer.data()[(yy * lw * 4) as usize..((yy + 1) * lw * 4) as usize];
            let at = (((yy + pad) * pw + pad) * 4) as usize;
            padded[at..at + src.len()].copy_from_slice(src);
        }
        let blurred = sharex_skia::blur::blur(&Pixmap::from_premul_rgba(pw as u32, ph as u32, &padded), s, s);
        let (ox, oy) = (ll - pad, lt - pad);
        for yy in oy.max(0)..(oy + ph).min(dh) {
            for xx in ox.max(0)..(ox + pw).min(dw) {
                let c = blurred.pixel(xx - ox, yy - oy);
                cropped.set_pixel(
                    xx as u32,
                    yy as u32,
                    [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8],
                );
            }
        }
    } else {
        // No image filter (a zero blur): the shape straight onto the device.
        let moved = rrect_path(x as f32, (y + offset) as f32, w as f32, h as f32, radius);
        Canvas::new(&mut cropped).fill_sk_path(&moved, &Paint::fill(color));
    }
    // drawSpecial with the restore paint (no anti-aliasing): a sprite
    // unless the clip cuts it, then an image shader over the rect.
    let mut canvas = Canvas::new(out);
    canvas.aa_clip = Some(clip);
    let paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
    canvas.draw_bitmap(&cropped, 0.0, 0.0, Some(&paint));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratios() {
        assert_eq!(parse_ratio("Auto"), None);
        assert_eq!(parse_ratio("16:9"), Some(16.0 / 9.0));
        assert_eq!(parse_ratio("1:0"), None);
    }

    #[test]
    fn frame_sizes() {
        let bg = Background {
            margin: 80.0,
            padding: 40.0,
            smart_padding: false,
            corner: 20.0,
            shadow: 30.0,
            fill: Fill::Transparent,
            ratio: None,
            shadow_color: [0, 0, 0, 128],
        };
        let f = frame((400, 300), None, &bg);
        assert_eq!((f.width, f.height), (640, 540));
        assert_eq!(f.card, (80, 80, 480, 380));
        assert_eq!(f.viewport, (120, 120, 400, 300));
        let f = frame(
            (400, 300),
            None,
            &Background {
                ratio: Some(16.0 / 9.0),
                ..bg.clone()
            },
        );
        assert_eq!((f.width, f.height), (898, 540));
    }
}
