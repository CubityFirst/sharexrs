//! The tool options of `MainViewModel.ToolOptions.cs`: what the options
//! toolbar shows and new annotations take, loaded from
//! `ImageEditorOptions` for each tool (`LoadOptionsForTool`) and written
//! back to the tool's own option as they change (`On…Changed`).

use crate::model::{Annotation, ArrowStyle, BorderStyle, CursorType, EditorTool, Kind, StepType, TextHorizontalAlignment};
use crate::options::ImageEditorOptions;
use crate::render::parse_color;

/// `$"#{A:X2}{R:X2}{G:X2}{B:X2}"` of `Color.Parse(s)`.
pub fn normalize_hex(s: &str) -> String {
    let c = parse_color(s);
    format!("#{:02X}{:02X}{:02X}{:02X}", c.a, c.r, c.g, c.b)
}

/// `GetMaxEffectStrength`.
pub fn max_effect_strength(tool: EditorTool) -> f32 {
    match tool {
        EditorTool::Blur | EditorTool::Pixelate => 200.0,
        EditorTool::Magnify => 10.0,
        EditorTool::Spotlight => 100.0,
        _ => 30.0,
    }
}

const MIN_EFFECT_STRENGTH: f32 = 1.0;
const MAX_SPOTLIGHT_BLUR: f32 = 200.0;

/// The type of tool an annotation was made by (`ToolType`).
fn tool_of(a: Option<&Annotation>) -> Option<EditorTool> {
    a.map(|a| a.common.tool_type.clone())
}

/// The tool options (`MainViewModel`'s observable properties).
#[derive(Clone, Debug)]
pub struct ToolState {
    pub options: ImageEditorOptions,
    pub selected_color: String,
    pub stroke_width: i32,
    pub corner_radius: i32,
    pub fill_color: String,
    pub text_color: String,
    pub font_size: f32,
    pub step_start_number: i32,
    pub step_type: StepType,
    pub text_alignment: TextHorizontalAlignment,
    pub font_family: String,
    pub border_style: BorderStyle,
    pub arrow_style: ArrowStyle,
    pub cursor_type: CursorType,
    pub effect_strength: f32,
    pub spotlight_blur: f32,
    pub effect_ellipse: bool,
    pub shadow_enabled: bool,
    pub shadow_color: String,
    pub shadow_blur_radius: f64,
    pub shadow_opacity: f64,
    pub shadow_offset_x: f64,
    pub shadow_offset_y: f64,
    pub arrow_rounded_corners: bool,
    pub freehand_arrow: bool,
    pub speech_balloon_tail: bool,
    pub speech_balloon_auto_size: bool,
    pub text_bold: bool,
    pub text_italic: bool,
    loading: bool,
}

/// What a setter acts for: the active tool and the selected annotation.
#[derive(Clone, Copy)]
pub struct Target<'a> {
    pub tool: &'a EditorTool,
    pub selected: Option<&'a Annotation>,
}

impl Target<'_> {
    fn is_select(&self) -> bool {
        *self.tool == EditorTool::Select && self.selected.is_some()
    }

    /// `ActiveTool == tool`, or the selected annotation's tool in Select.
    fn effective(&self) -> EditorTool {
        if self.is_select() { tool_of(self.selected).unwrap_or(EditorTool::Select) } else { self.tool.clone() }
    }
}

