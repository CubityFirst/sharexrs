//! Reading the tools' options from ShareX's JSON (`TaskSettingsTools`) as
//! Newtonsoft does in ShareX's settings: enums by name (any case) or number,
//! and a value that doesn't convert keeps its default.

use serde_json::Value;

/// An enum value: its index in `names` (by name, ignoring case, or by number).
pub fn enum_name(v: &Value, names: &[&str]) -> Option<usize> {
    match v {
        Value::String(s) => names.iter().position(|n| n.eq_ignore_ascii_case(s.trim())),
        Value::Number(n) => n.as_u64().map(|n| n as usize).filter(|n| *n < names.len()),
        _ => None,
    }
}

pub fn json_i32(v: &Value) -> Option<i32> {
    match v {
        Value::Number(n) => n.as_i64().and_then(|n| i32::try_from(n).ok()),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

pub fn json_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// A string property (`null` stays `None`).
pub fn json_string(v: &Value) -> Option<Option<String>> {
    match v {
        Value::String(s) => Some(Some(s.clone())),
        Value::Null => Some(None),
        Value::Number(n) => Some(Some(n.to_string())),
        Value::Bool(b) => Some(Some(if *b { "True" } else { "False" }.into())),
        _ => None,
    }
}

/// A string property, or `default` when it's missing, null or not a string.
pub fn json_str_or(v: &Value, default: &str) -> String {
    match json_string(v) {
        Some(Some(s)) => s,
        _ => default.to_string(),
    }
}
