//! `FilmStripFrameImageEffect` (`Core/ImageEffects/Filters/FilmStripFrameImageEffect.cs`).

use crate::effects::prelude::*;

/// `FilmStripFrameImageEffect`.
#[derive(Clone, Debug)]
pub struct FilmStripFrameImageEffect {
    pub border_size: i32,
    pub hole_size: f32,
    pub frame_color: SkColor,
    pub side_strips: bool,
    pub scratches: bool,
}

impl Default for FilmStripFrameImageEffect {
    fn default() -> Self {
        FilmStripFrameImageEffect {
            border_size: 40,
            hole_size: 55.0,
            frame_color: SkColor::argb(0xFF0F0F0F),
            side_strips: false,
            scratches: true,
        }
    }
}

impl EffectImpl for FilmStripFrameImageEffect {
    fn info() -> Info {
        Info::new("film_strip_frame", "Film strip frame", EffectCategory::Filters, "app/film")
            .desc("Wraps the image in a cinema film strip frame with sprocket holes.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("border_size", "Border size", 10, 120, 40, |e: &mut Self, v| e.border_size = v),
            float_slider::<Self>("hole_size", "Hole size", 20.0, 80.0, 55.0, |e: &mut Self, v| e.hole_size = v),
            color_param::<Self>("frame_color", "Frame color", SkColor::argb(0xFF0F0F0F), |e: &mut Self, v| e.frame_color = v),
            bool_param::<Self>("side_strips", "Side strips", false, |e: &mut Self, v| e.side_strips = v),
            bool_param::<Self>("scratches", "Scratches", true, |e: &mut Self, v| e.scratches = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(10, 120);
        let hole_frac = clampf(self.hole_size, 20.0, 80.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let frame = solid(self.frame_color, false);
        let (bf, nwf, nhf) = (border as f32, nw as f32, nh as f32);
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, nwf, bf, &frame, None);
        sk::draw_rect_xywh(&mut canvas, 0.0, (nh - border) as f32, nwf, bf, &frame, None);
        sk::draw_rect_xywh(&mut canvas, 0.0, bf, bf, ih as f32, &frame, None);
        sk::draw_rect_xywh(&mut canvas, (nw - border) as f32, bf, bf, ih as f32, &frame, None);
        canvas.draw_bitmap(source, bf, bf, None);
        if self.scratches {
            let pen = Pen::new(0.7);
            let mut seed: i32 = 7391;
            for _ in 0..10 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let x = ((seed as u32) % (nw as u32)) as f32 / nwf * nwf;
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let alpha = (((seed as u32) % 100) as f32 / 100.0) * 0.35 + 0.07;
                let paint = solid(SkColor::rgba(140, 140, 140, (alpha * 255.0) as u8), false);
                sk::draw_line(&mut canvas, x, 0.0, x, nhf, &pen, &paint);
            }
        }
        let hole_h = bf * hole_frac;
        let hole_w = hole_h;
        let hole_r = hole_h * 0.15;
        let margin_v = (bf - hole_h) / 2.0;
        let step = hole_w * 1.75;
        let mut hole = solid(SkColor::rgb(0, 0, 0), false);
        hole.blend = Blend::Clear;
        let mut x = step * 0.4;
        while x + hole_w <= nwf - step * 0.2 {
            let top_y = margin_v;
            let bot_y = (nh - border) as f32 + margin_v;
            sk::draw_round_rect(&mut canvas, RRect::xywh(x, top_y, hole_w, hole_h), hole_r, hole_r, &hole, None);
            sk::draw_round_rect(&mut canvas, RRect::xywh(x, bot_y, hole_w, hole_h), hole_r, hole_r, &hole, None);
            x += step;
        }
        if self.side_strips {
            let margin_h = (bf - hole_w) / 2.0;
            let mut y = bf + step * 0.4;
            while y + hole_h <= (nh - border) as f32 - step * 0.2 {
                sk::draw_round_rect(&mut canvas, RRect::xywh(margin_h, y, hole_w, hole_h), hole_r, hole_r, &hole, None);
                sk::draw_round_rect(&mut canvas, RRect::xywh((nw - border) as f32 + margin_h, y, hole_w, hole_h), hole_r, hole_r, &hole, None);
                y += step;
            }
        }
        let line = solid(SkColor::rgb(60, 60, 60), true);
        sk::draw_rect_xywh(&mut canvas, bf - 0.6, bf - 0.6, iw as f32 + 1.2, ih as f32 + 1.2, &line, Some(&Pen::new(1.2)));
        drop(canvas);
        Ok(out)
    }
}
