//! `CylinderWrapImageEffect` (`Core/ImageEffects/Manipulations/CylinderWrapImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CylinderWrapOrientation {
    Vertical,
    Horizontal,
}

/// `CylinderWrapImageEffect`.
#[derive(Clone, Debug)]
pub struct CylinderWrapImageEffect {
    pub orientation: CylinderWrapOrientation,
    pub curvature: f32,
    pub edge_shading: f32,
}

impl Default for CylinderWrapImageEffect {
    fn default() -> Self {
        CylinderWrapImageEffect {
            orientation: CylinderWrapOrientation::Vertical,
            curvature: 65.0,
            edge_shading: 35.0,
        }
    }
}

impl EffectImpl for CylinderWrapImageEffect {
    fn info() -> Info {
        Info::new("cylinder_wrap", "Cylinder wrap", EffectCategory::Manipulations, "image-effect/cylinder")
            .desc("Wraps the image around a cylindrical surface.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, CylinderWrapOrientation>("orientation", "Orientation", CylinderWrapOrientation::Vertical, |e: &mut Self, v| e.orientation = v, &[("Vertical", CylinderWrapOrientation::Vertical), ("Horizontal", CylinderWrapOrientation::Horizontal)]),
            float_slider::<Self>("curvature", "Curvature", 0.0, 100.0, 65.0, |e: &mut Self, v| e.curvature = v),
            float_slider::<Self>("edge_shading", "Edge shading", 0.0, 100.0, 35.0, |e: &mut Self, v| e.edge_shading = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
