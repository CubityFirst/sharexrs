//! Every effect in `ShareX.ImageEffectsLib` (Adjustments/, Filters/, Drawings/,
//! Manipulations/): its properties with ShareX's names, defaults and setter clamps, its
//! summary (`GetSummary`), and `Apply`.

use serde_json::{Map, Value};

use crate::Bitmap;
use crate::dotnet::*;
use crate::ops::{self, Kernel};
use crate::paths;
use crate::random::RandomFast;
use crate::skia::{Blend, Canvas, Matrix, Paint, Rect, Sampling, Shader, TileMode};
use crate::text::Fonts;

/// What an effect needs besides the image.
pub struct Context {
    /// `RandomFast` (Slice, Torn edge, Particles).
    pub random: RandomFast,
    pub fonts: Fonts,
    /// `HelpersOptions.ShareXSpecialFolders` (`%ShareXImageEffects%` and the others),
    /// expanded in file paths.
    pub special_folders: Vec<(String, std::path::PathBuf)>,
    /// `AppDomain.CurrentDomain.BaseDirectory`: what relative paths are relative to.
    pub base_dir: std::path::PathBuf,
}

impl Default for Context {
    fn default() -> Self {
        Context {
            random: RandomFast::new(),
            fonts: Fonts::default(),
            special_folders: Vec::new(),
            base_dir: std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_default(),
        }
    }
}

/// An effect's behaviour.
pub trait Effect {
    /// `ImageEffect.Apply`: `None` where ShareX returns null (which ends the preset).
    fn apply(&self, bmp: Bitmap, ctx: &mut Context) -> Option<Bitmap>;
    /// `GetSummary`, shown after the name in the effect list.
    fn summary(&self) -> Option<String> {
        None
    }
}

macro_rules! effect {
    ($(#[$m:meta])* $name:ident { $( $field:ident : $ty:ty = $default:expr $(=> $norm:expr)? ),* $(,)? }) => {
        $(#[$m])*
        #[allow(non_snake_case)]
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name { $( pub $field: $ty, )* }

        impl Default for $name {
            fn default() -> Self {
                $name { $( $field: $default, )* }
            }
        }

        #[allow(unused_variables, unused_mut)]
        impl $name {
            /// The serialised properties, in ShareX's order.
            pub const PROPERTY_NAMES: &'static [&'static str] = &[$(stringify!($field)),*];

            fn write_props(&self, map: &mut Map<String, Value>) {
                $( map.insert(stringify!($field).to_string(), Prop::to_json(&self.$field)); )*
            }

            /// A property's value as ShareX serialises it.
            pub fn get_property(&self, name: &str) -> Option<Value> {
                $( if name.eq_ignore_ascii_case(stringify!($field)) { return Some(Prop::to_json(&self.$field)); } )*
                None
            }

            /// Sets a property through ShareX's setter (with its clamps); false if the
            /// name is unknown or the value doesn't convert.
            pub fn set_property(&mut self, name: &str, v: &Value) -> bool {
                $( if name.eq_ignore_ascii_case(stringify!($field)) {
                    let Some(x) = <$ty as Prop>::from_json(v) else { return false };
                    self.$field = x;
                    $( self.$field = ($norm)(self.$field.clone()); )?
                    return true;
                } )*
                false
            }
        }
    };
}

fn max_i(min: i32) -> impl Fn(i32) -> i32 {
    move |v| v.max(min)
}

fn lookup(key: &str) -> Option<&'static str> {
    sharex_strings::image_effects_lib::lookup(key)
}

/// `DrawText`/`DrawBorder`/`DrawBackground`'s default four-stop gradient.
fn default_gradient_4() -> Option<GradientInfo> {
    Some(GradientInfo::new(
        GradientMode::Vertical,
        &[
            (Color::rgb(68, 120, 194), 0.0),
            (Color::rgb(13, 58, 122), 50.0),
            (Color::rgb(6, 36, 78), 50.0),
            (Color::rgb(23, 89, 174), 100.0),
        ],
    ))
}

/// `Glow`/`DrawTextEx`'s random default gradient (`RandomFast.Next(0, 2)`).
fn random_gradient(kind: GradientMode, rng: &mut RandomFast) -> Option<GradientInfo> {
    let stops = match rng.next(0, 2) {
        0 => [
            (Color::rgb(0, 187, 138), 0.0),
            (Color::rgb(0, 105, 163), 100.0),
        ],
        1 => [
            (Color::rgb(255, 3, 135), 0.0),
            (Color::rgb(255, 143, 3), 100.0),
        ],
        _ => [
            (Color::rgb(184, 11, 195), 0.0),
            (Color::rgb(98, 54, 255), 100.0),
        ],
    };
    Some(GradientInfo::new(kind, &stops))
}

fn fmt_f32(v: f32) -> String {
    format!("{v}")
}

fn shader_or_gradient(
    use_gradient: bool,
    gradient: &Option<GradientInfo>,
    color: &Color,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Shader<'static> {
    match gradient {
        Some(g) if use_gradient && g.is_valid() => ops::gradient_shader(g, x, y, w, h),
        _ => Shader::Color(color.rgba()),
    }
}

// ───────────────────────────── Adjustments ─────────────────────────────

effect!(
    /// `Adjustments/Alpha.cs`.
    Alpha { Value: f32 = 1.0, Addition: f32 = 0.0 }
);
impl Effect for Alpha {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(
            &ops::cm_alpha(self.Value, self.Addition),
            &bmp,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(format!(
            "{}, {}",
            fmt_f32(self.Value),
            fmt_f32(self.Addition)
        ))
    }
}

effect!(
    /// `Adjustments/BlackWhite.cs`.
    BlackWhite {}
);
impl Effect for BlackWhite {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(&ops::cm_black_white(), &bmp))
    }
}

effect!(
    /// `Adjustments/Brightness.cs`.
    Brightness { Value: f32 = 0.0 }
);
impl Effect for Brightness {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(
            &ops::cm_brightness(self.Value),
            &bmp,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(fmt_f32(self.Value))
    }
}

effect!(
    /// `Adjustments/MatrixColor.cs`.
    MatrixColor {
        Rr: f32 = 1.0, Rg: f32 = 0.0, Rb: f32 = 0.0, Ra: f32 = 0.0, Ro: f32 = 0.0,
        Gr: f32 = 0.0, Gg: f32 = 1.0, Gb: f32 = 0.0, Ga: f32 = 0.0, Go: f32 = 0.0,
        Br: f32 = 0.0, Bg: f32 = 0.0, Bb: f32 = 1.0, Ba: f32 = 0.0, Bo: f32 = 0.0,
        Ar: f32 = 0.0, Ag: f32 = 0.0, Ab: f32 = 0.0, Aa: f32 = 1.0, Ao: f32 = 0.0,
    }
);
impl Effect for MatrixColor {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        let m = [
            [self.Rr, self.Gr, self.Br, self.Ar, 0.0],
            [self.Rg, self.Gg, self.Bg, self.Ag, 0.0],
            [self.Rb, self.Gb, self.Bb, self.Ab, 0.0],
            [self.Ra, self.Ga, self.Ba, self.Aa, 0.0],
            [self.Ro, self.Go, self.Bo, self.Ao, 1.0],
        ];
        Some(ops::apply_color_matrix(&m, &bmp))
    }
}

effect!(
    /// `Adjustments/Colorize.cs`.
    Colorize { Color: Color = Color::named("Red"), Value: f32 = 0.0 }
);
impl Effect for Colorize {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(
            &ops::cm_colorize(&self.Color, self.Value),
            &bmp,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(format!(
            "{}, {}, {}",
            self.Color.r, self.Color.g, self.Color.b
        ))
    }
}

