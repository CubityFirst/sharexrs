//! The editor's toolbar items and their saved layout:
//! `ToolbarCustomizationItemViewModel` (the definitions, `CreateFromOptions`,
//! `ToOptions`, `ToolTip`), `ImageEditorToolbarItemOptions` (what
//! `ImageEditorOptions.ToolbarItems` and `RegionCaptureOptions.ToolbarItems`
//! keep), region capture's own items (`RegionCaptureToolbar.CreateItems`)
//! and `ToolbarHotkeyHelper`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sharex_image_editor::model::EditorTool;
use sharex_strings::{image_editor as ie, screen_capture_lib as sc};
use sharex_ui::keys::{app_icons, image_editor_icons as icon};

/// `ImageEditorToolbarItemOptions`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[allow(non_snake_case)]
pub struct ItemOptions {
    pub Id: String,
    pub BeginGroup: bool,
    /// Extra space in pixels around the group separator before this item.
    /// Only used when the item begins a group.
    pub SpacerWidth: i32,
    /// Leaves out the line of the group separator before this item, keeping
    /// only its space.
    pub HideSpacerLine: bool,
    pub IsVisible: bool,
    pub Hotkey: String,
    pub SecondaryHotkey: String,
}

/// `ToolbarItemDefinition`.
#[derive(Clone, Debug)]
pub struct Def {
    pub id: &'static str,
    pub tool: Option<EditorTool>,
    pub name: fn() -> &'static str,
    pub icon: &'static str,
    pub default_hotkey: &'static str,
    pub hotkey_editable: bool,
    pub visibility_editable: bool,
    pub begin_group: bool,
    pub default_secondary: &'static str,
    pub visible_default: bool,
    /// Added by the host (region capture's region shapes).
    pub host: bool,
}

const fn tool(id: &'static str, t: EditorTool, name: fn() -> &'static str, icon: &'static str, hotkey: &'static str, secondary: &'static str, begin_group: bool) -> Def {
    Def {
        id,
        tool: Some(t),
        name,
        icon,
        default_hotkey: hotkey,
        hotkey_editable: true,
        visibility_editable: true,
        begin_group,
        default_secondary: secondary,
        visible_default: true,
        host: false,
    }
}

const fn other(id: &'static str, name: fn() -> &'static str, icon: &'static str, hotkey: &'static str, visibility_editable: bool) -> Def {
    Def {
        id,
        tool: None,
        name,
        icon,
        default_hotkey: hotkey,
        hotkey_editable: true,
        visibility_editable,
        begin_group: false,
        default_secondary: "",
        visible_default: true,
        host: false,
    }
}

pub const FILE: &str = "File";
pub const CANVAS_SIZE: &str = "CanvasSize";
pub const BACKGROUND: &str = "Background";
pub const IMAGE_EFFECTS: &str = "ImageEffects";
pub const REGION_ELLIPSE: &str = "RegionEllipse";
pub const REGION_FREEHAND: &str = "RegionFreehand";

