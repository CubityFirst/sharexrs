//! `BorderImageEffect` (`Core/ImageEffects/Filters/BorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `ImageHelpers.BorderType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderType {
    Outside,
    Inside,
}

/// `ImageHelpers.DashStyle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashStyle {
    Solid,
    Dash,
    Dot,
    DashDot,
}

/// `BorderImageEffect`.
#[derive(Clone, Debug)]
pub struct BorderImageEffect {
    pub r#type: BorderType,
    pub size: i32,
    pub dash_style: DashStyle,
    pub color: SkColor,
}

impl Default for BorderImageEffect {
    fn default() -> Self {
        BorderImageEffect {
            r#type: BorderType::Outside,
            size: 5,
            dash_style: DashStyle::Solid,
            color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for BorderImageEffect {
    fn info() -> Info {
        Info::new("border", "Border", EffectCategory::Drawings, "image-effect/frame")
            .desc("Adds a border to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, BorderType>("type", "Type", BorderType::Outside, |e: &mut Self, v| e.r#type = v, &[("Outside", BorderType::Outside), ("Inside", BorderType::Inside)]),
            int_slider::<Self>("size", "Size", 1, 100, 5, |e: &mut Self, v| e.size = v),
            enum_param::<Self, DashStyle>("dash_style", "Dash style", DashStyle::Solid, |e: &mut Self, v| e.dash_style = v, &[("Solid", DashStyle::Solid), ("Dash", DashStyle::Dash), ("Dot", DashStyle::Dot), ("Dash Dot", DashStyle::DashDot)]),
            color_param::<Self>("color", "Color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