effect!(
    /// `Adjustments/Contrast.cs`.
    Contrast { Value: f32 = 1.0 }
);
impl Effect for Contrast {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(&ops::cm_contrast(self.Value), &bmp))
    }
    fn summary(&self) -> Option<String> {
        Some(fmt_f32(self.Value))
    }
}

effect!(
    /// `Adjustments/Gamma.cs`.
    Gamma { Value: f32 = 1.0 }
);
impl Effect for Gamma {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::change_gamma(&bmp, self.Value))
    }
    fn summary(&self) -> Option<String> {
        Some(fmt_f32(self.Value))
    }
}

effect!(
    /// `Adjustments/Grayscale.cs`.
    Grayscale { Value: f32 = 1.0 }
);
impl Effect for Grayscale {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(
            &ops::cm_grayscale(self.Value),
            &bmp,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(fmt_f32(self.Value))
    }
}

effect!(
    /// `Adjustments/Hue.cs`.
    Hue { Angle: f32 = 0.0 }
);
impl Effect for Hue {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(&ops::cm_hue(self.Angle), &bmp))
    }
    fn summary(&self) -> Option<String> {
        Some(format!("{}°", fmt_f32(self.Angle)))
    }
}

effect!(
    /// `Adjustments/Inverse.cs`.
    Inverse {}
);
impl Effect for Inverse {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(&ops::cm_inverse(), &bmp))
    }
}

effect!(
    /// `Adjustments/Polaroid.cs`.
    Polaroid {}
);
impl Effect for Polaroid {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(&ops::cm_polaroid(), &bmp))
    }
}

effect!(
    /// `Adjustments/ReplaceColor.cs`.
    ReplaceColor {
        SourceColor: Color = Color::named("White"),
        AutoSourceColor: bool = false,
        TargetColor: Color = Color::named("Transparent"),
        Threshold: i32 = 0,
    }
);
impl Effect for ReplaceColor {
    fn apply(&self, mut bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        ops::replace_color(
            &mut bmp,
            &self.SourceColor,
            &self.TargetColor,
            self.AutoSourceColor,
            self.Threshold,
        );
        Some(bmp)
    }
}

effect!(
    /// `Adjustments/Saturation.cs`.
    Saturation { Value: f32 = 1.0 }
);
impl Effect for Saturation {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(
            &ops::cm_saturation(self.Value),
            &bmp,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(fmt_f32(self.Value))
    }
}

effect!(
    /// `Adjustments/SelectiveColor.cs`.
    SelectiveColor { LightColor: Color = Color::named("White"), DarkColor: Color = Color::named("Black"), PaletteSize: i32 = 10 }
);
impl Effect for SelectiveColor {
    fn apply(&self, mut bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        ops::selective_color(
            &mut bmp,
            &self.LightColor,
            &self.DarkColor,
            self.PaletteSize.clamp(2, 100),
        );
        Some(bmp)
    }
}

effect!(
    /// `Adjustments/Sepia.cs`.
    Sepia { Value: f32 = 1.0 }
);
impl Effect for Sepia {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::apply_color_matrix(&ops::cm_sepia(self.Value), &bmp))
    }
    fn summary(&self) -> Option<String> {
        Some(fmt_f32(self.Value))
    }
}

// ───────────────────────────── Filters ─────────────────────────────

effect!(
    /// `Filters/Blur.cs`: at least 3, always odd.
    Blur { Radius: i32 = 15 => |v: i32| { let r = v.max(3); if r % 2 == 0 { r + 1 } else { r } } }
);
impl Effect for Blur {
    fn apply(&self, mut bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        ops::box_blur(&mut bmp, self.Radius);
        Some(bmp)
    }
    fn summary(&self) -> Option<String> {
        Some(self.Radius.to_string())
    }
}

effect!(
    /// `Filters/ColorDepth.cs`.
    ColorDepth { BitsPerChannel: i32 = 4 => |v: i32| v.clamp(1, 8) }
);
impl Effect for ColorDepth {
    fn apply(&self, mut bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        ops::color_depth(&mut bmp, self.BitsPerChannel);
        Some(bmp)
    }
    fn summary(&self) -> Option<String> {
        Some(format!(
            "{} bit{}",
            self.BitsPerChannel,
            if self.BitsPerChannel > 1 { "s" } else { "" }
        ))
    }
}

effect!(
    /// `Filters/MatrixConvolution.cs`.
    MatrixConvolution {
        X0Y0: i32 = 0, X1Y0: i32 = 0, X2Y0: i32 = 0,
        X0Y1: i32 = 0, X1Y1: i32 = 1, X2Y1: i32 = 0,
        X0Y2: i32 = 0, X1Y2: i32 = 0, X2Y2: i32 = 0,
        Factor: f64 = 1.0,
        Offset: u8 = 0,
    }
);
impl Effect for MatrixConvolution {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        let mut k = Kernel::new(3, 3);
        let f = self.Factor;
        let v = [
            [self.X0Y0, self.X1Y0, self.X2Y0],
            [self.X0Y1, self.X1Y1, self.X2Y1],
            [self.X0Y2, self.X1Y2, self.X2Y2],
        ];
        for (y, row) in v.iter().enumerate() {
            for (x, c) in row.iter().enumerate() {
                k.set(y, x, *c as f64 / f);
            }
        }
        k.offset = self.Offset;
        Some(k.apply(&bmp))
    }
}

effect!(
    /// `Filters/EdgeDetect.cs`.
    EdgeDetect {}
);
impl Effect for EdgeDetect {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(Kernel::edge_detect().apply(&bmp))
    }
}

effect!(
    /// `Filters/Emboss.cs`.
    Emboss {}
);
impl Effect for Emboss {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(Kernel::emboss().apply(&bmp))
    }
}

effect!(
    /// `Filters/GaussianBlur.cs`.
    GaussianBlur { Radius: i32 = 15 => max_i(1) }
);
impl Effect for GaussianBlur {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::gaussian_blur(&bmp, self.Radius))
    }
    fn summary(&self) -> Option<String> {
        Some(self.Radius.to_string())
    }
}

effect!(
    /// `Filters/Glow.cs`.
    Glow {
        Size: i32 = 20 => max_i(0),
        Strength: f32 = 1.0 => |v: f32| v.max(0.1),
        Color: Color = Color::named("White"),
        UseGradient: bool = false,
        Gradient: Option<GradientInfo> = random_gradient(GradientMode::ForwardDiagonal, &mut RandomFast::seeded(0)),
        Offset: Point = Point::new(0, 0),
    }
);
impl Effect for Glow {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        let g = if self.UseGradient {
            self.Gradient.as_ref()
        } else {
            None
        };
        Some(ops::add_glow(
            &bmp,
            self.Size,
            self.Strength,
            &self.Color,
            self.Offset,
            g,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(self.Size.to_string())
    }
}

effect!(
    /// `Filters/MeanRemoval.cs`.
    MeanRemoval {}
);
impl Effect for MeanRemoval {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(Kernel::mean_removal().apply(&bmp))
    }
}

effect!(
    /// `Filters/Outline.cs`.
    Outline { Size: i32 = 1 => max_i(1), Padding: i32 = 0 => max_i(0), Color: Color = Color::named("Black"), OutlineOnly: bool = false }
);
impl Effect for Outline {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::outline(
            bmp,
            self.Size,
            &self.Color,
            self.Padding,
            self.OutlineOnly,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(self.Size.to_string())
    }
}