/// `ItemDefinitions`, in order.
pub fn editor_defs() -> Vec<Def> {
    use EditorTool as T;
    vec![
        other(FILE, ie::ToolbarCustomizationItemViewModel_File, app_icons::File, "", false),
        tool("Select", T::Select, ie::ToolbarCustomizationItemViewModel_Select, icon::Select, "V", "", true),
        tool("Rectangle", T::Rectangle, ie::ToolbarCustomizationItemViewModel_Rectangle, icon::Rectangle, "R", "NumPad1", false),
        tool("Ellipse", T::Ellipse, ie::ToolbarCustomizationItemViewModel_Ellipse, icon::Ellipse, "E", "NumPad2", false),
        tool("Line", T::Line, ie::ToolbarCustomizationItemViewModel_Line, icon::Line, "L", "NumPad4", false),
        tool("Arrow", T::Arrow, ie::ToolbarCustomizationItemViewModel_Arrow, icon::Arrow, "A", "NumPad5", false),
        tool("Freehand", T::Freehand, ie::ToolbarCustomizationItemViewModel_Freehand, icon::Freehand, "F", "NumPad3", false),
        tool("Text", T::Text, ie::ToolbarCustomizationItemViewModel_Text, icon::Text, "T", "NumPad6", false),
        tool("SpeechBalloon", T::SpeechBalloon, ie::ToolbarCustomizationItemViewModel_SpeechBalloon, icon::SpeechBalloon, "O", "", false),
        tool("Step", T::Step, ie::ToolbarCustomizationItemViewModel_Step, icon::Step, "N", "NumPad7", false),
        tool("Image", T::Image, ie::ToolbarCustomizationItemViewModel_Image, icon::Image, "I", "", false),
        tool("Clone", T::Clone, ie::ToolbarCustomizationItemViewModel_Clone, icon::Clone, "D", "", false),
        tool("Emoji", T::Emoji, ie::ToolbarCustomizationItemViewModel_Emoji, icon::Emoji, "J", "", false),
        tool("Cursor", T::Cursor, ie::ToolbarCustomizationItemViewModel_Cursor, icon::Cursor, "K", "", false),
        tool("Highlight", T::Highlight, ie::ToolbarCustomizationItemViewModel_Highlight, icon::Highlight, "H", "", false),
        tool("SmartEraser", T::SmartEraser, ie::ToolbarCustomizationItemViewModel_SmartEraser, icon::SmartEraser, "W", "", false),
        tool("Blur", T::Blur, ie::ToolbarCustomizationItemViewModel_Blur, icon::Blur, "B", "NumPad8", false),
        tool("Pixelate", T::Pixelate, ie::ToolbarCustomizationItemViewModel_Pixelate, icon::Pixelate, "P", "NumPad9", false),
        tool("Magnify", T::Magnify, ie::ToolbarCustomizationItemViewModel_Magnify, icon::Magnify, "M", "", false),
        tool("Spotlight", T::Spotlight, ie::ToolbarCustomizationItemViewModel_Spotlight, icon::Spotlight, "S", "", false),
        tool("Crop", T::Crop, ie::ToolbarCustomizationItemViewModel_Crop, icon::Crop, "C", "", true),
        tool("CutOut", T::CutOut, ie::ToolbarCustomizationItemViewModel_CutOut, icon::CutOut, "U", "", false),
        other(CANVAS_SIZE, ie::ToolbarCustomizationItemViewModel_CanvasSize, app_icons::Expand, "X", true),
        other(BACKGROUND, ie::ToolbarCustomizationItemViewModel_Background, app_icons::Wallpaper, "", true),
        other(IMAGE_EFFECTS, ie::ToolbarCustomizationItemViewModel_ImageEffects, app_icons::Sparkles, "", true),
    ]
}