impl ToolState {
    /// `new MainViewModel(options)`: the fields from the options, then
    /// `LoadOptionsForTool(initial tool)`.
    pub fn new(options: ImageEditorOptions, initial_tool: EditorTool) -> Self {
        let o = &options;
        let mut me = Self {
            selected_color: normalize_hex(&o.BorderColorHex),
            stroke_width: o.Thickness,
            corner_radius: o.CornerRadius,
            fill_color: normalize_hex(&o.FillColorHex),
            text_color: normalize_hex(&o.TextTextColorHex),
            font_size: o.TextFontSize,
            step_start_number: 1,
            step_type: StepType::Numeric,
            text_alignment: TextHorizontalAlignment::Center,
            font_family: normalize_font_family(&o.TextFontFamily),
            border_style: o.BorderStyle.clone(),
            arrow_style: o.ArrowStyle.clone(),
            cursor_type: CursorType::Default,
            effect_strength: 1.0,
            spotlight_blur: 0.0,
            effect_ellipse: false,
            shadow_enabled: o.Shadow,
            shadow_color: o.ShadowColorHex.clone(),
            shadow_blur_radius: o.ShadowBlurRadius,
            shadow_opacity: o.ShadowOpacity,
            shadow_offset_x: o.ShadowOffsetX,
            shadow_offset_y: o.ShadowOffsetY,
            arrow_rounded_corners: true,
            freehand_arrow: false,
            speech_balloon_tail: true,
            speech_balloon_auto_size: false,
            text_bold: o.TextBold,
            text_italic: o.TextItalic,
            options,
            loading: false,
        };
        me.load_for_tool(initial_tool);
        me
    }

    /// `LoadOptionsForTool`.
    pub fn load_for_tool(&mut self, tool: EditorTool) {
        self.loading = true;
        let t = Target { tool: &tool, selected: None };
        let o = self.options.clone();
        self.set_shadow_color(&o.ShadowColorHex);
        self.set_shadow_blur_radius(o.ShadowBlurRadius);
        self.set_shadow_opacity(o.ShadowOpacity);
        self.shadow_offset_x = o.ShadowOffsetX;
        self.options.ShadowOffsetX = o.ShadowOffsetX;
        self.shadow_offset_y = o.ShadowOffsetY;
        self.options.ShadowOffsetY = o.ShadowOffsetY;
        match &tool {
            EditorTool::Rectangle | EditorTool::Ellipse | EditorTool::Line | EditorTool::Freehand | EditorTool::Arrow => {
                self.set_selected_color(&o.BorderColorHex, t);
                self.set_fill_color(&o.FillColorHex, t);
                self.set_stroke_width(o.Thickness, t);
                self.set_corner_radius(o.CornerRadius, t);
                self.set_shadow_enabled(o.Shadow);
                if tool != EditorTool::Arrow {
                    self.set_border_style(o.BorderStyle.clone(), t);
                }
                self.set_font_size(o.TextFontSize, t);
                if tool == EditorTool::Freehand {
                    self.set_freehand_arrow(o.FreehandArrow, t);
                }
                if tool == EditorTool::Arrow {
                    self.set_arrow_style(o.ArrowStyle.clone(), t);
                    self.set_arrow_rounded_corners(o.ArrowRoundedCorners, t);
                }
            }
            EditorTool::Cursor => self.set_cursor_type(o.CursorType.clone(), t),
            EditorTool::Text => {
                self.set_selected_color(&o.TextBorderColorHex, t);
                self.set_text_color(&o.TextTextColorHex, t);
                self.set_stroke_width(o.TextThickness, t);
                self.set_shadow_enabled(o.Shadow);
                self.set_font_size(o.TextFontSize, t);
                self.set_font_family(&o.TextFontFamily, t);
                self.set_text_alignment(o.TextHorizontalAlignment.clone(), t);
                self.set_text_bold(o.TextBold, t);
                self.set_text_italic(o.TextItalic, t);
            }
            EditorTool::SpeechBalloon => {
                self.set_selected_color(&o.SpeechBalloonBorderColorHex, t);
                self.set_fill_color(&o.SpeechBalloonFillColorHex, t);
                self.set_text_color(&o.SpeechBalloonTextColorHex, t);
                self.set_stroke_width(o.SpeechBalloonThickness, t);
                self.set_corner_radius(o.CornerRadius, t);
                self.set_shadow_enabled(o.Shadow);
                self.set_speech_balloon_tail(o.SpeechBalloonTail, t);
                self.set_speech_balloon_auto_size(o.SpeechBalloonAutoSize, t);
                self.set_font_size(o.SpeechBalloonFontSize, t);
                self.set_font_family(&o.SpeechBalloonFontFamily, t);
                self.set_text_alignment(o.SpeechBalloonTextHorizontalAlignment.clone(), t);
                self.set_text_bold(o.SpeechBalloonTextBold, t);
                self.set_text_italic(o.SpeechBalloonTextItalic, t);
            }
            EditorTool::Step => {
                self.set_selected_color(&o.StepBorderColorHex, t);
                self.set_fill_color(&o.StepFillColorHex, t);
                self.set_text_color(&o.StepTextColorHex, t);
                self.set_stroke_width(o.StepThickness, t);
                self.set_shadow_enabled(o.Shadow);
                self.set_speech_balloon_tail(o.StepTail, t);
                self.set_font_size(o.StepFontSize, t);
                self.set_step_type(o.StepType.clone());
                self.set_text_bold(o.StepTextBold, t);
                self.set_text_italic(o.TextItalic, t);
            }
            EditorTool::Highlight => self.set_fill_color(&o.HighlightFillColorHex, t),
            EditorTool::Blur => self.set_effect_strength(o.BlurStrength, t),
            EditorTool::Pixelate => self.set_effect_strength(o.PixelateStrength, t),
            EditorTool::Magnify => {
                self.set_effect_strength(o.MagnifierStrength, t);
                self.set_effect_ellipse(o.MagnifierEllipse, t);
                self.set_selected_color(&o.MagnifierBorderColorHex, t);
                self.set_stroke_width(o.MagnifierThickness, t);
                self.set_border_style(o.MagnifierBorderStyle.clone(), t);
            }
            EditorTool::Spotlight => {
                self.set_effect_strength(o.SpotlightStrength, t);
                self.set_spotlight_blur(o.SpotlightBlur, t);
                self.set_effect_ellipse(o.SpotlightEllipse, t);
            }
            _ => {}
        }
        self.loading = false;
    }