effect!(
    /// `Filters/Pixelate.cs`.
    Pixelate { Size: i32 = 10 => max_i(2), BorderSize: i32 = 0 => max_i(0), BorderColor: Color = Color::named("Black") }
);
impl Effect for Pixelate {
    fn apply(&self, mut bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        ops::pixelate_border(&mut bmp, self.Size, self.BorderSize, &self.BorderColor);
        Some(bmp)
    }
    fn summary(&self) -> Option<String> {
        Some(self.Size.to_string())
    }
}

effect!(
    /// `Filters/Reflection.cs`.
    Reflection {
        Percentage: i32 = 20 => |v: i32| v.clamp(1, 100),
        MaxAlpha: i32 = 255 => |v: i32| v.clamp(0, 255),
        MinAlpha: i32 = 0 => |v: i32| v.clamp(0, 255),
        Offset: i32 = 0,
        Skew: bool = false,
        SkewSize: i32 = 25,
    }
);
impl Effect for Reflection {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::draw_reflection(
            &bmp,
            self.Percentage,
            self.MaxAlpha,
            self.MinAlpha,
            self.Offset,
            self.Skew,
            self.SkewSize,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(self.Percentage.to_string())
    }
}

effect!(
    /// `Filters/RGBSplit.cs`.
    RGBSplit { OffsetRed: Point = Point::new(-5, 0), OffsetGreen: Point = Point::new(0, 0), OffsetBlue: Point = Point::new(5, 0) }
);
impl Effect for RGBSplit {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        let src = crate::pixels::PixelBuffer::read(&bmp);
        let mut dest = crate::pixels::PixelBuffer::blank(src.width, src.height);
        let (right, bottom) = (src.width as i32 - 1, src.height as i32 - 1);
        let at = |x: i32, y: i32, o: Point| {
            src.get(
                (x - o.x).clamp(0, right) as usize,
                (y - o.y).clamp(0, bottom) as usize,
            )
        };
        for y in 0..src.height as i32 {
            for x in 0..src.width as i32 {
                let r = at(x, y, self.OffsetRed);
                let g = at(x, y, self.OffsetGreen);
                let b = at(x, y, self.OffsetBlue);
                let c = [
                    (r[0] as i32 * r[3] as i32 / 255) as u8,
                    (g[1] as i32 * g[3] as i32 / 255) as u8,
                    (b[2] as i32 * b[3] as i32 / 255) as u8,
                    ((r[3] as i32 + g[3] as i32 + b[3] as i32) / 3) as u8,
                ];
                dest.set(x as usize, y as usize, c);
            }
        }
        Some(dest.into_bitmap())
    }
}

effect!(
    /// `Filters/Shadow.cs`.
    Shadow {
        Opacity: f32 = 0.6 => |v: f32| v.clamp(0.1, 1.0),
        Size: i32 = 10 => max_i(0),
        Darkness: f32 = 0.0,
        Color: Color = Color::named("Black"),
        Offset: Point = Point::new(0, 0),
        AutoResize: bool = true,
    }
);
impl Effect for Shadow {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::add_shadow(
            &bmp,
            self.Opacity,
            self.Size,
            self.Darkness + 1.0,
            &self.Color,
            self.Offset,
            self.AutoResize,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(self.Size.to_string())
    }
}

effect!(
    /// `Filters/Sharpen.cs`.
    Sharpen {}
);
impl Effect for Sharpen {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(Kernel::sharpen().apply(&bmp))
    }
}

effect!(
    /// `Filters/Slice.cs`.
    Slice {
        MinSliceHeight: i32 = 10 => max_i(1),
        MaxSliceHeight: i32 = 100 => max_i(1),
        MinSliceShift: i32 = 0,
        MaxSliceShift: i32 = 10,
    }
);
impl Effect for Slice {
    fn apply(&self, bmp: Bitmap, ctx: &mut Context) -> Option<Bitmap> {
        let (min_h, max_h) = (
            self.MinSliceHeight.min(self.MaxSliceHeight),
            self.MinSliceHeight.max(self.MaxSliceHeight),
        );
        let (min_s, max_s) = (
            self.MinSliceShift.min(self.MaxSliceShift),
            self.MinSliceShift.max(self.MaxSliceShift),
        );
        Some(ops::slice(
            &bmp,
            min_h,
            max_h,
            min_s,
            max_s,
            &mut ctx.random,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(format!("{}, {}", self.MinSliceHeight, self.MaxSliceHeight))
    }
}

effect!(
    /// `Filters/Smooth.cs`.
    Smooth {}
);
impl Effect for Smooth {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(Kernel::smooth().apply(&bmp))
    }
}

effect!(
    /// `Filters/TornEdge.cs`.
    TornEdge { Depth: i32 = 15, Range: i32 = 20, Sides: Sides = Sides::ALL, CurvedEdges: bool = true }
);
impl Effect for TornEdge {
    fn apply(&self, bmp: Bitmap, ctx: &mut Context) -> Option<Bitmap> {
        Some(ops::torn_edges(
            bmp,
            self.Depth,
            self.Range,
            self.Sides,
            self.CurvedEdges,
            &mut ctx.random,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(format!("{}, {}", self.Depth, self.Range))
    }
}

effect!(
    /// `Filters/WaveEdge.cs`.
    WaveEdge { Depth: i32 = 15, Range: i32 = 20, Sides: Sides = Sides::ALL }
);
impl Effect for WaveEdge {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::wavy_edges(bmp, self.Depth, self.Range, self.Sides))
    }
    fn summary(&self) -> Option<String> {
        Some(format!("{}, {}", self.Depth, self.Range))
    }
}

// ───────────────────────────── Manipulations ─────────────────────────────

effect!(
    /// `Manipulations/AutoCrop.cs`.
    AutoCrop { Sides: Sides = Sides::ALL, Padding: i32 = 0 }
);
impl Effect for AutoCrop {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::auto_crop(bmp, true, self.Sides, self.Padding))
    }
    fn summary(&self) -> Option<String> {
        (self.Padding > 0).then(|| self.Padding.to_string())
    }
}

effect!(
    /// `Manipulations/Canvas.cs`.
    Canvas_ { Margin: Margins = Margins::all(0), MarginMode: CanvasMarginMode = CanvasMarginMode::AbsoluteSize, Color: Color = Color::named("Transparent") }
);
/// `Canvas` (renamed: `Canvas` is Skia's canvas here).
pub type CanvasEffect = Canvas_;
impl Effect for Canvas_ {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        let m = if self.MarginMode == CanvasMarginMode::PercentageOfCanvas {
            let pct = |v: i32, size: u32| (v as f32 / 100.0 * size as f32).round_ties_even() as i32;
            Margins::new(
                pct(self.Margin.left, bmp.width()),
                pct(self.Margin.top, bmp.height()),
                pct(self.Margin.right, bmp.width()),
                pct(self.Margin.bottom, bmp.height()),
            )
        } else {
            self.Margin
        };
        Some(ops::add_canvas(&bmp, m, &self.Color).unwrap_or(bmp))
    }
    fn summary(&self) -> Option<String> {
        let m = &self.Margin;
        Some(if m.all_value() == -1 {
            format!("{}, {}, {}, {}", m.left, m.top, m.right, m.bottom)
        } else {
            m.all_value().to_string()
        })
    }
}

