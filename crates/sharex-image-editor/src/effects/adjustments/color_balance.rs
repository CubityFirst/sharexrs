//! `ColorBalanceImageEffect` (`Core/ImageEffects/Adjustments/ColorBalanceImageEffect.cs`).

use crate::effects::prelude::*;

/// `ColorBalanceImageEffect`.
#[derive(Clone, Debug)]
pub struct ColorBalanceImageEffect {
    pub shadow_cyan_red: f32,
    pub shadow_magenta_green: f32,
    pub shadow_yellow_blue: f32,
    pub mid_cyan_red: f32,
    pub mid_magenta_green: f32,
    pub mid_yellow_blue: f32,
    pub highlight_cyan_red: f32,
    pub highlight_magenta_green: f32,
    pub highlight_yellow_blue: f32,
}

impl Default for ColorBalanceImageEffect {
    fn default() -> Self {
        ColorBalanceImageEffect {
            shadow_cyan_red: 0.0,
            shadow_magenta_green: 0.0,
            shadow_yellow_blue: 0.0,
            mid_cyan_red: 0.0,
            mid_magenta_green: 0.0,
            mid_yellow_blue: 0.0,
            highlight_cyan_red: 0.0,
            highlight_magenta_green: 0.0,
            highlight_yellow_blue: 0.0,
        }
    }
}

impl EffectImpl for ColorBalanceImageEffect {
    fn info() -> Info {
        Info::new(
            "color_balance",
            "Color Balance",
            EffectCategory::Adjustments,
            "app/sliders",
        )
        .desc("Adjusts color balance in shadows, midtones, and highlights.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "shadow_cyan_red",
                "Shadows: Cyan-Red",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.shadow_cyan_red = v,
            ),
            float_slider::<Self>(
                "shadow_magenta_green",
                "Shadows: Magenta-Green",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.shadow_magenta_green = v,
            ),
            float_slider::<Self>(
                "shadow_yellow_blue",
                "Shadows: Yellow-Blue",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.shadow_yellow_blue = v,
            ),
            float_slider::<Self>(
                "mid_cyan_red",
                "Midtones: Cyan-Red",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.mid_cyan_red = v,
            ),
            float_slider::<Self>(
                "mid_magenta_green",
                "Midtones: Magenta-Green",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.mid_magenta_green = v,
            ),
            float_slider::<Self>(
                "mid_yellow_blue",
                "Midtones: Yellow-Blue",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.mid_yellow_blue = v,
            ),
            float_slider::<Self>(
                "highlight_cyan_red",
                "Highlights: Cyan-Red",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.highlight_cyan_red = v,
            ),
            float_slider::<Self>(
                "highlight_magenta_green",
                "Highlights: Magenta-Green",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.highlight_magenta_green = v,
            ),
            float_slider::<Self>(
                "highlight_yellow_blue",
                "Highlights: Yellow-Blue",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.highlight_yellow_blue = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let all_zero = [
            self.shadow_cyan_red,
            self.shadow_magenta_green,
            self.shadow_yellow_blue,
            self.mid_cyan_red,
            self.mid_magenta_green,
            self.mid_yellow_blue,
            self.highlight_cyan_red,
            self.highlight_magenta_green,
            self.highlight_yellow_blue,
        ]
        .iter()
        .all(|v| v.abs() < 0.01);
        if all_zero {
            return Ok(source.clone());
        }
        let (s_cr, s_mg, s_yb) = (
            self.shadow_cyan_red / 100.0,
            self.shadow_magenta_green / 100.0,
            self.shadow_yellow_blue / 100.0,
        );
        let (m_cr, m_mg, m_yb) = (
            self.mid_cyan_red / 100.0,
            self.mid_magenta_green / 100.0,
            self.mid_yellow_blue / 100.0,
        );
        let (h_cr, h_mg, h_yb) = (
            self.highlight_cyan_red / 100.0,
            self.highlight_magenta_green / 100.0,
            self.highlight_yellow_blue / 100.0,
        );
        Ok(apply_pixel_operation(source, |c| {
            let r = c.r as f32 / 255.0;
            let g = c.g as f32 / 255.0;
            let b = c.b as f32 / 255.0;
            let lum = 0.299 * r + 0.587 * g + 0.114 * b;
            let shadow_w = clampf(1.0 - (lum * 4.0), 0.0, 1.0);
            let highlight_w = clampf((lum - 0.75) * 4.0, 0.0, 1.0);
            let mut mid_w = 1.0 - shadow_w - highlight_w;
            mid_w = mid_w.max(0.0);
            let r_shift = (s_cr * shadow_w + m_cr * mid_w + h_cr * highlight_w) * 0.5;
            let g_shift = (s_mg * shadow_w + m_mg * mid_w + h_mg * highlight_w) * 0.5;
            let b_shift = (s_yb * shadow_w + m_yb * mid_w + h_yb * highlight_w) * 0.5;
            let r = clampf(r + r_shift, 0.0, 1.0);
            let g = clampf(g + g_shift, 0.0, 1.0);
            let b = clampf(b + b_shift, 0.0, 1.0);
            SkColor::rgba(
                (r * 255.0).round_ties_even() as u8,
                (g * 255.0).round_ties_even() as u8,
                (b * 255.0).round_ties_even() as u8,
                c.a,
            )
        }))
    }
}
