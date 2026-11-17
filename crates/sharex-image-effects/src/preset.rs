//! `ImageEffectPreset` and `ImageEffect`'s common properties, in ShareX's JSON.
//!
//! Two forms of the same JSON exist: the settings form (`TaskSettings.ImageSettings
//! .ImageEffectPresets` in `ApplicationConfig.json`), where each effect's `$type` is
//! `"ShareX.ImageEffectsLib.Canvas, ShareX.ImageEffectsLib"`, and the export form
//! (`.sxie` packages and the window's import/export, through
//! `ImageEffectsSerializationBinder`), where it's just `"Canvas"`. Properties are written
//! in declaration order, the effect's own first, then `Enabled`, then `Name` when it
//! isn't empty; every other property is always written.

use serde_json::{Map, Value};

use crate::Bitmap;
use crate::dotnet::{Margins, Point, Prop};
use crate::effects::{self, Context, EffectKind};
use crate::random::RandomFast;

const NAMESPACE: &str = "ShareX.ImageEffectsLib.";
const ASSEMBLY: &str = ", ShareX.ImageEffectsLib";

/// How effect types are named in the JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeNames {
    /// `"ShareX.ImageEffectsLib.Canvas, ShareX.ImageEffectsLib"` (settings files).
    Full,
    /// `"Canvas"` (`ImageEffectsSerializationBinder`: exports and `.sxie`).
    Short,
}

/// One effect in a preset.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageEffect {
    pub enabled: bool,
    /// The user's name for it (empty: the type's name and summary are shown). `None`
    /// is .NET's null: effects without a constructor (`BlackWhite`, `Inverse`, ...)
    /// never set it, and Newtonsoft writes it as `"Name": null`.
    pub name: Option<String>,
    pub kind: EffectKind,
}

impl ImageEffect {
    pub fn new(kind: EffectKind) -> Self {
        let name = (!kind.name_defaults_to_null()).then(String::new);
        ImageEffect {
            enabled: true,
            name,
            kind,
        }
    }

    /// `ImageEffect.ToString()`: the name, else "Type name: summary".
    pub fn label(&self) -> String {
        if let Some(n) = self.name.as_ref().filter(|n| !n.is_empty()) {
            return n.clone();
        }
        let name = self.kind.display_name().to_string();
        match self.kind.summary() {
            Some(s) if !s.is_empty() => format!("{name}: {s}"),
            _ => name,
        }
    }

    pub fn to_json(&self, names: TypeNames) -> Value {
        let mut map = Map::new();
        let t = self.kind.type_name();
        let type_name = match names {
            TypeNames::Full => format!("{NAMESPACE}{t}{ASSEMBLY}"),
            TypeNames::Short => t.to_string(),
        };
        map.insert("$type".into(), Value::String(type_name));
        self.kind.write_props(&mut map);
        map.insert("Enabled".into(), Value::Bool(self.enabled));
        // `DefaultValueHandling.Ignore` against `[DefaultValue("")]`.
        match &self.name {
            Some(n) if n.is_empty() => {}
            Some(n) => {
                map.insert("Name".into(), Value::String(n.clone()));
            }
            None => {
                map.insert("Name".into(), Value::Null);
            }
        }
        Value::Object(map)
    }

    /// Reads an effect as Newtonsoft does: the constructor's defaults, then each known
    /// property through its setter; unknown properties and values that don't convert
    /// are skipped. Both type name forms are accepted. `None` for an unknown type.
    pub fn from_json(v: &Value, rng: &mut RandomFast) -> Option<Self> {
        let obj = v.as_object()?;
        let t = obj.get("$type")?.as_str()?;
        let t = t.split(',').next().unwrap_or(t).trim();
        let t = t.strip_prefix(NAMESPACE).unwrap_or(t);
        let mut kind = EffectKind::create(t, rng)?;
        let mut effect = ImageEffect::new(kind.clone());
        for (key, value) in obj {
            if key == "$type" {
                continue;
            }
            if key.eq_ignore_ascii_case("Enabled") {
                if let Some(b) = bool::from_json(value) {
                    effect.enabled = b;
                }
            } else if key.eq_ignore_ascii_case("Name") {
                match value {
                    Value::Null => effect.name = None,
                    v => {
                        if let Some(s) = String::from_json(v) {
                            effect.name = Some(s);
                        }
                    }
                }
            } else {
                kind.set_property(key, value);
            }
        }
        effect.kind = kind;
        Some(effect)
    }
}

/// `ImageEffectPreset`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImageEffectPreset {
    pub name: String,
    pub effects: Vec<ImageEffect>,
}