    /// `SelectedColor` (`UpdateDefaultOptionsColor`).
    pub fn set_selected_color(&mut self, v: &str, t: Target) {
        let v = normalize_hex(v);
        if v == self.selected_color {
            return;
        }
        self.selected_color = v.clone();
        if *t.tool == EditorTool::Select && t.selected.is_some() {
            return;
        }
        let o = &mut self.options;
        match t.tool {
            EditorTool::Step => o.StepBorderColorHex = v,
            EditorTool::SpeechBalloon => o.SpeechBalloonBorderColorHex = v,
            EditorTool::Text => o.TextBorderColorHex = v,
            EditorTool::Magnify => o.MagnifierBorderColorHex = v,
            _ => o.BorderColorHex = v,
        }
    }

    /// `StrokeWidth`.
    pub fn set_stroke_width(&mut self, v: i32, t: Target) {
        if v == self.stroke_width {
            return;
        }
        self.stroke_width = v;
        let o = &mut self.options;
        match t.tool {
            EditorTool::Step => o.StepThickness = v,
            EditorTool::SpeechBalloon => o.SpeechBalloonThickness = v,
            EditorTool::Text => o.TextThickness = v,
            EditorTool::Magnify => o.MagnifierThickness = v,
            EditorTool::Select if t.selected.is_some() => match &t.selected.unwrap().kind {
                Kind::Number(_) => o.StepThickness = v,
                Kind::SpeechBalloon(_) => o.SpeechBalloonThickness = v,
                Kind::Text(_) => o.TextThickness = v,
                Kind::Magnify(_) => o.MagnifierThickness = v,
                Kind::SmartEraser(_) => {}
                _ => o.Thickness = v,
            },
            _ => o.Thickness = v,
        }
    }

