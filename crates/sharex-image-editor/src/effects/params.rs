//! Effect parameters (`Core/ImageEffects/Parameters`): what ShareX's effect
//! dialog (`SchemaDrivenEffectDialog`) builds its controls from, and how a
//! value reaches the effect (`EffectParameters`' typed helpers: an int
//! slider rounds half to even, an int numeric box rounds half away from
//! zero, a float slider narrows to `float`).

use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};

use super::color::SkColor;

/// The control a parameter is edited with, its range and default.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    /// `SliderEffectParameter`: a slider; `format` is its Avalonia
    /// `StringFormat` (`"{}{0:0}"`, `"{}{0:0.0}"`...).
    Slider {
        min: f64,
        max: f64,
        default: f64,
        tick: f64,
        snap: bool,
        format: &'static str,
    },
    /// `NumericEffectParameter`: a numeric up-down (a `decimal`); `format`
    /// is its .NET format string (`"0"`, `"0.0"`...).
    Numeric {
        min: f64,
        max: f64,
        default: f64,
        increment: f64,
        format: &'static str,
    },
    /// `CheckboxEffectParameter`: a toggle switch.
    Checkbox { default: bool },
    /// `EnumEffectParameter`: a combo box of option labels.
    Enum {
        options: Vec<&'static str>,
        default: usize,
    },
    /// `ColorEffectParameter`: a colour picker.
    Color { default: SkColor },
    /// `TextEffectParameter`: a text box.
    Text { default: String },
    /// `FilePathEffectParameter`: a path box with a Browse button
    /// (`filter`: `"Image files|*.png;*.jpg"`).
    FilePath {
        default: String,
        filter: Option<&'static str>,
    },
}

/// A parameter's value, as its control holds it.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    /// A slider's `double` or a numeric box's `decimal`.
    Number(f64),
    Bool(bool),
    /// The index of an enum's option.
    Choice(usize),
    Color(SkColor),
    /// Text, or a file path.
    Text(String),
}

impl ParamValue {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            ParamValue::Number(v) => Some(*v),
            _ => None,
        }
    }
}

type Applier = Box<dyn Fn(&mut dyn Any, &ParamValue) + Send + Sync>;

/// `EffectParameter`: key, label, optional description, control, and how
/// the value is set on the effect.
pub struct ParamDef {
    pub key: &'static str,
    pub label: &'static str,
    pub description: Option<&'static str>,
    pub kind: ParamKind,
    apply: Applier,
}

impl std::fmt::Debug for ParamDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParamDef")
            .field("key", &self.key)
            .field("label", &self.label)
            .field("kind", &self.kind)
            .finish()
    }
}

impl ParamDef {
    /// The value the control starts with.
    pub fn default_value(&self) -> ParamValue {
        match &self.kind {
            ParamKind::Slider { default, .. } | ParamKind::Numeric { default, .. } => {
                ParamValue::Number(*default)
            }
            ParamKind::Checkbox { default } => ParamValue::Bool(*default),
            ParamKind::Enum { default, .. } => ParamValue::Choice(*default),
            ParamKind::Color { default } => ParamValue::Color(*default),
            ParamKind::Text { default } | ParamKind::FilePath { default, .. } => {
                ParamValue::Text(default.clone())
            }
        }
    }

    /// `EffectParameter.Apply`: a value of the wrong type falls back to the
    /// default, as ShareX's.
    pub(crate) fn apply(&self, effect: &mut dyn Any, value: &ParamValue) {
        let ok = matches!(
            (&self.kind, value),
            (ParamKind::Slider { .. }, ParamValue::Number(_))
                | (ParamKind::Numeric { .. }, ParamValue::Number(_))
                | (ParamKind::Checkbox { .. }, ParamValue::Bool(_))
                | (ParamKind::Color { .. }, ParamValue::Color(_))
                | (ParamKind::Text { .. }, ParamValue::Text(_))
                | (ParamKind::FilePath { .. }, ParamValue::Text(_))
        ) || matches!((&self.kind, value), (ParamKind::Enum { options, .. }, ParamValue::Choice(i)) if *i < options.len());
        if ok {
            (self.apply)(effect, value);
        } else {
            (self.apply)(effect, &self.default_value());
        }
    }