/// `RegionCaptureToolbar.CreateRegionItems`: the ShareX 21 ellipse and
/// freehand regions, hidden until shown in the toolbar customisation.
fn region_host_defs() -> Vec<Def> {
    let host = |id, name, icon| Def {
        id,
        tool: None,
        name,
        icon,
        default_hotkey: "",
        hotkey_editable: true,
        visibility_editable: true,
        begin_group: false,
        default_secondary: "",
        visible_default: false,
        host: true,
    };
    vec![
        host(REGION_ELLIPSE, sc::RegionCaptureWindow_EllipseRegion as fn() -> &'static str, icon::RegionEllipse),
        host(REGION_FREEHAND, sc::RegionCaptureWindow_FreehandRegion, icon::RegionFreehand),
    ]
}

/// `MaxSpacerWidth`: the most extra space a group separator can have, in pixels.
pub const MAX_SPACER_WIDTH: i32 = 96;

/// `ClampSpacerWidth`.
pub fn clamp_spacer_width(width: i32) -> i32 {
    width.clamp(0, MAX_SPACER_WIDTH)
}

/// `ToolbarCustomizationItemViewModel`.
#[derive(Clone, Debug)]
pub struct Item {
    pub def: Def,
    pub visible: bool,
    pub begin_group: bool,
    /// `SpacerWidth`: extra space around the group separator before it.
    pub spacer_width: i32,
    /// `ShowSpacerLine`: whether that separator draws its line.
    pub show_spacer_line: bool,
    pub hotkey: String,
    pub secondary_hotkey: String,
}

impl Item {
    /// `CreateItem`.
    #[allow(clippy::too_many_arguments)]
    fn create(
        def: &Def,
        visible: Option<bool>,
        hotkey: Option<&str>,
        secondary: Option<&str>,
        begin_group: Option<bool>,
        spacer_width: i32,
        show_spacer_line: bool,
    ) -> Item {
        Item {
            hotkey: if def.hotkey_editable { hotkey.unwrap_or(def.default_hotkey).to_string() } else { String::new() },
            secondary_hotkey: if def.hotkey_editable { secondary.unwrap_or(def.default_secondary).to_string() } else { String::new() },
            visible: if def.visibility_editable { visible.unwrap_or(def.visible_default) } else { true },
            begin_group: begin_group.unwrap_or(def.begin_group),
            spacer_width: clamp_spacer_width(spacer_width),
            show_spacer_line,
            def: def.clone(),
        }
    }

    /// `IsFileMenu`.
    pub fn is_file_menu(&self) -> bool {
        self.def.id == FILE
    }

    pub fn id(&self) -> &'static str {
        self.def.id
    }

    pub fn name(&self) -> &'static str {
        (self.def.name)()
    }

    /// `Hotkeys`: the non-empty ones, trimmed.
    pub fn hotkeys(&self) -> Vec<String> {
        if !self.def.hotkey_editable {
            return Vec::new();
        }
        [&self.hotkey, &self.secondary_hotkey].into_iter().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect()
    }

    /// `ToolTip`: `Rectangle (R, NumPad1)`.
    pub fn tooltip(&self) -> String {
        let mut keys: Vec<String> = Vec::new();
        for h in self.hotkeys() {
            if !keys.iter().any(|k| k.eq_ignore_ascii_case(&h)) {
                keys.push(h);
            }
        }
        let header = if keys.is_empty() { self.name().to_string() } else { format!("{} ({})", self.name(), keys.join(", ")) };
        if self.def.id == IMAGE_EFFECTS { ie::ToolbarCustomizationItemViewModel_FavoriteImageEffectsRightClick().replace("{0}", &header) } else { header }
    }

    /// `IsGroupSeparatorVisible`.
    pub fn separator(&self) -> bool {
        self.visible && self.begin_group
    }

    /// `SpacerMargin`: the spacer width split to both sides of the
    /// separator's line (left, right).
    pub fn spacer_margin(&self) -> (f32, f32) {
        let w = self.spacer_width;
        ((w / 2) as f32, (w - w / 2) as f32)
    }

    /// The separator's width on the toolbar (its 1px line and the margins
    /// around it), or 0 without one.
    pub fn separator_width(&self) -> f32 {
        if self.separator() { 1.0 + self.spacer_width as f32 } else { 0.0 }
    }

    /// `ToOptions`.
    pub fn to_options(&self) -> ItemOptions {
        ItemOptions {
            Id: self.def.id.to_string(),
            BeginGroup: self.begin_group,
            SpacerWidth: if self.begin_group { self.spacer_width } else { 0 },
            HideSpacerLine: self.begin_group && !self.show_spacer_line,
            IsVisible: !self.def.visibility_editable || self.visible,
            Hotkey: if self.def.hotkey_editable { self.hotkey.trim().to_string() } else { String::new() },
            SecondaryHotkey: if self.def.hotkey_editable { self.secondary_hotkey.trim().to_string() } else { String::new() },
        }
    }

    /// Whether `key` with the modifiers is one of the item's hotkeys
    /// (`ToolbarHotkeyHelper.Matches`).
    pub fn matches(&self, key: &str, mods: Mods) -> bool {
        self.hotkeys().iter().any(|h| hotkey::matches(h, key, mods))
    }
}

/// Keyboard modifiers (`KeyModifiers`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

/// The saved options as JSON (`List<ImageEditorToolbarItemOptions>`).
pub fn parse_options(values: &[Value]) -> Vec<ItemOptions> {
    values.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect()
}

pub fn options_json(items: &[Item]) -> Vec<Value> {
    items.iter().map(|i| serde_json::to_value(i.to_options()).unwrap_or(Value::Null)).collect()
}

/// `CreateFromOptions`: the saved items in their order (unknown ids
/// dropped), every definition they miss inserted after its predecessor.
pub fn from_options(options: &[ItemOptions], defs: &[Def]) -> Vec<Item> {
    let defaults = || defs.iter().map(|d| Item::create(d, None, None, None, None, 0, true)).collect::<Vec<_>>();
    if options.is_empty() {
        return defaults();
    }
    let mut items: Vec<Item> = Vec::new();
    let mut used: Vec<&str> = Vec::new();
    for o in options {
        if let Some(d) = defs.iter().find(|d| d.id.eq_ignore_ascii_case(&o.Id)) {
            items.push(Item::create(d, Some(o.IsVisible), Some(&o.Hotkey), Some(&o.SecondaryHotkey), Some(o.BeginGroup), o.SpacerWidth, !o.HideSpacerLine));
            used.push(d.id);
        }
    }
    for (i, d) in defs.iter().enumerate() {
        if used.iter().any(|u| u.eq_ignore_ascii_case(d.id)) {
            continue;
        }
        let previous = if i > 0 { items.iter().position(|it| it.def.id == defs[i - 1].id).map(|p| p as isize).unwrap_or(-1) } else { -1 };
        items.insert((previous + 1) as usize, Item::create(d, None, None, None, None, 0, true));
    }
    if items.is_empty() { defaults() } else { items }
}

