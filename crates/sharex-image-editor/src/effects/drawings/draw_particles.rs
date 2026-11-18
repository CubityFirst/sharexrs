//! `DrawParticlesEffect` (`Core/ImageEffects/Drawings/DrawParticlesEffect.cs`).

use crate::effects::prelude::*;

/// `DrawParticlesEffect`.
#[derive(Clone, Debug)]
pub struct DrawParticlesEffect {
    /// `Math.Clamp(value, 1, 1000)` when set.
    pub image_count: i32,
    pub image_folder: String,
    pub background: bool,
    pub random_size: bool,
    pub random_size_min: i32,
    pub random_size_max: i32,
    pub random_angle: bool,
    pub random_angle_min: i32,
    pub random_angle_max: i32,
    pub random_opacity: bool,
    pub random_opacity_min: i32,
    pub random_opacity_max: i32,
    pub no_overlap: bool,
    pub no_overlap_offset: i32,
    pub edge_overlap: bool,
}

impl Default for DrawParticlesEffect {
    fn default() -> Self {
        DrawParticlesEffect {
            image_count: 1,
            image_folder: "".to_string(),
            background: false,
            random_size: false,
            random_size_min: 64,
            random_size_max: 128,
            random_angle: false,
            random_angle_min: 0,
            random_angle_max: 360,
            random_opacity: false,
            random_opacity_min: 0,
            random_opacity_max: 100,
            no_overlap: false,
            no_overlap_offset: 0,
            edge_overlap: false,
        }
    }
}

impl EffectImpl for DrawParticlesEffect {
    fn info() -> Info {
        Info::new("draw_particles", "Particles", EffectCategory::Drawings, "app/sparkles")
            .desc("Draws particle images on the source image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            file_param::<Self>("image_folder", "Image folder", "", |e: &mut Self, v| e.image_folder = v),
            int_numeric::<Self>("image_count", "Image count", 1, 1000, 1, |e: &mut Self, v| e.image_count = v.clamp(1, 1000)),
            bool_param::<Self>("background", "Background", false, |e: &mut Self, v| e.background = v),
            bool_param::<Self>("random_size", "Random size", false, |e: &mut Self, v| e.random_size = v),
            int_numeric::<Self>("random_size_min", "Random size min", 1, 10000, 64, |e: &mut Self, v| e.random_size_min = v),
            int_numeric::<Self>("random_size_max", "Random size max", 1, 10000, 128, |e: &mut Self, v| e.random_size_max = v),
            bool_param::<Self>("random_angle", "Random angle", false, |e: &mut Self, v| e.random_angle = v),
            int_numeric::<Self>("random_angle_min", "Random angle min", 0, 360, 0, |e: &mut Self, v| e.random_angle_min = v),
            int_numeric::<Self>("random_angle_max", "Random angle max", 0, 360, 360, |e: &mut Self, v| e.random_angle_max = v),
            bool_param::<Self>("random_opacity", "Random opacity", false, |e: &mut Self, v| e.random_opacity = v),
            int_numeric::<Self>("random_opacity_min", "Random opacity min", 0, 100, 0, |e: &mut Self, v| e.random_opacity_min = v),
            int_numeric::<Self>("random_opacity_max", "Random opacity max", 0, 100, 100, |e: &mut Self, v| e.random_opacity_max = v),
            bool_param::<Self>("no_overlap", "No overlap", false, |e: &mut Self, v| e.no_overlap = v),
            int_numeric::<Self>("no_overlap_offset", "No overlap offset", 0, 1000, 0, |e: &mut Self, v| e.no_overlap_offset = v),
            bool_param::<Self>("edge_overlap", "Edge overlap", false, |e: &mut Self, v| e.edge_overlap = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
