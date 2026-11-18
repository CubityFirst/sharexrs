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
        let energy = clampf(self.energy, 0.0, 100.0) / 100.0;
        let density = clampf(self.arc_density, 0.0, 100.0) / 100.0;
        let glow = clampf(self.glow, 0.0, 100.0) / 100.0;
        let turbulence = clampf(self.turbulence, 0.0, 100.0) / 100.0;
        let thickness = clampf(self.thickness, 0.0, 100.0) / 100.0;
        if energy <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let inv_w = 1.0 / 1.max(width - 1) as f32;
        let inv_h = 1.0 / 1.max(height - 1) as f32;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let line_width = 0.020 + ((1.0 - thickness) * 0.10);
        let dfreq = 16.0 + (density * 18.0);
        let tfreq = 2.0 + (turbulence * 6.0);
        let l01 = |c: SkColor| ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0;
        let p = |x: i32, y: i32| src[(y * width + x) as usize];
        let scr = |s: f32, o: f32| 1.0 - ((1.0 - s) * (1.0 - clamp01(o)));
        for y in 0..height {
            let row = y * width;
            let v = y as f32 * inv_h;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let (sr, sg, sb) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let u = x as f32 * inv_w;
                let fa = ((u * dfreq) + ((v * tfreq * 2.4) + 0.7).sin() * (1.8 + turbulence * 4.5) + 0.8).sin();
                let fb = ((v * (dfreq * 0.92)) - ((u * tfreq * 1.6) - 1.1).sin() * (1.4 + turbulence * 4.2) - 1.3).cos();
                let fc = (((u + v) * (10.0 + density * 12.0)) + 2.1).sin();
                let core_f = (fa * 0.46) + (fb * 0.34) + (fc * 0.20);
                let branch_f = (fa * 0.28) - (fb * 0.44) + (fc * 0.28);
                let core = 1.0 - smooth_step(line_width, line_width + 0.08, core_f.abs());
                let branch = 1.0 - smooth_step(line_width * 0.70, line_width * 0.70 + 0.09, branch_f.abs());
                let mut arc = core.max(branch * 0.72);
                let edge = clamp01(
                    ((l01(p((width - 1).min(x + 1), y)) - l01(p(0.max(x - 1), y))).abs()
                        + (l01(p(x, (height - 1).min(y + 1))) - l01(p(x, 0.max(y - 1)))).abs())
                        * 1.1,
                );
                arc *= 0.72 + (edge * 0.28);
                let gm = arc.max(0.0).powf(0.34) * glow;
                let cm = arc * energy;
                let or = (cm * 0.20) + (gm * 0.10);
                let og = (cm * 0.70) + (gm * 0.26);
                let ob = (cm * 1.00) + (gm * 0.34);
                let r = lerp(scr(sr, or), 1.0, cm * 0.18);
                let g = lerp(scr(sg, og), 1.0, cm * 0.22);
                let b = lerp(scr(sb, ob), 1.0, cm * 0.28);
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