    /// `CornerRadius` (at least 0).
    pub fn set_corner_radius(&mut self, v: i32, t: Target) {
        let v = v.max(0);
        if v == self.corner_radius {
            return;
        }
        self.corner_radius = v;
        let applies = match tool_of(t.selected).filter(|_| t.is_select()) {
            Some(st) => matches!(st, EditorTool::Rectangle | EditorTool::SpeechBalloon),
            None => matches!(*t.tool, EditorTool::Rectangle | EditorTool::SpeechBalloon),
        };
        if applies {
            self.options.CornerRadius = v;
        }
    }

    /// `FillColor`.
    pub fn set_fill_color(&mut self, v: &str, t: Target) {
        let v = normalize_hex(v);
        if v == self.fill_color {
            return;
        }
        self.fill_color = v.clone();
        let o = &mut self.options;
        match t.tool {
            EditorTool::Step => o.StepFillColorHex = v,
            EditorTool::SpeechBalloon => o.SpeechBalloonFillColorHex = v,
            EditorTool::Highlight => o.HighlightFillColorHex = v,
            _ => o.FillColorHex = v,
        }
    }

    /// `TextColor`.
    pub fn set_text_color(&mut self, v: &str, t: Target) {
        let v = normalize_hex(v);
        if v == self.text_color {
            return;
        }
        self.text_color = v.clone();
        let o = &mut self.options;
        match t.effective() {
            EditorTool::Step => o.StepTextColorHex = v,
            EditorTool::SpeechBalloon => o.SpeechBalloonTextColorHex = v,
            EditorTool::Text => o.TextTextColorHex = v,
            _ => {}
        }
    }

    /// `FontSize`.
    pub fn set_font_size(&mut self, v: f32, t: Target) {
        if v == self.font_size {
            return;
        }
        self.font_size = v;
        let o = &mut self.options;
        match t.effective() {
            EditorTool::Step => o.StepFontSize = v,
            EditorTool::SpeechBalloon => o.SpeechBalloonFontSize = v,
            _ => o.TextFontSize = v,
        }
    }

    /// `StepStartNumber` (1 to 10); also resets the counter.
    pub fn set_step_start_number(&mut self, v: i32) -> bool {
        let v = v.clamp(1, 10);
        if v == self.step_start_number {
            return false;
        }
        self.step_start_number = v;
        true
    }

    /// `SelectedStepType`.
    pub fn set_step_type(&mut self, v: StepType) {
        if v == self.step_type {
            return;
        }
        self.step_type = v.clone();
        self.options.StepType = v;
    }

    /// `SelectedTextHorizontalAlignment`.
    pub fn set_text_alignment(&mut self, v: TextHorizontalAlignment, t: Target) {
        if v == self.text_alignment {
            return;
        }
        self.text_alignment = v.clone();
        match t.effective() {
            EditorTool::Text => self.options.TextHorizontalAlignment = v,
            EditorTool::SpeechBalloon => self.options.SpeechBalloonTextHorizontalAlignment = v,
            _ => {}
        }
    }

    /// `SelectedFontFamily`.
    pub fn set_font_family(&mut self, v: &str, t: Target) {
        let v = normalize_font_family(v);
        if v == self.font_family {
            return;
        }
        self.font_family = v.clone();
        match t.effective() {
            EditorTool::Text => self.options.TextFontFamily = v,
            EditorTool::SpeechBalloon => self.options.SpeechBalloonFontFamily = v,
            _ => {}
        }
    }

    /// `SelectedBorderStyle`.
    pub fn set_border_style(&mut self, v: BorderStyle, t: Target) {
        if v == self.border_style {
            return;
        }
        self.border_style = v.clone();
        match t.effective() {
            EditorTool::Rectangle | EditorTool::Ellipse | EditorTool::Line | EditorTool::Freehand => {
                self.options.BorderStyle = v
            }
            EditorTool::Magnify => self.options.MagnifierBorderStyle = v,
            _ => {}
        }
    }

