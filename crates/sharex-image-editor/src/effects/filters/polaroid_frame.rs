//! `PolaroidFrameImageEffect` (`Core/ImageEffects/Filters/PolaroidFrameImageEffect.cs`).

use crate::effects::prelude::*;

/// `PolaroidFrameImageEffect`.
#[derive(Clone, Debug)]
pub struct PolaroidFrameImageEffect {
    pub border_size: i32,
    pub bottom_size: i32,
    pub rotation: f32,
    pub frame_color: SkColor,
    pub caption: String,
    pub shadow_size: i32,
}

impl Default for PolaroidFrameImageEffect {
    fn default() -> Self {
        PolaroidFrameImageEffect {
            border_size: 30,
            bottom_size: 80,
            rotation: 0.0,
            frame_color: SkColor::argb(0xFFFAF9F5),
            caption: "".to_string(),
            shadow_size: 15,
        }
    }
}

impl EffectImpl for PolaroidFrameImageEffect {
    fn info() -> Info {
        Info::new("polaroid_frame", "Polaroid frame", EffectCategory::Filters, "app/camera")
            .desc("Wraps the image in a Polaroid-style instant photo frame.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("border_size", "Border size", 5, 100, 30, |e: &mut Self, v| e.border_size = v),
            int_slider::<Self>("bottom_size", "Bottom size", 20, 200, 80, |e: &mut Self, v| e.bottom_size = v),
            float_slider::<Self>("rotation", "Rotation", -30.0, 30.0, 0.0, |e: &mut Self, v| e.rotation = v),
            color_param::<Self>("frame_color", "Frame color", SkColor::argb(0xFFFAF9F5), |e: &mut Self, v| e.frame_color = v),
            text_param::<Self>("caption", "Caption", "", |e: &mut Self, v| e.caption = v),
            int_slider::<Self>("shadow_size", "Shadow size", 0, 60, 15, |e: &mut Self, v| e.shadow_size = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(5, 100);
        let bottom = self.bottom_size.clamp(20, 200);
        let shadow = self.shadow_size.clamp(0, 60);
        let rotation = clampf(self.rotation, -30.0, 30.0);
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let pw = sw + border * 2;
        let ph = sh + border + bottom;
        let rad = rotation.abs() * std::f32::consts::PI / 180.0;
        let (cos, sin) = (rad.cos(), rad.sin());
        let rw = (pw as f32 * cos + ph as f32 * sin).ceil() as i32;
        let rh = (pw as f32 * sin + ph as f32 * cos).ceil() as i32;
        let cw = rw.max(pw) + shadow * 2 + 20;
        let ch = rh.max(ph) + shadow * 2 + 20;
        let mut out = Bitmap::new(cw.max(0) as u32, ch.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let (cx, cy) = (cw as f32 / 2.0, ch as f32 / 2.0);
        canvas.translate(cx, cy);
        canvas.rotate(rotation);
        canvas.translate(-pw as f32 / 2.0, -ph as f32 / 2.0);
        let (pwf, phf) = (pw as f32, ph as f32);
        if shadow > 0 {
            sk::draw_rect_blur(&mut canvas, RRect::xywh(0.0, 0.0, pwf, phf), &solid(SkColor::rgba(0, 0, 0, 50), true), None, shadow as f32 / 2.0);
        }
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, pwf, phf, &solid(self.frame_color, true), None);
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, pwf, phf, &solid(SkColor::rgb(200, 198, 190), true), Some(&Pen::new(1.0)));
        let bf = border as f32;
        canvas.draw_bitmap(source, bf, bf, None);
        sk::draw_rect_xywh(&mut canvas, bf, bf, sw as f32, sh as f32, &solid(SkColor::rgba(180, 175, 165, 100), true), Some(&Pen::new(1.0)));
        if !is_blank(&self.caption) {
            let size = 12f32.max(bottom as f32 * 0.3);
            let tf = Typeface::from_family("Segoe Script", 400, false);
            let font = Font::new(tf, size);
            let tx = pwf / 2.0;
            let ty = (sh + border) as f32 + (bottom as f32 / 2.0) + (size / 3.0);
            font.draw(&mut canvas, &self.caption, tx, ty, Align::Center, &solid(SkColor::rgb(60, 55, 50), true));
        }
        drop(canvas);
        Ok(out)
    }
}
