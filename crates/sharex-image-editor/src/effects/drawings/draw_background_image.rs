//! `DrawBackgroundImageEffect` (`Core/ImageEffects/Drawings/DrawBackgroundImageEffect.cs`).

use crate::effects::prelude::*;

/// `DrawBackgroundImageEffect`.
#[derive(Clone, Debug)]
pub struct DrawBackgroundImageEffect {
    pub image_file_path: String,
    pub center: bool,
    pub tile: bool,
}

impl Default for DrawBackgroundImageEffect {
    fn default() -> Self {
        DrawBackgroundImageEffect {
            image_file_path: "".to_string(),
            center: true,
            tile: false,
        }
    }
}

impl EffectImpl for DrawBackgroundImageEffect {
    fn info() -> Info {
        Info::new(
            "draw_background_image",
            "Background image",
            EffectCategory::Drawings,
            "app/image",
        )
        .desc("Draws a background image behind the source image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            file_param::<Self>(
                "image_file_path",
                "Image file path",
                "",
                |e: &mut Self, v| e.image_file_path = v,
            ),
            bool_param::<Self>("center", "Center", true, |e: &mut Self, v| e.center = v),
            bool_param::<Self>("tile", "Tile", false, |e: &mut Self, v| e.tile = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let path = expand_environment_variables(self.image_file_path.trim());
        if is_blank(&path) || !file_exists(&path) {
            return Ok(source.clone());
        }
        let Some(bg) = decode_file(&path) else {
            return Ok(source.clone());
        };
        if bg.width() == 0 || bg.height() == 0 {
            return Ok(source.clone());
        }
        let (rw, rh) = (source.width() as i32, source.height() as i32);
        let (bw, bh) = (bg.width() as i32, bg.height() as i32);
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        if self.tile {
            let (mut tx, mut ty) = (0, 0);
            if self.center {
                tx = (rw - bw) / 2 % bw;
                ty = (rh - bh) / 2 % bh;
            }
            let mut paint = Paint::shader(Shader::Image {
                image: &bg,
                matrix: Matrix::IDENTITY,
                sampling: Sampling::CatmullRom,
                tile_x: TileMode::Repeat,
                tile_y: TileMode::Repeat,
            });
            paint.anti_alias = true;
            if self.center {
                let saved = canvas.matrix;
                canvas.translate(tx as f32, ty as f32);
                sk::draw_rect_xywh(
                    &mut canvas,
                    -tx as f32,
                    -ty as f32,
                    rw as f32,
                    rh as f32,
                    &paint,
                    None,
                );
                canvas.matrix = saved;
            } else {
                sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, rw as f32, rh as f32, &paint, None);
            }
        } else {
            let aspect = bw as f32 / bh as f32;
            let mut width = rw;
            let mut height = (width as f32 / aspect) as i32;
            if height < rh {
                height = rh;
                width = (height as f32 * aspect) as i32;
            }
            let x = if self.center { (rw - width) / 2 } else { 0 };
            let y = if self.center { (rh - height) / 2 } else { 0 };
            let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
            paint.anti_alias = true;
            canvas.draw_image_rect(
                &bg,
                None,
                RRect::ltrb(x as f32, y as f32, (x + width) as f32, (y + height) as f32),
                Sampling::CatmullRom,
                Some(&paint),
            );
        }
        canvas.draw_bitmap(source, 0.0, 0.0, None);
        drop(canvas);
        Ok(out)
    }
}
