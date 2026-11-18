//! `SymmetryImageEffect` (`Core/ImageEffects/Manipulations/SymmetryImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymmetryDirection {
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
}

/// `SymmetryImageEffect`.
#[derive(Clone, Debug)]
pub struct SymmetryImageEffect {
    pub direction: SymmetryDirection,
}

impl Default for SymmetryImageEffect {
    fn default() -> Self {
        SymmetryImageEffect {
            direction: SymmetryDirection::LeftToRight,
        }
    }
}

impl EffectImpl for SymmetryImageEffect {
    fn info() -> Info {
        Info::new(
            "symmetry",
            "Symmetry",
            EffectCategory::Manipulations,
            "image-effect/flip-horizontal",
        )
        .desc("Creates bilateral symmetry by mirroring one half of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![enum_param::<Self, SymmetryDirection>(
            "direction",
            "Direction",
            SymmetryDirection::LeftToRight,
            |e: &mut Self, v| e.direction = v,
            &[
                ("Left → Right", SymmetryDirection::LeftToRight),
                ("Right → Left", SymmetryDirection::RightToLeft),
                ("Top → Bottom", SymmetryDirection::TopToBottom),
                ("Bottom → Top", SymmetryDirection::BottomToTop),
            ],
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let mut out = source.clone();
        let (w, h) = (source.width() as f32, source.height() as f32);
        {
            let mut canvas = Canvas::new(&mut out);
            let (src_rect, sx, sy, px, py) = match self.direction {
                SymmetryDirection::LeftToRight => {
                    (RRect::ltrb(0.0, 0.0, w / 2.0, h), -1.0, 1.0, w / 2.0, 0.0)
                }
                SymmetryDirection::RightToLeft => {
                    (RRect::ltrb(w / 2.0, 0.0, w, h), -1.0, 1.0, w / 2.0, 0.0)
                }
                SymmetryDirection::TopToBottom => {
                    (RRect::ltrb(0.0, 0.0, w, h / 2.0), 1.0, -1.0, 0.0, h / 2.0)
                }
                SymmetryDirection::BottomToTop => {
                    (RRect::ltrb(0.0, h / 2.0, w, h), 1.0, -1.0, 0.0, h / 2.0)
                }
            };
            scale_about(&mut canvas, sx, sy, px, py);
            canvas.draw_image_rect(source, Some(src_rect), src_rect, Sampling::Nearest, None);
        }
        Ok(out)
    }
}