effect!(
    /// `Manipulations/Crop.cs`: negative margins are ignored by the setter.
    Crop { Margin: Margins = Margins::all(0) }
);
impl Crop {
    fn normalized(old: Margins, new: Margins) -> Margins {
        if new.top >= 0 && new.right >= 0 && new.bottom >= 0 && new.left >= 0 {
            new
        } else {
            old
        }
    }
}
impl Effect for Crop {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        let m = self.Margin;
        if m.all_value() == 0 {
            return Some(bmp);
        }
        ops::crop_bitmap(
            &bmp,
            m.left,
            m.top,
            bmp.width() as i32 - m.horizontal(),
            bmp.height() as i32 - m.vertical(),
        )
    }
    fn summary(&self) -> Option<String> {
        let m = &self.Margin;
        Some(if m.all_value() == -1 {
            format!("{}, {}, {}, {}", m.left, m.top, m.right, m.bottom)
        } else {
            m.all_value().to_string()
        })
    }
}

effect!(
    /// `Manipulations/Flip.cs`.
    Flip { Horizontally: bool = false, Vertically: bool = false }
);
impl Effect for Flip {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        if self.Horizontally || self.Vertically {
            Some(ops::flip(&bmp, self.Horizontally, self.Vertically))
        } else {
            Some(bmp)
        }
    }
    fn summary(&self) -> Option<String> {
        let b = |v: bool| if v { "True" } else { "False" };
        Some(format!("{}, {}", b(self.Horizontally), b(self.Vertically)))
    }
}

effect!(
    /// `Manipulations/ForceProportions.cs`.
    ForceProportions {
        ProportionalWidth: i32 = 1 => max_i(1),
        ProportionalHeight: i32 = 1 => max_i(1),
        Method: ForceProportionsMethod = ForceProportionsMethod::Grow,
        GrowFillColor: Color = Color::named("Transparent"),
    }
);
impl Effect for ForceProportions {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        let (w, h) = (bmp.width() as i32, bmp.height() as i32);
        let current = w as f32 / h as f32;
        let target = self.ProportionalWidth as f32 / self.ProportionalHeight as f32;
        let wider = target > current;
        let (mut tw, mut th) = (w, h);
        match self.Method {
            ForceProportionsMethod::Crop => {
                let (mut ml, mut mt) = (0, 0);
                if wider {
                    th = (w as f32 / target).round_ties_even() as i32;
                    mt = (h - th) / 2;
                } else {
                    tw = (h as f32 * target).round_ties_even() as i32;
                    ml = (w - tw) / 2;
                }
                ops::crop_bitmap(&bmp, ml, mt, tw, th)
            }
            ForceProportionsMethod::Grow => {
                if wider {
                    tw = (h as f32 * target).round_ties_even() as i32;
                } else {
                    th = (w as f32 / target).round_ties_even() as i32;
                }
                Some(ops::resize_image_fit(
                    &bmp,
                    tw,
                    th,
                    false,
                    true,
                    &self.GrowFillColor,
                ))
            }
        }
    }
    fn summary(&self) -> Option<String> {
        Some(format!(
            "{}, {}",
            self.ProportionalWidth, self.ProportionalHeight
        ))
    }
}

effect!(
    /// `Manipulations/Resize.cs`.
    Resize { Width: i32 = 250, Height: i32 = 0, Mode: ResizeMode = ResizeMode::ResizeAll }
);
impl Effect for Resize {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        if self.Width <= 0 && self.Height <= 0 {
            return Some(bmp);
        }
        let (w, h) = ops::apply_aspect_ratio(self.Width, self.Height, bmp.width(), bmp.height());
        let (bw, bh) = (bmp.width() as i32, bmp.height() as i32);
        if (self.Mode == ResizeMode::ResizeIfBigger && bw <= w && bh <= h)
            || (self.Mode == ResizeMode::ResizeIfSmaller && bw >= w && bh >= h)
        {
            return Some(bmp);
        }
        Some(ops::resize_image(bmp, w, h))
    }
    fn summary(&self) -> Option<String> {
        let px = |v: i32| {
            if v > 0 {
                format!("{v}px")
            } else {
                v.to_string()
            }
        };
        Some(format!("{}, {}", px(self.Width), px(self.Height)))
    }
}

effect!(
    /// `Manipulations/Rotate.cs`.
    Rotate { Angle: f32 = 0.0, Upsize: bool = true, Clip: bool = false }
);
impl Effect for Rotate {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        if self.Angle == 0.0 {
            return Some(bmp);
        }
        Some(ops::rotate_image(&bmp, self.Angle, self.Upsize, self.Clip))
    }
    fn summary(&self) -> Option<String> {
        Some(format!("{}°", fmt_f32(self.Angle)))
    }
}

effect!(
    /// `Manipulations/RoundedCorners.cs`.
    RoundedCorners { CornerRadius: i32 = 20 => max_i(0) }
);
impl Effect for RoundedCorners {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::rounded_corners(&bmp, self.CornerRadius))
    }
    fn summary(&self) -> Option<String> {
        Some(self.CornerRadius.to_string())
    }
}

effect!(
    /// `Manipulations/Scale.cs`.
    Scale { WidthPercentage: f32 = 100.0, HeightPercentage: f32 = 0.0 }
);
impl Effect for Scale {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        if self.WidthPercentage <= 0.0 && self.HeightPercentage <= 0.0 {
            return Some(bmp);
        }
        let w = (self.WidthPercentage / 100.0 * bmp.width() as f32).round_ties_even() as i32;
        let h = (self.HeightPercentage / 100.0 * bmp.height() as f32).round_ties_even() as i32;
        let (w, h) = ops::apply_aspect_ratio(w, h, bmp.width(), bmp.height());
        Some(ops::resize_image(bmp, w, h))
    }
    fn summary(&self) -> Option<String> {
        let pct = |v: f32| {
            if v > 0.0 {
                format!("{}%", fmt_f32(v))
            } else {
                fmt_f32(v)
            }
        };
        Some(format!(
            "{}, {}",
            pct(self.WidthPercentage),
            pct(self.HeightPercentage)
        ))
    }
}

effect!(
    /// `Manipulations/Skew.cs`.
    Skew { Horizontally: i32 = 0, Vertically: i32 = 0 }
);
impl Effect for Skew {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        if self.Horizontally == 0 && self.Vertically == 0 {
            return Some(bmp);
        }
        Some(ops::add_skew(&bmp, self.Horizontally, self.Vertically))
    }
    fn summary(&self) -> Option<String> {
        Some(format!("{}px, {}px", self.Horizontally, self.Vertically))
    }
}

// ───────────────────────────── Drawings ─────────────────────────────

effect!(
    /// `Drawings/DrawBackground.cs`.
    DrawBackground { Color: Color = Color::named("Black"), UseGradient: bool = false, Gradient: Option<GradientInfo> = default_gradient_4() }
);
impl Effect for DrawBackground {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        match &self.Gradient {
            Some(g) if self.UseGradient && g.is_valid() => {
                let s = ops::gradient_shader(g, 0, 0, bmp.width() as i32, bmp.height() as i32);
                Some(ops::fill_background(&bmp, s))
            }
            _ => Some(ops::fill_background_color(&bmp, &self.Color)),
        }
    }
    fn summary(&self) -> Option<String> {
        (!self.UseGradient).then(|| format!("{}, {}, {}", self.Color.r, self.Color.g, self.Color.b))
    }
}