    pub fn tick(mut self, t: f64) -> Self {
        if let ParamKind::Slider { tick, .. } = &mut self.kind {
            *tick = t;
        }
        self
    }

    pub fn snap(mut self, s: bool) -> Self {
        if let ParamKind::Slider { snap, .. } = &mut self.kind {
            *snap = s;
        }
        self
    }

    /// A slider's `valueStringFormat` or a numeric box's `formatString`.
    pub fn format(mut self, f: &'static str) -> Self {
        match &mut self.kind {
            ParamKind::Slider { format, .. } | ParamKind::Numeric { format, .. } => *format = f,
            _ => {}
        }
        self
    }

    pub fn increment(mut self, i: f64) -> Self {
        if let ParamKind::Numeric { increment, .. } = &mut self.kind {
            *increment = i;
        }
        self
    }

    pub fn describe(mut self, d: &'static str) -> Self {
        self.description = Some(d);
        self
    }

    pub fn filter(mut self, f: &'static str) -> Self {
        if let ParamKind::FilePath { filter, .. } = &mut self.kind {
            *filter = Some(f);
        }
        self
    }
}

fn downcast<T: 'static>(e: &mut dyn Any) -> &mut T {
    e.downcast_mut::<T>().expect("parameter applied to another effect")
}

fn number(v: &ParamValue) -> f64 {
    v.as_number().unwrap_or(0.0)
}

fn slider(key: &'static str, label: &'static str, min: f64, max: f64, default: f64, apply: Applier) -> ParamDef {
    ParamDef {
        key,
        label,
        description: None,
        kind: ParamKind::Slider {
            min,
            max,
            default,
            tick: 1.0,
            snap: true,
            format: "{}{0:0}",
        },
        apply,
    }
}

/// `EffectParameters.IntSlider`: `(int)Math.Round(value)`.
pub fn int_slider<T: 'static>(
    key: &'static str,
    label: &'static str,
    min: i32,
    max: i32,
    default: i32,
    f: fn(&mut T, i32),
) -> ParamDef {
    slider(key, label, min as f64, max as f64, default as f64,
        Box::new(move |e, v| f(downcast::<T>(e), number(v).round_ties_even() as i32)))
}

/// `EffectParameters.FloatSlider`: `(float)value`.
pub fn float_slider<T: 'static>(
    key: &'static str,
    label: &'static str,
    min: f64,
    max: f64,
    default: f64,
    f: fn(&mut T, f32),
) -> ParamDef {
    slider(key, label, min, max, default, Box::new(move |e, v| f(downcast::<T>(e), number(v) as f32)))
}

/// `EffectParameters.Bool`.
pub fn bool_param<T: 'static>(key: &'static str, label: &'static str, default: bool, f: fn(&mut T, bool)) -> ParamDef {
    ParamDef {
        key,
        label,
        description: None,
        kind: ParamKind::Checkbox { default },
        apply: Box::new(move |e, v| f(downcast::<T>(e), matches!(v, ParamValue::Bool(true)))),
    }
}

/// `EffectParameters.Enum`: the options in order, the default by value.
pub fn enum_param<T: 'static, E: Copy + PartialEq + Send + Sync + 'static>(
    key: &'static str,
    label: &'static str,
    default: E,
    f: fn(&mut T, E),
    options: &[(&'static str, E)],
) -> ParamDef {
    let default_index = options
        .iter()
        .position(|(_, v)| *v == default)
        .expect("enum default value must be present in the options list");
    let values: Vec<E> = options.iter().map(|(_, v)| *v).collect();
    ParamDef {
        key,
        label,
        description: None,
        kind: ParamKind::Enum {
            options: options.iter().map(|(l, _)| *l).collect(),
            default: default_index,
        },
        apply: Box::new(move |e, v| {
            let i = match v {
                ParamValue::Choice(i) => *i,
                _ => default_index,
            };
            f(downcast::<T>(e), values[i]);
        }),
    }
}

/// `EffectParameters.Color`.
pub fn color_param<T: 'static>(key: &'static str, label: &'static str, default: SkColor, f: fn(&mut T, SkColor)) -> ParamDef {
    ParamDef {
        key,
        label,
        description: None,
        kind: ParamKind::Color { default },
        apply: Box::new(move |e, v| {
            if let ParamValue::Color(c) = v {
                f(downcast::<T>(e), *c);
            }
        }),
    }
}

fn numeric(key: &'static str, label: &'static str, min: f64, max: f64, default: f64, increment: f64, format: &'static str, apply: Applier) -> ParamDef {
    ParamDef {
        key,
        label,
        description: None,
        kind: ParamKind::Numeric {
            min,
            max,
            default,
            increment,
            format,
        },
        apply,
    }
}

/// `EffectParameters.IntNumeric`: `decimal.Round(value, 0, AwayFromZero)`.
pub fn int_numeric<T: 'static>(
    key: &'static str,
    label: &'static str,
    min: i32,
    max: i32,
    default: i32,
    f: fn(&mut T, i32),
) -> ParamDef {
    numeric(key, label, min as f64, max as f64, default as f64, 1.0, "0",
        Box::new(move |e, v| f(downcast::<T>(e), number(v).round() as i32)))
}

