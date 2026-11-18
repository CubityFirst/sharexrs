//! `PlasmaEnergyArcsImageEffect` (`Core/ImageEffects/Filters/PlasmaEnergyArcsImageEffect.cs`).

use crate::effects::prelude::*;

/// `PlasmaEnergyArcsImageEffect`.
#[derive(Clone, Debug)]
pub struct PlasmaEnergyArcsImageEffect {
    pub energy: f32,
    pub arc_density: f32,
    pub glow: f32,
    pub turbulence: f32,
    pub thickness: f32,
}

impl Default for PlasmaEnergyArcsImageEffect {
    fn default() -> Self {
        PlasmaEnergyArcsImageEffect {
            energy: 65.0,
            arc_density: 46.0,
            glow: 70.0,
            turbulence: 52.0,
            thickness: 38.0,
        }
    }
}

impl EffectImpl for PlasmaEnergyArcsImageEffect {
    fn info() -> Info {
        Info::new("plasma_energy_arcs", "Plasma energy arcs", EffectCategory::Filters, "app/zap")
            .desc("Generates plasma-like energy arcs overlaid on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("energy", "Energy", 0.0, 100.0, 65.0, |e: &mut Self, v| e.energy = v),
            float_slider::<Self>("arc_density", "Arc density", 0.0, 100.0, 46.0, |e: &mut Self, v| e.arc_density = v),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 70.0, |e: &mut Self, v| e.glow = v),
            float_slider::<Self>("turbulence", "Turbulence", 0.0, 100.0, 52.0, |e: &mut Self, v| e.turbulence = v),
            float_slider::<Self>("thickness", "Thickness", 0.0, 100.0, 38.0, |e: &mut Self, v| e.thickness = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