    /// `SelectedArrowStyle`.
    pub fn set_arrow_style(&mut self, v: ArrowStyle, t: Target) {
        if v == self.arrow_style {
            return;
        }
        self.arrow_style = v.clone();
        if t.effective() == EditorTool::Arrow {
            self.options.ArrowStyle = v;
        }
    }

    /// `SelectedCursorType`.
    pub fn set_cursor_type(&mut self, v: CursorType, t: Target) {
        if v == self.cursor_type {
            return;
        }
        self.cursor_type = v.clone();
        if t.effective() == EditorTool::Cursor {
            self.options.CursorType = v;
        }
    }

    /// `EffectStrength`, kept within the tool's range.
    pub fn set_effect_strength(&mut self, v: f32, t: Target) {
        let tool = t.effective();
        let v = v.clamp(MIN_EFFECT_STRENGTH, max_effect_strength(tool.clone()));
        if v == self.effect_strength {
            return;
        }
        self.effect_strength = v;
        if self.loading {
            return;
        }
        match tool {
            EditorTool::Blur => self.options.BlurStrength = v,
            EditorTool::Pixelate => self.options.PixelateStrength = v,
            EditorTool::Magnify => self.options.MagnifierStrength = v,
            EditorTool::Spotlight => self.options.SpotlightStrength = v,
            _ => {}
        }
    }

    /// `SpotlightBlur` (0 to 200).
    pub fn set_spotlight_blur(&mut self, v: f32, t: Target) {
        let v = v.clamp(0.0, MAX_SPOTLIGHT_BLUR);
        if v == self.spotlight_blur {
            return;
        }
        self.spotlight_blur = v;
        if !self.loading && t.effective() == EditorTool::Spotlight {
            self.options.SpotlightBlur = v;
        }
    }

    /// `EffectEllipse`.
    pub fn set_effect_ellipse(&mut self, v: bool, t: Target) {
        if v == self.effect_ellipse {
            return;
        }
        self.effect_ellipse = v;
        match t.effective() {
            EditorTool::Magnify => self.options.MagnifierEllipse = v,
            EditorTool::Spotlight => self.options.SpotlightEllipse = v,
            _ => {}
        }
    }

    /// `ShadowEnabled`.
    pub fn set_shadow_enabled(&mut self, v: bool) {
        if v == self.shadow_enabled {
            return;
        }
        self.shadow_enabled = v;
        self.options.Shadow = v;
    }

    pub fn set_shadow_color(&mut self, v: &str) {
        if v == self.shadow_color {
            return;
        }
        self.shadow_color = v.to_string();
        self.options.ShadowColorHex = normalize_hex(v);
    }

    pub fn set_shadow_blur_radius(&mut self, v: f64) {
        let v = v.max(0.0);
        if v == self.shadow_blur_radius {
            return;
        }
        self.shadow_blur_radius = v;
        self.options.ShadowBlurRadius = v;
    }

    pub fn set_shadow_opacity(&mut self, v: f64) {
        let v = v.clamp(0.0, 1.0);
        if v == self.shadow_opacity {
            return;
        }
        self.shadow_opacity = v;
        self.options.ShadowOpacity = v;
    }

    /// `ArrowRoundedCorners`.
    pub fn set_arrow_rounded_corners(&mut self, v: bool, t: Target) {
        if v == self.arrow_rounded_corners {
            return;
        }
        self.arrow_rounded_corners = v;
        if t.effective() == EditorTool::Arrow {
            self.options.ArrowRoundedCorners = v;
        }
    }

    /// `FreehandArrow`.
    pub fn set_freehand_arrow(&mut self, v: bool, t: Target) {
        if v == self.freehand_arrow {
            return;
        }
        self.freehand_arrow = v;
        if t.effective() == EditorTool::Freehand {
            self.options.FreehandArrow = v;
        }
    }

