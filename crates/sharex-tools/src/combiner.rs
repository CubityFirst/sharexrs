//! Image combiner: `SkiaImageHelpers.CombineImages` (ShareX.HelpersLib) as
//! `TaskHelpers.OpenImageCombiner` runs it, with `ImageCombinerOptions`.

use serde_json::Value;
use sharex_skia::raster::{Canvas, Rect};

use crate::imaging::{self, Bitmap};
use crate::options::{enum_name, json_bool, json_i32};

/// `ImageOrientation` / `ImageCombinerOrientation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// `ImageCombinerAlignment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    LeftOrTop,
    Center,
    RightOrBottom,
}

/// `ImageCombinerOptions` (`TaskSettingsTools.ImageCombinerOptions`).
#[derive(Debug, Clone, PartialEq)]
pub struct ImageCombinerOptions {
    pub orientation: Orientation,
    pub alignment: Alignment,
    pub space: i32,
    pub wrap_after: i32,
    pub auto_fill_background: bool,
}

impl Default for ImageCombinerOptions {
    fn default() -> Self {
        ImageCombinerOptions {
            orientation: Orientation::Vertical,
            alignment: Alignment::LeftOrTop,
            space: 0,
            wrap_after: 0,
            auto_fill_background: true,
        }
    }
}

impl ImageCombinerOptions {
    /// From ShareX's JSON (enums by name or number).
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        ImageCombinerOptions {
            orientation: match enum_name(&v["Orientation"], &["Horizontal", "Vertical"]) {
                Some(0) => Orientation::Horizontal,
                Some(1) => Orientation::Vertical,
                _ => d.orientation,
            },
            alignment: match enum_name(&v["Alignment"], &["LeftOrTop", "Center", "RightOrBottom"]) {
                Some(0) => Alignment::LeftOrTop,
                Some(1) => Alignment::Center,
                Some(2) => Alignment::RightOrBottom,
                _ => d.alignment,
            },
            space: json_i32(&v["Space"]).unwrap_or(d.space),
            wrap_after: json_i32(&v["WrapAfter"]).unwrap_or(d.wrap_after),
            auto_fill_background: json_bool(&v["AutoFillBackground"]).unwrap_or(d.auto_fill_background),
        }
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "Orientation": format!("{:?}", self.orientation),
            "Alignment": format!("{:?}", self.alignment),
            "Space": self.space,
            "WrapAfter": self.wrap_after,
            "AutoFillBackground": self.auto_fill_background,
        })
    }
}

/// `System.Drawing.Rectangle` as `Rectangle.Union` and `Combine` use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct IRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl IRect {
    fn is_empty(&self) -> bool {
        self.x == 0 && self.y == 0 && self.w == 0 && self.h == 0
    }

    fn union(a: IRect, b: IRect) -> IRect {
        let x1 = a.x.min(b.x);
        let x2 = (a.x + a.w).max(b.x + b.w);
        let y1 = a.y.min(b.y);
        let y2 = (a.y + a.h).max(b.y + b.h);
        IRect { x: x1, y: y1, w: x2 - x1, h: y2 - y1 }
    }
}

/// `SkiaImageHelpers.CombineImages(List<SKBitmap>, ...)`.
pub fn combine_images(images: &[Bitmap], o: &ImageCombinerOptions) -> Bitmap {
    let count = images.len();
    let mut rects = vec![IRect::default(); count];
    let (mut px, mut py) = (0i32, 0i32);
    let mut current = 0i32;
    let space = o.space;
    let wrap = o.wrap_after;
    for (i, image) in images.iter().enumerate() {
        let (iw, ih) = (image.width() as i32, image.height() as i32);
        let (mut ox, mut oy) = (0, 0);
        if o.orientation == Orientation::Horizontal {
            if wrap > 0 {
                if i as i32 % wrap == 0 {
                    if i > 0 {
                        px = 0;
                        py += current + space;
                    }
                    current = images[i..].iter().take(wrap as usize).map(|b| b.height() as i32).max().unwrap();
                }
            } else if i == 0 {
                current = images.iter().map(|b| b.height() as i32).max().unwrap();
            }
            oy = match o.alignment {
                Alignment::LeftOrTop => 0,
                Alignment::Center => current / 2 - ih / 2,
                Alignment::RightOrBottom => current - ih,
            };
            rects[i] = IRect { x: px + ox, y: py + oy, w: iw, h: ih };
            px += iw + space;
        } else {
            if wrap > 0 {
                if i as i32 % wrap == 0 {
                    if i > 0 {
                        px += current + space;
                        py = 0;
                    }
                    current = images[i..].iter().take(wrap as usize).map(|b| b.width() as i32).max().unwrap();
                }
            } else if i == 0 {
                current = images.iter().map(|b| b.width() as i32).max().unwrap();
            }
            ox = match o.alignment {
                Alignment::LeftOrTop => 0,
                Alignment::Center => current / 2 - iw / 2,
                Alignment::RightOrBottom => current - iw,
            };
            rects[i] = IRect { x: px + ox, y: py + oy, w: iw, h: ih };
            py += ih + space;
        }
    }
    let mut total = IRect::default();
    for r in &rects {
        total = if total.is_empty() { *r } else { IRect::union(total, *r) };
    }
    let mut bmp = imaging::create_bitmap(total.w.max(0) as u32, total.h.max(0) as u32);
    {
        let mut g = Canvas::new(&mut bmp);
        for (i, image) in images.iter().enumerate() {
            if o.auto_fill_background && i == 0 {
                let c = imaging::get_color(image, image.width() - 1, image.height() - 1);
                g.clear(c);
            }
            let r = rects[i];
            let src = Rect::xywh(0.0, 0.0, image.width() as f32, image.height() as f32);
            imaging::draw_image(&mut g, image, src, Rect::xywh(r.x as f32, r.y as f32, r.w as f32, r.h as f32), None);
        }
    }
    bmp
}

/// `SkiaImageHelpers.CombineImages(IEnumerable<string>, ...)`: the images that
/// load, combined when there are at least two.
pub fn combine_files<P: AsRef<std::path::Path>>(files: &[P], o: &ImageCombinerOptions) -> Option<Bitmap> {
    let images: Vec<Bitmap> = files.iter().filter_map(imaging::load_image).collect();
    (images.len() > 1).then(|| combine_images(&images, o))
}

/// `CreateImageCombinerPreviewAsync`: the combined image as PNG.
pub fn create_preview<P: AsRef<std::path::Path>>(files: &[P], o: &ImageCombinerOptions) -> Option<Vec<u8>> {
    combine_files(files, o).map(|b| imaging::encode_png(&b))
}
