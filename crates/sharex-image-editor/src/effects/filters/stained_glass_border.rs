//! `StainedGlassBorderImageEffect` (`Core/ImageEffects/Filters/StainedGlassBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `StainedGlassBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct StainedGlassBorderImageEffect {
    pub border_size: i32,
    pub panel_width: f32,
    pub lead_thickness: f32,
    pub glass_brightness: f32,
    pub glow: bool,
    pub inner_line: bool,
}

impl Default for StainedGlassBorderImageEffect {
    fn default() -> Self {
        StainedGlassBorderImageEffect {
            border_size: 60,
            panel_width: 40.0,
            lead_thickness: 3.5,
            glass_brightness: 75.0,
            glow: true,
            inner_line: true,
        }
    }
}

impl EffectImpl for StainedGlassBorderImageEffect {
    fn info() -> Info {
        Info::new("stained_glass_border", "Stained glass border", EffectCategory::Filters, "app/palette")
            .desc("Adds a stained glass border with jewel-toned glass panels divided by dark lead came lines.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 20, 300, 60, |e: &mut Self, v| e.border_size = v),
            float_slider::<Self>("panel_width", "Panel width", 10.0, 120.0, 40.0, |e: &mut Self, v| e.panel_width = v),
            float_slider::<Self>("lead_thickness", "Lead thickness", 1.0, 12.0, 3.5, |e: &mut Self, v| e.lead_thickness = v),
            float_slider::<Self>("glass_brightness", "Glass brightness", 30.0, 100.0, 75.0, |e: &mut Self, v| e.glass_brightness = v),
            bool_param::<Self>("glow", "Glass glow", true, |e: &mut Self, v| e.glow = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| e.inner_line = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(20, 300);
        let panel_w = clampf(self.panel_width, 10.0, 120.0);
        let lead_t = clampf(self.lead_thickness, 1.0, 12.0);
        let brightness = clampf(self.glass_brightness, 30.0, 100.0);
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let lead = SkColor::rgb(14, 10, 8);
        let lf = solid(lead, false);
        let (bf, nwf, nhf) = (border as f32, nw as f32, nh as f32);
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, nwf, bf, &lf, None);
        sk::draw_rect_xywh(&mut canvas, 0.0, (nh - border) as f32, nwf, bf, &lf, None);
        sk::draw_rect_xywh(&mut canvas, 0.0, bf, bf, ih as f32, &lf, None);
        sk::draw_rect_xywh(&mut canvas, (nw - border) as f32, bf, bf, ih as f32, &lf, None);
        let glow = self.glow;
        glass_panels(&mut canvas, 0.0, 0.0, nwf, bf, true, panel_w, lead_t, brightness, 0x1A, glow);
        glass_panels(&mut canvas, 0.0, (nh - border) as f32, nwf, bf, true, panel_w, lead_t, brightness, 0x2B, glow);
        glass_panels(&mut canvas, 0.0, bf, bf, ih as f32, false, panel_w, lead_t, brightness, 0x3C, glow);
        glass_panels(&mut canvas, (nw - border) as f32, bf, bf, ih as f32, false, panel_w, lead_t, brightness, 0x4D, glow);
        canvas.draw_bitmap(source, bf, bf, None);
        if self.inner_line {
            sk::draw_rect_xywh(&mut canvas, bf, bf, iw as f32, ih as f32, &lf, Some(&Pen::new(lead_t * 1.5)));
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

const JEWEL_HUES: [f32; 9] = [0.0, 30.0, 60.0, 120.0, 180.0, 210.0, 270.0, 300.0, 340.0];

#[allow(clippy::too_many_arguments)]
fn glass_panels(
    canvas: &mut Canvas,
    rx: f32,
    ry: f32,
    rw: f32,
    rh: f32,
    horizontal: bool,
    panel_w: f32,
    lead_t: f32,
    brightness: f32,
    seed: i32,
    glow: bool,
) {
    let primary = if horizontal { rw } else { rh };
    let cross = if horizontal { rh } else { rw };
    let rows = 1f32.max(cross / panel_w);
    let panel_h = cross / rows;
    sk::with_clip_rect(canvas, RRect::ltrb(rx, ry, rx + rw, ry + rh), |canvas| {
        let mut index = 0i32;
        let mut cr = 0f32;
        while cr < rows {
            let cs = cr * panel_h;
            let ce = cs + panel_h;
            let mut along = 0f32;
            while along < primary {
                let wv = hash01(index, seed, seed ^ 0x7F);
                let len = panel_w * (0.65 + wv * 0.75);
                let end = (along + len).min(primary);
                let hh = hash01(index ^ index.wrapping_mul(7), seed ^ 0x33, seed);
                let hue = JEWEL_HUES[((hh * 9.0) as i32 % 9) as usize];
                let sat = 70.0 + hash01(index, seed ^ 0x55, seed) * 30.0;
                let h = lead_t * 0.5;
                let (x1, y1, x2, y2) = if horizontal {
                    (rx + along + h, ry + cs + h, rx + end - h, ry + ce - h)
                } else {
                    (rx + cs + h, ry + along + h, rx + ce - h, ry + end - h)
                };
                if x2 > x1 + 1.0 && y2 > y1 + 1.0 {
                    glass_panel(canvas, x1, y1, x2, y2, SkColor::from_hsv(hue, sat, brightness, 255), lead_t, glow);
                }
                along = end;
                index += 1;
            }
            cr += 1.0;
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn glass_panel(canvas: &mut Canvas, x1: f32, y1: f32, x2: f32, y2: f32, base: SkColor, lead_t: f32, glow: bool) {
    let (w, h) = (x2 - x1, y2 - y1);
    if w < 1.0 || h < 1.0 {
        return;
    }
    if glow {
        sk::draw_rect_blur(canvas, RRect::xywh(x1, y1, w, h), &solid(base.with_alpha(60), true), None, lead_t * 1.8);
    }
    let light = |v: u8| 255.min(v as i32 + (0.25f32 * 255.0) as i32) as u8;
    let dark = |v: u8| 0.max(v as i32 - (0.15f32 * 255.0) as i32) as u8;
    let lc = SkColor::rgba(light(base.r), light(base.g), light(base.b), 245);
    let dc = SkColor::rgba(dark(base.r), dark(base.g), dark(base.b), 235);
    let gp = sk::linear_gradient((x1, y1), (x2, y2), &[lc, dc], None, true);
    sk::draw_rect_xywh(canvas, x1, y1, w, h, &gp, None);
    let sp = sk::radial_gradient(
        (x1 + w * 0.2, y1 + h * 0.2),
        w.min(h) * 0.4,
        &[SkColor::rgba(255, 255, 255, 90), color::TRANSPARENT],
        None,
        true,
    );
    sk::draw_rect_xywh(canvas, x1, y1, w, h, &sp, None);
}