effect!(
    /// `Drawings/DrawBackgroundImage.cs`.
    DrawBackgroundImage { ImageFilePath: String = String::new(), Center: bool = true, Tile: bool = false }
);
impl Effect for DrawBackgroundImage {
    fn apply(&self, bmp: Bitmap, ctx: &mut Context) -> Option<Bitmap> {
        let Some(path) = paths::safe_local_path(&self.ImageFilePath, false, ctx) else {
            return Some(bmp);
        };
        match crate::bitmap::load(&path) {
            Some(bg) if !bg.is_empty() => Some(ops::draw_background_image(
                &bmp,
                &bg,
                self.Center,
                self.Tile,
            )),
            _ => Some(bmp),
        }
    }
    fn summary(&self) -> Option<String> {
        paths::file_name(&self.ImageFilePath)
    }
}

effect!(
    /// `Drawings/DrawBorder.cs`.
    DrawBorder {
        Type: BorderType = BorderType::Outside,
        Size: i32 = 1 => max_i(1),
        DashStyle: DashStyle = DashStyle::Solid,
        Color: Color = Color::named("Black"),
        UseGradient: bool = false,
        Gradient: Option<GradientInfo> = default_gradient_4(),
    }
);
impl Effect for DrawBorder {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        let outside = self.Type == BorderType::Outside;
        let shader = match &self.Gradient {
            Some(g) if self.UseGradient && g.is_valid() => {
                let (mut w, mut h) = (bmp.width() as i32, bmp.height() as i32);
                if outside {
                    w += self.Size * 2;
                    h += self.Size * 2;
                }
                ops::gradient_shader(g, 0, 0, w, h)
            }
            _ => Shader::Color(self.Color.rgba()),
        };
        Some(ops::draw_border(
            bmp,
            shader,
            self.Size,
            outside,
            self.DashStyle,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(format!("{}px", self.Size))
    }
}

effect!(
    /// `Drawings/DrawCheckerboard.cs`.
    DrawCheckerboard { Size: i32 = 10 => max_i(1), Color: Color = Color::named("LightGray"), Color2: Color = Color::named("White") }
);
impl Effect for DrawCheckerboard {
    fn apply(&self, bmp: Bitmap, _: &mut Context) -> Option<Bitmap> {
        Some(ops::draw_checkers(
            &bmp,
            self.Size,
            &self.Color,
            &self.Color2,
        ))
    }
    fn summary(&self) -> Option<String> {
        Some(format!("{0}x{0}", self.Size))
    }
}

effect!(
    /// `Drawings/DrawImage.cs`.
    DrawImage {
        ImageLocation: String = String::new(),
        Placement: ContentAlignment = ContentAlignment::TopLeft,
        Offset: Point = Point::new(0, 0),
        SizeMode: DrawImageSizeMode = DrawImageSizeMode::DontResize,
        Size: Size = Size::new(0, 0),
        RotateFlip: RotateFlipType = RotateFlipType::None,
        Tile: bool = false,
        AutoHide: bool = false,
        InterpolationMode: InterpolationMode = InterpolationMode::HighQualityBicubic,
        CompositingMode: CompositingMode = CompositingMode::SourceOver,
        Opacity: i32 = 100 => |v: i32| v.clamp(0, 100),
    }
);
impl Effect for DrawImage {
    fn apply(&self, mut bmp: Bitmap, ctx: &mut Context) -> Option<Bitmap> {
        if self.Opacity < 1
            || (self.SizeMode != DrawImageSizeMode::DontResize
                && self.Size.width <= 0
                && self.Size.height <= 0)
        {
            return Some(bmp);
        }
        let Some(path) =
            paths::safe_local_path(&self.ImageLocation, false, ctx).filter(|p| p.is_file())
        else {
            return Some(bmp);
        };
        let Some(mut wm) = crate::bitmap::load(&path) else {
            return Some(bmp);
        };
        if self.RotateFlip != RotateFlipType::None {
            wm = ops::rotate_flip(&wm, self.RotateFlip.value());
        }
        let (bw, bh) = (bmp.width() as i32, bmp.height() as i32);
        let (ww, wh) = (wm.width(), wm.height());
        let pct = |v: i32, size: i32| (v as f32 / 100.0 * size as f32).round_ties_even() as i32;
        let (iw, ih) = match self.SizeMode {
            DrawImageSizeMode::AbsoluteSize => {
                let w = if self.Size.width == -1 {
                    bw
                } else {
                    self.Size.width
                };
                let h = if self.Size.height == -1 {
                    bh
                } else {
                    self.Size.height
                };
                ops::apply_aspect_ratio(w, h, ww, wh)
            }
            DrawImageSizeMode::PercentageOfWatermark => ops::apply_aspect_ratio(
                pct(self.Size.width, ww as i32),
                pct(self.Size.height, wh as i32),
                ww,
                wh,
            ),
            DrawImageSizeMode::PercentageOfCanvas => {
                ops::apply_aspect_ratio(pct(self.Size.width, bw), pct(self.Size.height, bh), ww, wh)
            }
            DrawImageSizeMode::DontResize => (ww as i32, wh as i32),
        };
        let pos = ops::get_position(
            self.Placement,
            self.Offset,
            Size::new(bw, bh),
            Size::new(iw, ih),
        );
        if self.AutoHide && !ops::bounds_contain(bmp.width(), bmp.height(), pos.x, pos.y, iw, ih) {
            return Some(bmp);
        }
        let blend = if self.CompositingMode == CompositingMode::SourceCopy {
            Blend::Src
        } else {
            Blend::SrcOver
        };
        let rect = Rect::xywh(pos.x as f32, pos.y as f32, iw as f32, ih as f32);
        let mut g = Canvas::new(&mut bmp);
        if self.Tile {
            let mut tex = Paint::texture(&wm, TileMode::Repeat);
            tex.blend = blend;
            tex.translate_shader(pos.x as f32, pos.y as f32);
            g.draw_rect(rect, &tex);
        } else {
            let mut paint = Paint::shader(Shader::Color([255, 255, 255, 255]));
            paint.blend = blend;
            if self.Opacity < 100 {
                paint.alpha = (self.Opacity as f64 * 2.55).round_ties_even() as u8;
            }
            let sampling = match self.InterpolationMode {
                InterpolationMode::NearestNeighbor => Sampling::Nearest,
                InterpolationMode::Bilinear | InterpolationMode::HighQualityBilinear => {
                    Sampling::Linear
                }
                _ => Sampling::Mitchell,
            };
            g.draw_image_rect(&wm, None, rect, sampling, Some(&paint));
        }
        Some(bmp)
    }
    fn summary(&self) -> Option<String> {
        paths::file_name(&self.ImageLocation)
    }
}

effect!(
    /// `Drawings/DrawParticles.cs`.
    DrawParticles {
        ImageFolder: String = String::new(),
        ImageCount: i32 = 1 => |v: i32| v.clamp(1, 1000),
        Background: bool = false,
        RandomSize: bool = false,
        RandomSizeMin: i32 = 64,
        RandomSizeMax: i32 = 128,
        RandomAngle: bool = false,
        RandomAngleMin: i32 = 0,
        RandomAngleMax: i32 = 360,
        RandomOpacity: bool = false,
        RandomOpacityMin: i32 = 0,
        RandomOpacityMax: i32 = 100,
        NoOverlap: bool = false,
        NoOverlapOffset: i32 = 0,
        EdgeOverlap: bool = false,
    }
);
impl DrawParticles {
    fn draw_from_folder(&self, bmp: &mut Bitmap, ctx: &mut Context) {
        let Some(folder) =
            paths::safe_local_path(&self.ImageFolder, true, ctx).filter(|p| p.is_dir())
        else {
            return;
        };
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&folder)
            .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        files.retain(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| {
                    let e = e.to_string_lossy().to_ascii_lowercase();
                    e == "png" || e == "jpg"
                })
        });
        files.sort();
        if files.is_empty() {
            return;
        }
        let mut cache: std::collections::HashMap<std::path::PathBuf, Option<Bitmap>> =
            Default::default();
        let mut rects: Vec<(i32, i32, i32, i32)> = Vec::new();
        for _ in 0..self.ImageCount {
            let file = ctx.random.pick(&files).clone();
            let img = cache
                .entry(file.clone())
                .or_insert_with(|| crate::bitmap::load(&file))
                .clone();
            if let Some(img) = img {
                self.draw_one(bmp, &img, &mut rects, &mut ctx.random);
            }
        }
    }

    fn draw_one(
        &self,
        bmp: &mut Bitmap,
        img2: &Bitmap,
        rects: &mut Vec<(i32, i32, i32, i32)>,
        rng: &mut RandomFast,
    ) {
        let (mut width, mut height);
        if self.RandomSize {
            let size = rng.next(
                self.RandomSizeMin.min(self.RandomSizeMax),
                self.RandomSizeMin.max(self.RandomSizeMax),
            );
            width = size;
            height = size;
            let (iw, ih) = (img2.width() as f64, img2.height() as f64);
            if iw > ih {
                height = (size as f64 * (ih / iw)).round_ties_even() as i32;
            } else if iw < ih {
                width = (size as f64 * (iw / ih)).round_ties_even() as i32;
            }
        } else {
            width = img2.width() as i32;
            height = img2.height() as i32;
        }
        if width < 1 || height < 1 {
            return;
        }
        let min_x = if self.EdgeOverlap { -width + 1 } else { 0 };
        let min_y = if self.EdgeOverlap { -height + 1 } else { 0 };
        let max_x = bmp.width() as i32 - if self.EdgeOverlap { 0 } else { width } - 1;
        let max_y = bmp.height() as i32 - if self.EdgeOverlap { 0 } else { height } - 1;
        let mut attempts = 0;
        let rect = loop {
            attempts += 1;
            if attempts > 1000 {
                return;
            }
            let x = rng.next(min_x.min(max_x), min_x.max(max_x));
            let y = rng.next(min_y.min(max_y), min_y.max(max_y));
            let r = (x, y, width, height);
            let o = self.NoOverlapOffset;
            let ov = (x - o, y - o, width + o * 2, height + o * 2);
            let intersects = |a: &(i32, i32, i32, i32)| {
                ov.0 < a.0 + a.2 && a.0 < ov.0 + ov.2 && ov.1 < a.1 + a.3 && a.1 < ov.1 + ov.3
            };
            if !(self.NoOverlap && rects.iter().any(intersects)) {
                break r;
            }
        };
        rects.push(rect);
        let mut g = Canvas::new(bmp);
        if self.RandomAngle {
            let mx = rect.0 as f32 + rect.2 as f32 / 2.0;
            let my = rect.1 as f32 + rect.3 as f32 / 2.0;
            let rotate = rng.next(
                self.RandomAngleMin.min(self.RandomAngleMax),
                self.RandomAngleMin.max(self.RandomAngleMax),
            );
            g.translate(mx, my);
            g.rotate(rotate as f32);
            g.translate(-mx, -my);
        }
        let src = Rect::xywh(0.0, 0.0, img2.width() as f32, img2.height() as f32);
        let dst = Rect::xywh(rect.0 as f32, rect.1 as f32, rect.2 as f32, rect.3 as f32);
        if self.RandomOpacity {
            let o = rng
                .next(
                    self.RandomOpacityMin.min(self.RandomOpacityMax),
                    self.RandomOpacityMin.max(self.RandomOpacityMax),
                )
                .clamp(0, 100) as f32
                / 100.0;
            let mut m: ops::ColorMatrix5 = [[0.0; 5]; 5];
            for (i, row) in m.iter_mut().enumerate() {
                row[i] = 1.0;
            }
            m[3][3] = o;
            let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
            paint.color_filter = Some(ops::skia_matrix(&m));
            g.draw_image_sx(img2, src, dst, Some(&paint));
        } else {
            g.draw_image_sx(img2, src, dst, None);
        }
    }
}
impl Effect for DrawParticles {
    fn apply(&self, mut bmp: Bitmap, ctx: &mut Context) -> Option<Bitmap> {
        if self.Background {
            let mut result = bmp.empty_like(0, 0);
            self.draw_from_folder(&mut result, ctx);
            let (w, h) = (bmp.width() as f32, bmp.height() as f32);
            Canvas::new(&mut result).draw_image_xywh(&bmp, 0.0, 0.0, w, h);
            Some(result)
        } else {
            self.draw_from_folder(&mut bmp, ctx);
            Some(bmp)
        }
    }
    fn summary(&self) -> Option<String> {
        paths::file_name(&self.ImageFolder)
    }
}

