//! `MinecraftGrassImageEffect` (`Core/ImageEffects/Filters/MinecraftGrassImageEffect.cs`).

use crate::effects::prelude::*;

/// `MinecraftGrassImageEffect`.
#[derive(Clone, Debug)]
pub struct MinecraftGrassImageEffect {
    pub block_size: i32,
    pub grass_rows: i32,
    pub dirt_rows: i32,
    pub top_edge: bool,
    pub overlap: i32,
    pub grass_color: SkColor,
    pub dirt_color: SkColor,
}

impl Default for MinecraftGrassImageEffect {
    fn default() -> Self {
        MinecraftGrassImageEffect {
            block_size: 16,
            grass_rows: 2,
            dirt_rows: 4,
            top_edge: false,
            overlap: 1,
            grass_color: SkColor::argb(0xFF59B030),
            dirt_color: SkColor::argb(0xFF866043),
        }
    }
}

impl EffectImpl for MinecraftGrassImageEffect {
    fn info() -> Info {
        Info::new("minecraft_grass", "Minecraft grass", EffectCategory::Filters, "image-effect/grass")
            .desc("Adds a Minecraft-style pixelated grass border to the top or bottom of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("block_size", "Block size", 4, 64, 16, |e: &mut Self, v| e.block_size = v),
            int_slider::<Self>("grass_rows", "Grass rows", 1, 8, 2, |e: &mut Self, v| e.grass_rows = v),
            int_slider::<Self>("dirt_rows", "Dirt rows", 1, 16, 4, |e: &mut Self, v| e.dirt_rows = v),
            bool_param::<Self>("top_edge", "Top edge", false, |e: &mut Self, v| e.top_edge = v),
            int_slider::<Self>("overlap", "Overlap", 0, 8, 1, |e: &mut Self, v| e.overlap = v),
            color_param::<Self>("grass_color", "Grass color", SkColor::argb(0xFF59B030), |e: &mut Self, v| e.grass_color = v),
            color_param::<Self>("dirt_color", "Dirt color", SkColor::argb(0xFF866043), |e: &mut Self, v| e.dirt_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
