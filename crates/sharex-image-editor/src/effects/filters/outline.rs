//! `OutlineImageEffect` (`Core/ImageEffects/Filters/OutlineImageEffect.cs`).

use crate::effects::prelude::*;

/// `OutlineImageEffect`.
#[derive(Clone, Debug)]
pub struct OutlineImageEffect {
    pub size: i32,
    pub padding: i32,
    pub outline_only: bool,
    pub color: SkColor,
}

impl Default for OutlineImageEffect {
    fn default() -> Self {
        OutlineImageEffect {
            size: 3,
            padding: 0,
            outline_only: false,
            color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for OutlineImageEffect {
    fn info() -> Info {
        Info::new(
            "outline",
            "Outline",
            EffectCategory::Filters,
            "image-effect/square-dashed",
        )
        .desc("Applies an outline effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 1, 50, 3, |e: &mut Self, v| e.size = v).snap(false),
            int_slider::<Self>("padding", "Padding", 0, 100, 0, |e: &mut Self, v| {
                e.padding = v
            })
            .snap(false),
            bool_param::<Self>("outline_only", "Outline only", false, |e: &mut Self, v| {
                e.outline_only = v
            }),
            color_param::<Self>(
                "color",
                "Color",
                SkColor::argb(0xFF000000),
                |e: &mut Self, v| e.color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        use sharex_skia::raster as r;
        if self.size <= 0 {
            return Ok(source.clone());
        }
        let total = self.size + self.padding;
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (sw + total * 2, sh + total * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let n = (nw.max(0) * nh.max(0)) as usize;
        // The dilated footprint, tinted by the lazy SrcIn colour filter as it's drawn
        // (SkRasterPipelineSpriteBlitter, highp), src-over onto transparent.
        let outer = dilate(source, total.min(256), total, nw, nh);
        let tint = r::premul_f(r::load(self.color.rgba_array()));
        for (i, o) in outer.iter().enumerate().take(n) {
            let da = r::from_byte(o[3]);
            let c = r::clamp_01(tint.map(|t| t * da));
            let d = out.px(i);
            let inv = 1.0 - c[3];
            out.set_px(
                i,
                [0, 1, 2, 3]
                    .map(|k| r::to_unorm(r::mad(d[k] as f32, inv, c[k] * 255.0), 1.0, 255.0)),
            );
        }
        if self.padding > 0 {
            let gap = dilate(source, self.padding.min(256), total, nw, nh);
            for (i, s) in gap.iter().enumerate() {
                dst_out(&mut out, i, *s);
            }
        }
        if self.outline_only {
            for y in 0..sh {
                for x in 0..sw {
                    let i = ((y + total) * nw + x + total) as usize;
                    dst_out(&mut out, i, source.px((y * sw + x) as usize));
                }
            }
        } else {
            canvas_draw(&mut out, source, total);
        }
        Ok(out)
    }
}

// ---- port ----

/// `SkImageFilters::Dilate(radius)` of `source` drawn at (`at`, `at`) on a
/// `w`x`h` device: each channel's maximum over the (2r+1)² window, transparent
/// outside the image.
fn dilate(source: &Bitmap, radius: i32, at: i32, w: i32, h: i32) -> Vec<[u8; 4]> {
    let (sw, sh) = (source.width() as i32, source.height() as i32);
    let mut xs = vec![[0u8; 4]; (w.max(0) * h.max(0)) as usize];
    // Rows outside the image stay transparent; only image columns can contribute.
    for sy in 0..sh {
        let y = sy + at;
        if y < 0 || y >= h {
            continue;
        }
        for x in 0..w {
            let lo = (x - radius - at).max(0);
            let hi = (x + radius - at).min(sw - 1);
            let mut m = [0u8; 4];
            for sx in lo..=hi {
                let p = source.px((sy * sw + sx) as usize);
                for c in 0..4 {
                    m[c] = m[c].max(p[c]);
                }
            }
            xs[(y * w + x) as usize] = m;
        }
    }
    let mut out = vec![[0u8; 4]; xs.len()];
    for y in 0..h {
        let lo = (y - radius).max(at).max(0);
        let hi = (y + radius).min(at + sh - 1).min(h - 1);
        for x in 0..w {
            let mut m = [0u8; 4];
            for yy in lo..=hi {
                let p = xs[(yy * w + x) as usize];
                for c in 0..4 {
                    m[c] = m[c].max(p[c]);
                }
            }
            out[(y * w + x) as usize] = m;
        }
    }
    out
}

/// A DstOut sprite pixel (lowp: `div255_accurate(d * (255 - sa))`).
fn dst_out(out: &mut Bitmap, i: usize, s: [u8; 4]) {
    let d = out.px(i);
    let inv = 255 - s[3] as u32;
    out.set_px(
        i,
        d.map(|v| sharex_skia::raster::div255_accurate(v as u32 * inv) as u8),
    );
}

fn canvas_draw(out: &mut Bitmap, source: &Bitmap, at: i32) {
    Canvas::new(out).draw_bitmap(source, at as f32, at as f32, None);
}