fn truncate_summary(text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    // `Truncate(20, "...")`: the first 20 characters, then "...".
    let chars: Vec<char> = text.chars().collect();
    Some(if chars.len() > 20 {
        chars[..20].iter().collect::<String>() + "..."
    } else {
        text.to_string()
    })
}

fn parse_text(text: &str, bmp: &Bitmap) -> String {
    let mut parser = sharex_helpers::name_parser::NameParser::new(
        sharex_helpers::name_parser::NameParserType::Text,
    );
    parser.image_width = bmp.width();
    parser.image_height = bmp.height();
    parser.parse(text)
}

effect!(
    /// `Drawings/DrawText.cs` ("Text watermark").
    DrawText {
        Text: String = lookup("ImageEffectDefault_Text_watermark").unwrap_or("Text watermark").to_string(),
        Placement: ContentAlignment = ContentAlignment::BottomRight,
        Offset: Point = Point::new(5, 5),
        AutoHide: bool = false,
        TextFont: Option<ImageFont> = Some(ImageFont::new("Arial", 11.25)),
        TextRenderingMode: TextRenderingMode = TextRenderingMode::SystemDefault,
        TextColor: Color = Color::rgb(235, 235, 235),
        DrawTextShadow: bool = true,
        TextShadowColor: Color = Color::named("Black"),
        TextShadowOffset: Point = Point::new(-1, -1),
        CornerRadius: i32 = 4 => max_i(0),
        Padding: Margins = Margins::all(5),
        DrawBorder: bool = true,
        BorderColor: Color = Color::named("Black"),
        BorderSize: i32 = 1,
        DrawBackground: bool = true,
        BackgroundColor: Color = Color::rgb(42, 47, 56),
        UseGradient: bool = false,
        Gradient: Option<GradientInfo> = default_gradient_4(),
    }
);
impl Effect for DrawText {
    fn apply(&self, mut bmp: Bitmap, ctx: &mut Context) -> Option<Bitmap> {
        let Some(font) = &self.TextFont else {
            return Some(bmp);
        };
        if self.Text.is_empty() || font.size < 1.0 {
            return Some(bmp);
        }
        let Some(sk) = ctx.fonts.font(font) else {
            return Some(bmp);
        };
        let text = parse_text(&self.Text, &bmp);
        let (tw, th) = crate::text::measure_text(&sk, &text);
        let size = Size::new(self.Padding.horizontal() + tw, self.Padding.vertical() + th);
        let pos = ops::get_position(
            self.Placement,
            self.Offset,
            Size::new(bmp.width() as i32, bmp.height() as i32),
            size,
        );
        if self.AutoHide
            && !ops::bounds_contain(
                bmp.width(),
                bmp.height(),
                pos.x,
                pos.y,
                size.width,
                size.height,
            )
        {
            return Some(bmp);
        }
        let rect = Rect::xywh(
            pos.x as f32,
            pos.y as f32,
            size.width as f32,
            size.height as f32,
        );
        let path = ops::round_rect_proper(rect, self.CornerRadius as f32, 1.0);
        let mut g = Canvas::new(&mut bmp);
        if let Some(path) = &path {
            if self.DrawBackground {
                let mut p = Paint::shader(shader_or_gradient(
                    self.UseGradient,
                    &self.Gradient,
                    &self.BackgroundColor,
                    pos.x,
                    pos.y,
                    size.width,
                    size.height,
                ));
                p.anti_alias = true;
                g.fill_path(path, &p);
            }
            if self.DrawBorder {
                let stroke = ops::stroke_paint(
                    Shader::Color(self.BorderColor.rgba()),
                    self.BorderSize.max(1) as f32,
                );
                ops::stroke_path(&mut g, path, &stroke);
            }
        }
        let tx = (pos.x + self.Padding.left) as f32;
        let ty = (pos.y + self.Padding.top) as f32;
        let aa = !matches!(
            self.TextRenderingMode,
            TextRenderingMode::SingleBitPerPixel | TextRenderingMode::SingleBitPerPixelGridFit
        );
        let shadow = || {
            crate::text::draw_text_path(
                &sk,
                &text,
                tx + self.TextShadowOffset.x as f32,
                ty + self.TextShadowOffset.y as f32,
            )
        };
        if let Some(p) = self.DrawTextShadow.then(shadow).flatten() {
            {
                // The shadow paint keeps Fill()'s anti-aliasing; the edging follows the mode.
                let mut paint = Paint::fill(self.TextShadowColor.rgba());
                paint.anti_alias = aa;
                g.fill_path(&p, &paint);
            }
        }
        if let Some(p) = crate::text::draw_text_path(&sk, &text, tx, ty) {
            let mut paint = Paint::fill(self.TextColor.rgba());
            paint.anti_alias = aa;
            g.fill_path(&p, &paint);
        }
        Some(bmp)
    }
    fn summary(&self) -> Option<String> {
        truncate_summary(&self.Text)
    }
}