/// The image editor's items from `ImageEditorOptions.ToolbarItems`.
pub fn editor_items(values: &[Value]) -> Vec<Item> {
    from_options(&parse_options(values), &editor_defs())
}

/// The default image editor items (`CreateDefaultItems`).
pub fn editor_default_items() -> Vec<Item> {
    from_options(&[], &editor_defs())
}

/// `RegionCaptureToolbar.CreateItems`: the region shapes first, then the
/// tools (not Crop and Cut out).
pub fn region_items(values: &[Value]) -> Vec<Item> {
    let mut defs = region_host_defs();
    defs.extend(editor_defs());
    from_options(&parse_options(values), &defs)
        .into_iter()
        .filter(|i| i.def.host || i.def.tool.as_ref().is_some_and(|t| !matches!(t, EditorTool::Crop | EditorTool::CutOut)))
        .collect()
}

/// `ToolbarHotkeyHelper`, with Avalonia's `Key` names.
pub mod hotkey {
    use super::Mods;

    /// Whether `name` is one of Avalonia's `Key` names this port knows
    /// (`Enum.TryParse` for the keys ShareXrs can press).
    pub fn key_name(name: &str) -> Option<String> {
        let n = name.trim();
        let u = n.to_ascii_uppercase();
        if u.len() == 1 && u.chars().all(|c| c.is_ascii_uppercase()) {
            return Some(u);
        }
        if let Some(d) = u.strip_prefix('D')
            && d.len() == 1
            && d.chars().all(|c| c.is_ascii_digit())
        {
            return Some(format!("D{d}"));
        }
        if let Some(d) = u.strip_prefix("NUMPAD")
            && d.len() == 1
            && d.chars().all(|c| c.is_ascii_digit())
        {
            return Some(format!("NumPad{d}"));
        }
        if let Some(f) = u.strip_prefix('F')
            && let Ok(v) = f.parse::<u32>()
            && (1..=24).contains(&v)
        {
            return Some(format!("F{v}"));
        }
        const NAMES: [&str; 22] = [
            "Space", "Enter", "Return", "Escape", "Tab", "Back", "Delete", "Insert", "Home", "End", "PageUp", "PageDown", "Left", "Up", "Right",
            "Down", "OemPlus", "OemMinus", "OemComma", "OemPeriod", "Add", "Subtract",
        ];
        NAMES.iter().find(|k| k.eq_ignore_ascii_case(n)).map(|k| k.to_string())
    }

    /// `TryParse`: modifiers (Ctrl/Control, Shift, Alt, Win/Meta) then a key.
    pub fn try_parse(hotkey: &str) -> Option<(String, Mods)> {
        let parts: Vec<&str> = hotkey.split('+').map(str::trim).filter(|s| !s.is_empty()).collect();
        let (last, mods) = parts.split_last()?;
        let mut m = Mods::default();
        for p in mods {
            match p.to_ascii_uppercase().as_str() {
                "CTRL" | "CONTROL" => m.ctrl = true,
                "SHIFT" => m.shift = true,
                "ALT" => m.alt = true,
                "WIN" | "META" => m.meta = true,
                _ => return None,
            }
        }
        Some((key_name(last)?, m))
    }

    /// `Format`: `Ctrl+Shift+Alt+Meta+Key`.
    pub fn format(key: &str, m: Mods) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if m.ctrl {
            parts.push("Ctrl");
        }
        if m.shift {
            parts.push("Shift");
        }
        if m.alt {
            parts.push("Alt");
        }
        if m.meta {
            parts.push("Meta");
        }
        parts.push(key);
        parts.join("+")
    }

    /// `Normalize`.
    pub fn normalize(hotkey: &str) -> String {
        match try_parse(hotkey) {
            Some((k, m)) => format(&k, m),
            None => hotkey.trim().to_string(),
        }
    }

    /// `Matches`.
    pub fn matches(hotkey: &str, key: &str, m: Mods) -> bool {
        try_parse(hotkey).is_some_and(|(k, hm)| k.eq_ignore_ascii_case(key) && hm == m)
    }
}