/// `EffectParameters.DoubleNumeric`.
pub fn double_numeric<T: 'static>(
    key: &'static str,
    label: &'static str,
    min: f64,
    max: f64,
    default: f64,
    f: fn(&mut T, f64),
) -> ParamDef {
    numeric(key, label, min, max, default, 1.0, "0.0", Box::new(move |e, v| f(downcast::<T>(e), number(v))))
}

/// `EffectParameters.Text`.
pub fn text_param<T: 'static>(key: &'static str, label: &'static str, default: &str, f: fn(&mut T, String)) -> ParamDef {
    ParamDef {
        key,
        label,
        description: None,
        kind: ParamKind::Text {
            default: default.to_string(),
        },
        apply: Box::new(move |e, v| {
            if let ParamValue::Text(s) = v {
                f(downcast::<T>(e), s.clone());
            }
        }),
    }
}

/// `EffectParameters.FilePath`.
pub fn file_param<T: 'static>(key: &'static str, label: &'static str, default: &str, f: fn(&mut T, String)) -> ParamDef {
    ParamDef {
        key,
        label,
        description: None,
        kind: ParamKind::FilePath {
            default: default.to_string(),
            filter: None,
        },
        apply: Box::new(move |e, v| {
            if let ParamValue::Text(s) = v {
                f(downcast::<T>(e), s.clone());
            }
        }),
    }
}

static CHANGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// The values an effect dialog holds (`EffectParameterState`s), in the
/// definition's parameter order. Each records when it last changed
/// (`NumericParameterState.LastChangedSequence`), which the Resize image
/// effect uses to keep the width or the height.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectValues {
    keys: Vec<&'static str>,
    values: Vec<ParamValue>,
    changed: Vec<u64>,
}

impl EffectValues {
    /// Every parameter at its default (`EffectParameterState.Create`).
    pub fn defaults(params: &[ParamDef]) -> Self {
        EffectValues {
            keys: params.iter().map(|p| p.key).collect(),
            values: params.iter().map(ParamDef::default_value).collect(),
            changed: vec![0; params.len()],
        }
    }

    pub fn get(&self, key: &str) -> Option<&ParamValue> {
        self.keys.iter().position(|k| *k == key).map(|i| &self.values[i])
    }

    pub fn values(&self) -> impl Iterator<Item = (&'static str, &ParamValue)> {
        self.keys.iter().copied().zip(self.values.iter())
    }

    /// Sets a value as its control does; a change (not the same value)
    /// takes the next change sequence. Returns false for an unknown key.
    pub fn set(&mut self, key: &str, value: ParamValue) -> bool {
        let Some(i) = self.keys.iter().position(|k| *k == key) else {
            return false;
        };
        if self.values[i] != value {
            self.values[i] = value;
            self.changed[i] = CHANGE_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
        }
        true
    }

    /// When the value last changed (0: never).
    pub fn last_changed(&self, key: &str) -> u64 {
        self.keys
            .iter()
            .position(|k| k.eq_ignore_ascii_case(key))
            .map_or(0, |i| self.changed[i])
    }

    pub(crate) fn value_at(&self, i: usize) -> &ParamValue {
        &self.values[i]
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