effect!(
    /// `Drawings/DrawTextEx.cs` ("Text").
    DrawTextEx {
        Text: String = lookup("ImageEffectDefault_Text").unwrap_or("Text").to_string(),
        Placement: ContentAlignment = ContentAlignment::TopLeft,
        Offset: Point = Point::new(0, 0),
        Angle: i32 = 0,
        AutoHide: bool = false,
        Font: Option<ImageFont> = Some(ImageFont::new("Arial", 36.0)),
        Color: Color = Color::rgb(235, 235, 235),
        UseGradient: bool = false,
        Gradient: Option<GradientInfo> = random_gradient(GradientMode::Horizontal, &mut RandomFast::seeded(0)),
        Outline: bool = false,
        OutlineSize: i32 = 5,
        OutlineColor: Color = Color::rgb(235, 0, 0),
        OutlineUseGradient: bool = false,
        OutlineGradient: Option<GradientInfo> = random_gradient(GradientMode::Horizontal, &mut RandomFast::seeded(0)),
        Shadow: bool = false,
        ShadowOffset: Point = Point::new(0, 5),
        ShadowColor: Color = Color::argb(125, 0, 0, 0),
        ShadowUseGradient: bool = false,
        ShadowGradient: Option<GradientInfo> = random_gradient(GradientMode::Horizontal, &mut RandomFast::seeded(0)),
    }
);
impl Effect for DrawTextEx {
    fn apply(&self, mut bmp: Bitmap, ctx: &mut Context) -> Option<Bitmap> {
        let Some(font) = &self.Font else {
            return Some(bmp);
        };
        if self.Text.is_empty() || font.size < 1.0 {
            return Some(bmp);
        }
        let Some(sk) = ctx.fonts.font(font) else {
            return Some(bmp);
        };
        let Some(mut path) = crate::text::text_path(&sk, &parse_text(&self.Text, &bmp)) else {
            return Some(bmp);
        };
        if self.Angle != 0 {
            let m = Matrix::rotate(self.Angle as f32);
            path = path.transform(tiny_skia::Transform::from_row(
                m.sx(),
                m.ky(),
                m.kx(),
                m.sy(),
                0.0,
                0.0,
            ))?;
        }
        let b = path.bounds();
        if b.width() <= 0.0 || b.height() <= 0.0 {
            return Some(bmp);
        }
        let size = Size::new(b.width().ceil() as i32 + 1, b.height().ceil() as i32 + 1);
        let pos = ops::get_position(
            self.Placement,
            self.Offset,
            Size::new(bmp.width() as i32, bmp.height() as i32),
            size,
        );
        if self.AutoHide
            && !ops::bounds_contain(
                bmp.width(),
                bmp.height(),
                pos.x,
                pos.y,
                size.width,
                size.height,
            )
        {
            return Some(bmp);
        }
        let path = path.transform(tiny_skia::Transform::from_translate(
            pos.x as f32 - b.left(),
            pos.y as f32 - b.top(),
        ))?;
        let (x, y, w, h) = (pos.x, pos.y, size.width, size.height);
        let grow = |r: (i32, i32, i32, i32), o: i32| (r.0 - o, r.1 - o, r.2 + o * 2, r.3 + o * 2);
        let gshader =
            |g: &GradientInfo, r: (i32, i32, i32, i32)| ops::gradient_shader(g, r.0, r.1, r.2, r.3);
        let outline_on = self.Outline && self.OutlineSize > 0;
        let mut g = Canvas::new(&mut bmp);
        let shadow_visible = if self.ShadowUseGradient {
            self.ShadowGradient.as_ref().is_some_and(|g| g.is_visible())
        } else {
            self.ShadowColor.a > 0
        };
        if self.Shadow && shadow_visible {
            let so = self.ShadowOffset;
            if let Some(sp) = path.clone().transform(tiny_skia::Transform::from_translate(
                so.x as f32,
                so.y as f32,
            )) {
                let shader = match &self.ShadowGradient {
                    Some(gr) if self.ShadowUseGradient => {
                        let r = grow(
                            (x, y, w, h),
                            if self.Outline {
                                self.OutlineSize + 1
                            } else {
                                1
                            },
                        );
                        gshader(gr, (r.0 + so.x, r.1 + so.y, r.2, r.3))
                    }
                    _ => Shader::Color(self.ShadowColor.rgba()),
                };
                if outline_on {
                    ops::stroke_path(
                        &mut g,
                        &sp,
                        &ops::stroke_paint(shader, self.OutlineSize as f32),
                    );
                } else {
                    let mut p = Paint::shader(shader);
                    p.anti_alias = true;
                    g.fill_path(&sp, &p);
                }
            }
        }
        let outline_visible = if self.OutlineUseGradient {
            self.OutlineGradient
                .as_ref()
                .is_some_and(|g| g.is_visible())
        } else {
            self.OutlineColor.a > 0
        };
        if outline_on && outline_visible {
            let shader = match &self.OutlineGradient {
                Some(gr) if self.OutlineUseGradient => {
                    gshader(gr, grow((x, y, w, h), self.OutlineSize + 1))
                }
                _ => Shader::Color(self.OutlineColor.rgba()),
            };
            ops::stroke_path(
                &mut g,
                &path,
                &ops::stroke_paint(shader, self.OutlineSize as f32),
            );
        }
        let fill_visible = if self.UseGradient {
            self.Gradient.as_ref().is_some_and(|g| g.is_visible())
        } else {
            self.Color.a > 0
        };
        if fill_visible {
            let shader = match &self.Gradient {
                Some(gr) if self.UseGradient => gshader(gr, grow((x, y, w, h), 1)),
                _ => Shader::Color(self.Color.rgba()),
            };
            let mut p = Paint::shader(shader);
            p.anti_alias = true;
            g.fill_path(&path, &p);
        }
        Some(bmp)
    }
    fn summary(&self) -> Option<String> {
        truncate_summary(&self.Text)
    }
}

