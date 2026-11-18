//! `Core/ImageEffects/Drawings`.

use super::{EffectDefinition, def};

pub(crate) mod confetti;
pub(crate) mod draw_background;
pub(crate) mod draw_background_image;
pub(crate) mod draw_checkerboard;
pub(crate) mod draw_image;
pub(crate) mod draw_line;
pub(crate) mod draw_particles;
pub(crate) mod draw_shape;
pub(crate) mod draw_text;
pub(crate) mod enums;
pub(crate) mod fake_cursor;
pub(crate) mod gradient_overlay;
pub(crate) mod helpers;
pub(crate) mod snowflakes;
pub(crate) mod text_watermark;

pub(crate) fn register(list: &mut Vec<EffectDefinition>) {
    list.push(def::<confetti::ConfettiImageEffect>());
    list.push(def::<draw_background::DrawBackgroundEffect>());
    list.push(def::<draw_background_image::DrawBackgroundImageEffect>());
    list.push(def::<draw_checkerboard::DrawCheckerboardEffect>());
    list.push(def::<draw_image::DrawImageEffect>());
    list.push(def::<draw_line::DrawLineEffect>());
    list.push(def::<draw_particles::DrawParticlesEffect>());
    list.push(def::<draw_shape::DrawShapeEffect>());
    list.push(def::<draw_text::DrawTextEffect>());
    list.push(def::<fake_cursor::FakeCursorImageEffect>());
    list.push(def::<gradient_overlay::GradientOverlayImageEffect>());
    list.push(def::<snowflakes::SnowflakesImageEffect>());
    list.push(def::<text_watermark::TextWatermarkEffect>());
}