    /// `SpeechBalloonTail` (also the step's tail).
    pub fn set_speech_balloon_tail(&mut self, v: bool, t: Target) {
        if v == self.speech_balloon_tail {
            return;
        }
        self.speech_balloon_tail = v;
        let (balloon, step) = if *t.tool == EditorTool::Select {
            (
                matches!(t.selected.map(|a| &a.kind), Some(Kind::SpeechBalloon(_))),
                matches!(t.selected.map(|a| &a.kind), Some(Kind::Number(_))),
            )
        } else {
            (*t.tool == EditorTool::SpeechBalloon, *t.tool == EditorTool::Step)
        };
        if balloon {
            self.options.SpeechBalloonTail = v;
        } else if step {
            self.options.StepTail = v;
        }
    }

    /// `SpeechBalloonAutoSize`.
    pub fn set_speech_balloon_auto_size(&mut self, v: bool, t: Target) {
        if v == self.speech_balloon_auto_size {
            return;
        }
        self.speech_balloon_auto_size = v;
        if t.effective() == EditorTool::SpeechBalloon {
            self.options.SpeechBalloonAutoSize = v;
        }
    }

    /// `TextBold`.
    pub fn set_text_bold(&mut self, v: bool, t: Target) {
        if v == self.text_bold {
            return;
        }
        self.text_bold = v;
        match t.effective() {
            EditorTool::Text => self.options.TextBold = v,
            EditorTool::SpeechBalloon => self.options.SpeechBalloonTextBold = v,
            EditorTool::Step => self.options.StepTextBold = v,
            _ => {}
        }
    }

    /// `TextItalic`.
    pub fn set_text_italic(&mut self, v: bool, t: Target) {
        if v == self.text_italic {
            return;
        }
        self.text_italic = v;
        match t.effective() {
            EditorTool::Text => self.options.TextItalic = v,
            EditorTool::SpeechBalloon => self.options.SpeechBalloonTextItalic = v,
            _ => {}
        }
    }
}

/// `NormalizeFontFamily`: a family the system has, else Segoe UI.
pub fn normalize_font_family(f: &str) -> String {
    if !f.trim().is_empty() && crate::render::text::load_face(f, false, false).is_some() {
        f.to_string()
    } else {
        "Segoe UI".to_string()
    }
}

/// Which option controls the toolbar shows for a tool (`Show…`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Visible {
    pub strength: bool,
    pub border_color: bool,
    pub fill_color: bool,
    pub text_color: bool,
    pub thickness: bool,
    pub border_style: bool,
    pub corner_radius: bool,
    pub font_family: bool,
    pub text_alignment: bool,
    pub arrow_style: bool,
    pub cursor_type: bool,
    pub font_size: bool,
    pub text_style: bool,
    pub text_italic: bool,
    pub step_type: bool,
    pub step_start_number: bool,
    pub spotlight_blur: bool,
    pub effect_ellipse: bool,
    pub arrow_rounded_corners: bool,
    pub freehand_arrow: bool,
    pub speech_balloon_tail: bool,
    pub speech_balloon_auto_size: bool,
    pub shadow: bool,
}