// ───────────────────────────── Registry ─────────────────────────────

/// The effect categories, in ShareX's order (`ImageEffectCatalog`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Drawings,
    Manipulations,
    Adjustments,
    Filters,
}

impl Category {
    /// The localised category name (`ImageEffectCategory_*`).
    pub fn name(self) -> &'static str {
        let (key, fallback) = match self {
            Category::Drawings => ("ImageEffectCategory_Drawings", "Drawings"),
            Category::Manipulations => ("ImageEffectCategory_Manipulations", "Manipulations"),
            Category::Adjustments => ("ImageEffectCategory_Adjustments", "Adjustments"),
            Category::Filters => ("ImageEffectCategory_Filters", "Filters"),
        };
        lookup(key).unwrap_or(fallback)
    }
}

macro_rules! registry {
    ($( $cat:ident: [ $( $ty:ident $(as $rust:ident)? => $display:literal ),* $(,)? ] ),* $(,)?) => {
        /// One effect of any type.
        #[derive(Debug, Clone, PartialEq)]
        pub enum EffectKind { $($( $ty(registry!(@rust $ty $($rust)?)), )*)* }

        impl EffectKind {
            /// ShareX's class name (`$type` without namespace).
            pub fn type_name(&self) -> &'static str {
                match self { $($( EffectKind::$ty(_) => stringify!($ty), )*)* }
            }

            pub fn category(&self) -> Category {
                match self { $($( EffectKind::$ty(_) => Category::$cat, )*)* }
            }

            /// A new effect as ShareX's constructor makes it (`rng` for the random
            /// default gradients).
            pub fn create(type_name: &str, rng: &mut RandomFast) -> Option<Self> {
                $($( if type_name == stringify!($ty) {
                    #[allow(unused_mut)]
                    let mut e = <registry!(@rust $ty $($rust)?)>::default();
                    registry!(@init $ty e rng);
                    return Some(EffectKind::$ty(e));
                } )*)*
                None
            }

            /// The property names, in ShareX's order.
            pub fn property_names(&self) -> &'static [&'static str] {
                match self { $($( EffectKind::$ty(_) => <registry!(@rust $ty $($rust)?)>::PROPERTY_NAMES, )*)* }
            }

            pub fn get_property(&self, name: &str) -> Option<Value> {
                match self { $($( EffectKind::$ty(e) => e.get_property(name), )*)* }
            }

            pub fn set_property(&mut self, name: &str, v: &Value) -> bool {
                if let EffectKind::Crop(c) = self {
                    // Crop's setter ignores negative margins.
                    let old = c.Margin;
                    let ok = c.set_property(name, v);
                    c.Margin = Crop::normalized(old, c.Margin);
                    return ok;
                }
                match self { $($( EffectKind::$ty(e) => e.set_property(name, v), )*)* }
            }

            pub(crate) fn write_props(&self, map: &mut Map<String, Value>) {
                match self { $($( EffectKind::$ty(e) => e.write_props(map), )*)* }
            }

            pub fn apply(&self, bmp: Bitmap, ctx: &mut Context) -> Option<Bitmap> {
                match self { $($( EffectKind::$ty(e) => e.apply(bmp, ctx), )*)* }
            }

            pub fn summary(&self) -> Option<String> {
                match self { $($( EffectKind::$ty(e) => e.summary(), )*)* }
            }

            /// Effects without a constructor never run `ApplyDefaultPropertyValues`, so
            /// their `Name` stays null.
            pub fn name_defaults_to_null(&self) -> bool {
                matches!(self, EffectKind::BlackWhite(_) | EffectKind::Inverse(_) | EffectKind::Polaroid(_) | EffectKind::EdgeDetect(_)
                    | EffectKind::Emboss(_) | EffectKind::MeanRemoval(_) | EffectKind::Sharpen(_) | EffectKind::Smooth(_)
                    | EffectKind::RGBSplit(_) | EffectKind::ForceProportions(_))
            }

            /// The localised effect name (`ImageEffectsLocalization.GetEffectName`).
            pub fn display_name(&self) -> &'static str {
                match self { $($( EffectKind::$ty(_) => lookup(concat!("ImageEffect_", stringify!($ty))).unwrap_or($display), )*)* }
            }
        }

        /// Every effect type, by category (`ImageEffectCatalog.All`).
        pub const CATALOG: &[(Category, &str)] = &[ $($( (Category::$cat, stringify!($ty)), )*)* ];
    };
    (@rust $ty:ident) => { $ty };
    (@rust $ty:ident $rust:ident) => { $rust };
    (@init Glow $e:ident $rng:ident) => { $e.Gradient = random_gradient(GradientMode::ForwardDiagonal, $rng); };
    (@init DrawTextEx $e:ident $rng:ident) => {
        $e.Gradient = random_gradient(GradientMode::Horizontal, $rng);
        $e.OutlineGradient = random_gradient(GradientMode::Horizontal, $rng);
        $e.ShadowGradient = random_gradient(GradientMode::Horizontal, $rng);
    };
    (@init $ty:ident $e:ident $rng:ident) => { let _ = &$rng; };
}

registry! {
    Drawings: [
        DrawBackground => "Background", DrawBackgroundImage => "Background image", DrawBorder => "Border",
        DrawCheckerboard => "Checkerboard", DrawImage => "Image", DrawParticles => "Particles",
        DrawTextEx => "Text", DrawText => "Text watermark",
    ],
    Manipulations: [
        AutoCrop => "Auto crop", Canvas as Canvas_ => "Canvas", Crop => "Crop", Flip => "Flip",
        ForceProportions => "Force proportions", Resize => "Resize", Rotate => "Rotate",
        RoundedCorners => "Rounded corners", Scale => "Scale", Skew => "Skew",
    ],
    Adjustments: [
        Alpha => "Alpha", BlackWhite => "Black & white", Brightness => "Brightness", MatrixColor => "Color matrix",
        Colorize => "Colorize", Contrast => "Contrast", Gamma => "Gamma", Grayscale => "Grayscale", Hue => "Hue",
        Inverse => "Inverse", Polaroid => "Polaroid", ReplaceColor => "Replace color", Saturation => "Saturation",
        SelectiveColor => "Selective color", Sepia => "Sepia",
    ],
    Filters: [
        Blur => "Blur", ColorDepth => "Color depth", MatrixConvolution => "Convolution matrix", EdgeDetect => "Edge detect",
        Emboss => "Emboss", GaussianBlur => "Gaussian blur", Glow => "Glow", MeanRemoval => "Mean removal",
        Outline => "Outline", Pixelate => "Pixelate", Reflection => "Reflection", RGBSplit => "RGB split",
        Shadow => "Shadow", Sharpen => "Sharpen", Slice => "Slice", Smooth => "Smooth", TornEdge => "Torn edge",
        WaveEdge => "Wave edge",
    ],
}
