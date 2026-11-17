//! Defaults for the settings classes ShareXrs keeps as plain JSON until the
//! tools that own them are ported (`OCROptions`, the
//! tools' options, ...): exactly what ShareX's serializer writes for a new
//! instance, taken from a fresh ShareX profile (`defaults.json`).
//!
//! When a tool is ported, its options become a [`crate::settings!`] class
//! and leave this file.

use std::sync::OnceLock;

use serde_json::{Map, Value};

static DEFAULTS: OnceLock<Map<String, Value>> = OnceLock::new();

/// ShareX's serialised default for `class` (a key of `defaults.json`).
pub fn json(class: &str) -> Value {
    let defaults = DEFAULTS.get_or_init(|| serde_json::from_str(include_str!("defaults.json")).expect("defaults.json"));
    defaults.get(class).cloned().unwrap_or_else(|| panic!("no default for {class}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_default_parses() {
        for class in ["ImageEditorOptions", "TimeZoneInfo.Utc", "ProxyInfo"] {
            assert!(super::json(class).is_object(), "{class}");
        }
        assert!(super::json("NotificationActionButton.CreateDefaultButtons").is_array());
    }
}