impl ImageEffectPreset {
    /// `ImageEffectPreset.GetDefaultPreset()`: a 30 px canvas below the image and a
    /// gradient text watermark in it.
    pub fn default_preset() -> Self {
        let canvas = effects::CanvasEffect {
            Margin: Margins::new(0, 0, 0, 30),
            ..Default::default()
        };
        let text = effects::DrawText {
            Offset: Point::new(0, 0),
            UseGradient: true,
            ..Default::default()
        };
        ImageEffectPreset {
            name: String::new(),
            effects: vec![
                ImageEffect::new(EffectKind::Canvas(canvas)),
                ImageEffect::new(EffectKind::DrawText(text)),
            ],
        }
    }

    /// `ToString()`: the name, or "Name" when it has none.
    pub fn label(&self) -> &str {
        if self.name.is_empty() {
            "Name"
        } else {
            &self.name
        }
    }

    pub fn to_json(&self, names: TypeNames) -> Value {
        let mut map = Map::new();
        map.insert("Name".into(), Value::String(self.name.clone()));
        map.insert(
            "Effects".into(),
            Value::Array(self.effects.iter().map(|e| e.to_json(names)).collect()),
        );
        Value::Object(map)
    }

    /// As `JsonHelpers.SerializeToString` writes it: indented, `Environment.NewLine`.
    pub fn to_json_string(&self, names: TypeNames) -> String {
        let text = serde_json::to_string_pretty(&self.to_json(names)).expect("preset serialises");
        if cfg!(windows) {
            text.replace('\n', "\r\n")
        } else {
            text
        }
    }

    /// Reads a preset; effects of unknown types are dropped (as ShareX's error handler
    /// does).
    pub fn from_json(v: &Value) -> Option<Self> {
        Self::from_json_with(v, &mut RandomFast::new())
    }

    pub fn from_json_with(v: &Value, rng: &mut RandomFast) -> Option<Self> {
        let obj = v.as_object()?;
        let mut preset = ImageEffectPreset::default();
        for (key, value) in obj {
            if key.eq_ignore_ascii_case("Name") {
                preset.name = String::from_json(value).unwrap_or_default();
            } else if let (true, Value::Array(items)) = (key.eq_ignore_ascii_case("Effects"), value)
            {
                preset.effects = items
                    .iter()
                    .filter_map(|e| ImageEffect::from_json(e, rng))
                    .collect();
            }
        }
        Some(preset)
    }

    pub fn from_json_str(json: &str) -> Option<Self> {
        let json = json.trim_start_matches('\u{feff}');
        Self::from_json(&serde_json::from_str(json).ok()?)
    }

    /// `ImageEffectPreset.ApplyEffects`: the enabled effects in order on a copy;
    /// `None` if an effect returned null (ShareX stops there and returns null).
    pub fn apply(&self, bmp: &Bitmap, ctx: &mut Context) -> Option<Bitmap> {
        let mut result = bmp.clone();
        for e in self.effects.iter().filter(|e| e.enabled) {
            result = e.kind.apply(result, ctx)?;
        }
        Some(result)
    }
}

/// The task settings ShareX's after-capture "Add image effects" reads
/// (`TaskSettingsImage`).
pub struct ImageEffectsSettings<'a> {
    pub presets: &'a [ImageEffectPreset],
    pub selected: i32,
    pub use_random: bool,
}

/// `TaskHelpers.ApplyImageEffects` without the window (`ShowImageEffectsWindowAfterCapture`
/// is the caller's): the random or selected preset applied to `bmp`. Returns the image
/// unchanged when there's no preset; `None` where ShareX's result would be null.
pub fn apply_image_effects(
    bmp: Bitmap,
    settings: &ImageEffectsSettings,
    ctx: &mut Context,
) -> Option<Bitmap> {
    let preset = if settings.use_random {
        if settings.presets.is_empty() {
            None
        } else {
            Some(ctx.random.pick(settings.presets))
        }
    } else if settings.selected >= 0 && (settings.selected as usize) < settings.presets.len() {
        Some(&settings.presets[settings.selected as usize])
    } else {
        None
    };
    match preset {
        Some(p) => p.apply(&bmp, ctx),
        None => Some(bmp),
    }
}

/// `ImageEffectsViewModel.SanitizeImportedPreset`: an imported preset may only load
/// files from the ImageEffects folder.
pub fn sanitize_imported(
    preset: &mut ImageEffectPreset,
    image_effects_folder: &std::path::Path,
    ctx: &Context,
) {
    let allowed =
        |p: &str| p.is_empty() || crate::paths::is_path_in_folder(p, image_effects_folder, ctx);
    for e in &mut preset.effects {
        match &mut e.kind {
            EffectKind::DrawImage(d) if !allowed(&d.ImageLocation) => d.ImageLocation.clear(),
            EffectKind::DrawBackgroundImage(d) if !allowed(&d.ImageFilePath) => {
                d.ImageFilePath.clear()
            }
            EffectKind::DrawParticles(d) if !allowed(&d.ImageFolder) => d.ImageFolder.clear(),
            _ => {}
        }
    }
}
