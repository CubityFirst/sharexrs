//! `JpegArtifactImageEffect` (`Core/ImageEffects/Filters/JpegArtifactImageEffect.cs`).

use crate::effects::prelude::*;

/// `JpegArtifactImageEffect`.
#[derive(Clone, Debug)]
pub struct JpegArtifactImageEffect {
    pub compression: f32,
    pub block_size: i32,
    pub chroma_bleed: f32,
    pub ringing: f32,
}

impl Default for JpegArtifactImageEffect {
    fn default() -> Self {
        JpegArtifactImageEffect {
            compression: 62.0,
            block_size: 8,
            chroma_bleed: 45.0,
            ringing: 28.0,
        }
    }
}

impl EffectImpl for JpegArtifactImageEffect {
    fn info() -> Info {
        Info::new("jpeg_artifact", "JPEG artifact", EffectCategory::Filters, "app/file-image")
            .desc("Simulates JPEG compression artifacts with blocking and ringing.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("compression", "Compression", 0.0, 100.0, 62.0, |e: &mut Self, v| e.compression = v),
            int_slider::<Self>("block_size", "Block size", 4, 32, 8, |e: &mut Self, v| e.block_size = v),
            float_slider::<Self>("chroma_bleed", "Chroma bleed", 0.0, 100.0, 45.0, |e: &mut Self, v| e.chroma_bleed = v),
            float_slider::<Self>("ringing", "Ringing", 0.0, 100.0, 28.0, |e: &mut Self, v| e.ringing = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let amount = clampf(self.compression, 0.0, 100.0) / 100.0;
        let block = self.block_size.clamp(4, 32);
        let bleed = clampf(self.chroma_bleed, 0.0, 100.0) / 100.0;
        let ringing = clampf(self.ringing, 0.0, 100.0) / 100.0;
        if amount <= 0.0001 && ringing <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut comp = vec![SkColor::default(); src.len()];
        let luma_step = 1.0 + (amount * 22.0);
        let chroma_step = 2.0 + (amount * 38.0);
        let quant = |v: f32, step: f32| (v / step).round_ties_even() * step;
        let ycc = |c: SkColor| {
            let (r, g, b) = (c.r as f32, c.g as f32, c.b as f32);
            (
                (0.299 * r) + (0.587 * g) + (0.114 * b),
                128.0 - (0.168736 * r) - (0.331264 * g) + (0.5 * b),
                128.0 + (0.5 * r) - (0.418688 * g) - (0.081312 * b),
            )
        };
        let mut by = 0;
        while by < height {
            let y1 = height.min(by + block);
            let mut bx = 0;
            while bx < width {
                let x1 = width.min(bx + block);
                let (mut ay, mut acb, mut acr) = (0f32, 0f32, 0f32);
                let mut count = 0;
                for y in by..y1 {
                    for x in bx..x1 {
                        let (yy, cb, cr) = ycc(src[(y * width + x) as usize]);
                        ay += yy;
                        acb += cb;
                        acr += cr;
                        count += 1;
                    }
                }
                if count > 0 {
                    let inv = 1.0 / count as f32;
                    ay *= inv;
                    acb *= inv;
                    acr *= inv;
                }
                for y in by..y1 {
                    for x in bx..x1 {
                        let s = src[(y * width + x) as usize];
                        let (yy, cb, cr) = ycc(s);
                        let qy = quant(lerp(yy, ay, amount * 0.16), luma_step);
                        let mix = 0.15 + (bleed * 0.70) + (amount * 0.08);
                        let qcb = quant(lerp(cb, acb, mix), chroma_step);
                        let qcr = quant(lerp(cr, acr, mix), chroma_step);
                        let r = qy + (1.402 * (qcr - 128.0));
                        let g = qy - (0.344136 * (qcb - 128.0)) - (0.714136 * (qcr - 128.0));
                        let b = qy + (1.772 * (qcb - 128.0));
                        comp[(y * width + x) as usize] = SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), s.a);
                    }
                }
                bx += block;
            }
            by += block;
        }
        if ringing <= 0.0001 {
            return Ok(from_pixels(width as u32, height as u32, &comp));
        }
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let cur = comp[(row + x) as usize];
                let (lx, ly) = (x % block, y % block);
                let fx = if lx <= 1 || lx >= block - 2 { 1f32 } else { 0.0 };
                let fy = if ly <= 1 || ly >= block - 2 { 1f32 } else { 0.0 };
                let boundary = fx.max(fy);
                if boundary <= 0.0 {
                    dst[(row + x) as usize] = cur;
                    continue;
                }
                let l = analog_luminance(comp[(row + 0.max(x - 1)) as usize]);
                let r = analog_luminance(comp[(row + (width - 1).min(x + 1)) as usize]);
                let t = analog_luminance(comp[(0.max(y - 1) * width + x) as usize]);
                let b = analog_luminance(comp[((height - 1).min(y + 1) * width + x) as usize]);
                let edge = ((r - l) + (b - t)) * 0.25;
                let osc = (((x + y) as f32 * 0.82) + (((x / block) + (y / block)) as f32 * 1.7)).sin();
                let sharpen = edge * osc * ringing * boundary * 0.32;
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(cur.r as f32 + (sharpen * 1.05)),
                    clamp_to_byte(cur.g as f32 + (sharpen * 0.86)),
                    clamp_to_byte(cur.b as f32 - (sharpen * 0.30)),
                    cur.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