impl Visible {
    /// For the active tool, or the selected annotation's in Select.
    pub fn of(tool: EditorTool, selected: Option<&Annotation>) -> Self {
        use EditorTool as T;
        let select = tool == T::Select;
        let sel_tool = tool_of(selected);
        // `Show…` properties: the tool's own set, or in Select the
        // selected annotation's (none without a selection).
        let any = |set: &[T]| {
            if select { sel_tool.as_ref().is_some_and(|s| set.contains(s)) } else { set.contains(&tool) }
        };
        let kind = selected.map(|a| &a.kind);
        let only = |t: T, sel: bool| if select { sel } else { tool == t };
        Self {
            border_color: any(&[T::Rectangle, T::Ellipse, T::Line, T::Arrow, T::Freehand, T::SpeechBalloon, T::Text, T::Step, T::Magnify]),
            fill_color: any(&[T::Rectangle, T::Ellipse, T::SpeechBalloon, T::Step, T::Highlight]),
            text_color: any(&[T::Text, T::SpeechBalloon, T::Step]),
            thickness: any(&[T::Rectangle, T::Ellipse, T::Line, T::Arrow, T::Freehand, T::SpeechBalloon, T::Step, T::Text, T::Magnify]),
            font_size: any(&[T::Text, T::Step, T::SpeechBalloon]),
            step_start_number: tool == T::Step,
            step_type: tool == T::Step,
            text_alignment: any(&[T::Text, T::SpeechBalloon]),
            font_family: any(&[T::Text, T::SpeechBalloon]),
            border_style: any(&[T::Rectangle, T::Ellipse, T::Line, T::Freehand, T::Magnify]),
            arrow_style: only(T::Arrow, matches!(kind, Some(Kind::Arrow(_)))),
            cursor_type: only(T::Cursor, matches!(kind, Some(Kind::Cursor(_)))),
            corner_radius: any(&[T::Rectangle, T::SpeechBalloon]),
            strength: any(&[T::Blur, T::Pixelate, T::Magnify, T::Spotlight]),
            spotlight_blur: only(T::Spotlight, matches!(kind, Some(Kind::Spotlight(_)))),
            effect_ellipse: if select {
                matches!(kind, Some(Kind::Magnify(_) | Kind::Spotlight(_)))
            } else {
                matches!(tool, T::Magnify | T::Spotlight)
            },
            shadow: any(&[T::Rectangle, T::Ellipse, T::Line, T::Arrow, T::Freehand, T::Text, T::SpeechBalloon, T::Step]),
            arrow_rounded_corners: only(T::Arrow, matches!(kind, Some(Kind::Arrow(_)))),
            freehand_arrow: only(T::Freehand, matches!(kind, Some(Kind::Freehand(_)))),
            speech_balloon_tail: if select {
                matches!(kind, Some(Kind::SpeechBalloon(_) | Kind::Number(_)))
            } else {
                matches!(tool, T::SpeechBalloon | T::Step)
            },
            speech_balloon_auto_size: only(T::SpeechBalloon, matches!(kind, Some(Kind::SpeechBalloon(_)))),
            text_style: any(&[T::Text, T::SpeechBalloon, T::Step]),
            text_italic: any(&[T::Text, T::SpeechBalloon]),
        }
    }

    /// `ShowToolOptionsSeparator`: any of them.
    pub fn any(&self) -> bool {
        let v = *self;
        v.border_color || v.fill_color || v.text_color || v.thickness || v.font_size || v.step_start_number
            || v.step_type || v.text_alignment || v.font_family || v.border_style || v.arrow_style
            || v.cursor_type || v.corner_radius || v.strength || v.spotlight_blur || v.effect_ellipse
            || v.text_style || v.shadow || v.speech_balloon_tail || v.speech_balloon_auto_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_tool_uses_its_own_options() {
        let mut s = ToolState::new(ImageEditorOptions::default(), EditorTool::Rectangle);
        assert_eq!(s.selected_color, "#FFF23C3C");
        assert_eq!(s.stroke_width, 4);
        s.load_for_tool(EditorTool::Text);
        assert_eq!(s.stroke_width, 8);
        let t = Target { tool: &EditorTool::Text, selected: None };
        s.set_stroke_width(6, t);
        assert_eq!(s.options.TextThickness, 6);
        assert_eq!(s.options.Thickness, 4);
        s.load_for_tool(EditorTool::Blur);
        assert_eq!(s.effect_strength, 30.0);
        s.load_for_tool(EditorTool::Magnify);
        assert_eq!(s.effect_strength, 2.0);
    }
}