/// `RefreshValidation`: the first hotkey that doesn't parse, or the first
/// one two items share.
pub fn validation(items: &[Item]) -> Option<String> {
    let mut used: Vec<(String, usize)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        for h in item.hotkeys() {
            if hotkey::try_parse(&h).is_none() {
                return Some(ie::ToolbarCustomizationDialogView_InvalidHotkey().replace("{0}", item.name()));
            }
            let n = hotkey::normalize(&h);
            if let Some((_, j)) = used.iter().find(|(k, j)| k.eq_ignore_ascii_case(&n) && *j != i) {
                return Some(
                    ie::ToolbarCustomizationDialogView_DuplicateHotkey()
                        .replace("{0}", item.name())
                        .replace("{1}", items[*j].name())
                        .replace("{2}", &n),
                );
            }
            used.retain(|(k, _)| !k.eq_ignore_ascii_case(&n));
            used.push((n, i));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_round_trip() {
        let items = editor_default_items();
        assert_eq!(items.len(), 25);
        assert_eq!(items[0].id(), FILE);
        assert!(items[1].begin_group && items[20].begin_group);
        assert_eq!(items[2].tooltip(), "Rectangle (R, NumPad1)");
        let json = options_json(&items);
        assert_eq!(
            json[2],
            serde_json::json!({"Id": "Rectangle", "BeginGroup": false, "SpacerWidth": 0, "HideSpacerLine": false, "IsVisible": true, "Hotkey": "R", "SecondaryHotkey": "NumPad1"})
        );
        let back = editor_items(&json);
        assert_eq!(options_json(&back), json);
        // Region capture: the two hidden region shapes, then the tools.
        let r = region_items(&[]);
        assert_eq!(r.len(), 21);
        assert_eq!(r[0].id(), REGION_ELLIPSE);
        assert!(!r[0].visible);
    }

    /// ShareX #9278: a spacer's width and line are kept on the item after it,
    /// only while it begins a group; old configs without them load as a
    /// plain separator, and widths are clamped.
    #[test]
    fn spacers_round_trip() {
        let saved = vec![
            serde_json::json!({"Id": "File", "BeginGroup": false, "IsVisible": true, "Hotkey": "", "SecondaryHotkey": ""}),
            serde_json::json!({"Id": "Select", "BeginGroup": true, "SpacerWidth": 24, "HideSpacerLine": true, "IsVisible": true, "Hotkey": "V", "SecondaryHotkey": ""}),
            serde_json::json!({"Id": "Rectangle", "BeginGroup": true, "SpacerWidth": 500, "IsVisible": true, "Hotkey": "R", "SecondaryHotkey": ""}),
            serde_json::json!({"Id": "Ellipse", "BeginGroup": false, "SpacerWidth": 30, "HideSpacerLine": true, "IsVisible": true}),
        ];
        let items = editor_items(&saved);
        assert_eq!((items[1].spacer_width, items[1].show_spacer_line), (24, false));
        assert_eq!(items[1].spacer_margin(), (12.0, 12.0));
        assert_eq!(items[1].separator_width(), 25.0);
        assert_eq!((items[2].spacer_width, items[2].show_spacer_line), (MAX_SPACER_WIDTH, true));
        let json = options_json(&items);
        assert_eq!(json[1]["SpacerWidth"], 24);
        assert_eq!(json[1]["HideSpacerLine"], true);
        assert_eq!(json[2]["SpacerWidth"], 96);
        assert_eq!(json[2]["HideSpacerLine"], false);
        // Not a group: neither is saved.
        assert_eq!(json[3]["SpacerWidth"], 0);
        assert_eq!(json[3]["HideSpacerLine"], false);
        // An odd width splits with the extra pixel on the right.
        let mut odd = items[1].clone();
        odd.spacer_width = 5;
        assert_eq!(odd.spacer_margin(), (2.0, 3.0));
        // ShareX's property order.
        let keys: Vec<&str> = json[1].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["Id", "BeginGroup", "SpacerWidth", "HideSpacerLine", "IsVisible", "Hotkey", "SecondaryHotkey"]);
    }

    #[test]
    fn missing_items_go_after_their_predecessor() {
        let opts = vec![ItemOptions { Id: "Arrow".into(), IsVisible: false, ..Default::default() }, ItemOptions { Id: "Nope".into(), ..Default::default() }];
        let items = from_options(&opts, &editor_defs());
        assert_eq!(items[0].id(), FILE);
        assert_eq!(items[5].id(), "Arrow");
        assert!(!items[5].visible);
        assert_eq!(items.len(), 25);
    }

    #[test]
    fn hotkeys() {
        assert_eq!(hotkey::normalize("control+shift+r"), "Ctrl+Shift+R");
        assert!(hotkey::try_parse("Ctrl+").is_none());
        assert!(hotkey::try_parse("Hyper+R").is_none());
        let mut items = editor_default_items();
        assert_eq!(validation(&items), None);
        items[3].hotkey = "r".into();
        assert!(validation(&items).unwrap().contains("Rectangle"));
    }
}
