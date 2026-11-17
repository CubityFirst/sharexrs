//! `HistoryItem.Tags`: a `Dictionary<string, string>` (values may be null,
//! `Favorite` is one), kept in the order .NET's `Dictionary` enumerates it
//! (insertion order, but a removed entry's slot is reused by the next
//! addition, most recently freed first), and stored as Json.NET writes it
//! (`JsonConvert.SerializeObject`) in History.db's `Tags` column.

use serde_json::Value;

/// A `Dictionary<string, string>` with .NET's enumeration order and the
/// default (ordinal, case-sensitive) key comparer.
#[derive(Clone, Debug, Default)]
pub struct Tags {
    slots: Vec<Option<(String, Option<String>)>>,
    /// Freed slots, most recent last (`_freeList`).
    free: Vec<usize>,
}

impl PartialEq for Tags {
    /// The same pairs in the same order.
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Tags {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn position(&self, key: &str) -> Option<usize> {
        self.slots.iter().position(|s| s.as_ref().is_some_and(|(k, _)| k == key))
    }

    /// `ContainsKey`.
    pub fn contains_key(&self, key: &str) -> bool {
        self.position(key).is_some()
    }

    /// `TryGetValue`: `None` when missing, `Some(None)` for a null value.
    pub fn get(&self, key: &str) -> Option<Option<&str>> {
        self.position(key).and_then(|i| self.slots[i].as_ref()).map(|(_, v)| v.as_deref())
    }

    /// The indexer's setter: replaces a value in place, or adds the pair in
    /// the most recently freed slot (else at the end).
    pub fn set(&mut self, key: &str, value: Option<String>) {
        if let Some(i) = self.position(key) {
            self.slots[i].as_mut().unwrap().1 = value;
        } else if let Some(i) = self.free.pop() {
            self.slots[i] = Some((key.to_string(), value));
        } else {
            self.slots.push(Some((key.to_string(), value)));
        }
    }

    /// `Remove`.
    pub fn remove(&mut self, key: &str) -> bool {
        match self.position(key) {
            Some(i) => {
                self.slots[i] = None;
                self.free.push(i);
                true
            }
            None => false,
        }
    }

    /// The pairs in enumeration order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Option<&str>)> {
        self.slots.iter().flatten().map(|(k, v)| (k.as_str(), v.as_deref()))
    }

    /// `JsonConvert.SerializeObject(tags)`: `{"key":"value","Favorite":null}`.
    pub fn to_json(&self) -> String {
        let mut s = String::from("{");
        for (i, (k, v)) in self.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            write_json_string(&mut s, k);
            s.push(':');
            match v {
                Some(v) => write_json_string(&mut s, v),
                None => s.push_str("null"),
            }
        }
        s.push('}');
        s
    }

    /// `JsonConvert.DeserializeObject<Dictionary<string, string>>(json)`:
    /// `None` for an empty string or `null`; a repeated key keeps its first
    /// place and its last value; numbers and booleans become their text.
    pub fn from_json(json: &str) -> Result<Option<Tags>, serde_json::Error> {
        if json.trim().is_empty() {
            return Ok(None);
        }
        let value: Value = serde_json::from_str(json)?;
        Ok(Self::from_value(&value))
    }

    /// From a parsed JSON value (an object, else `None`).
    pub fn from_value(value: &Value) -> Option<Tags> {
        let map = value.as_object()?;
        let mut tags = Tags::new();
        for (k, v) in map {
            let v = match v {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                // Json.NET gives a boolean token's JSON text.
                Value::Bool(b) => Some(b.to_string()),
                other => Some(other.to_string()),
            };
            tags.set(k, v);
        }
        Some(tags)
    }

    pub fn to_value(&self) -> Value {
        Value::Object(
            self.iter()
                .map(|(k, v)| (k.to_string(), v.map_or(Value::Null, |v| Value::String(v.to_string()))))
                .collect(),
        )
    }
}

impl<'a> FromIterator<(&'a str, Option<&'a str>)> for Tags {
    fn from_iter<I: IntoIterator<Item = (&'a str, Option<&'a str>)>>(iter: I) -> Self {
        let mut t = Tags::new();
        for (k, v) in iter {
            t.set(k, v.map(str::to_string));
        }
        t
    }
}

/// A JSON string as Json.NET writes it (`StringEscapeHandling.Default`):
/// `"` and `\` escaped, `\b \t \n \f \r` short, the other control
/// characters and U+0085, U+2028, U+2029 as lowercase `\uXXXX`; everything
/// else as is.
pub fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c == '\u{85}' || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freed_slots_are_reused() {
        let mut t = Tags::new();
        t.set("WindowTitle", Some("a".into()));
        t.set("ProcessName", Some("b".into()));
        t.set("Favorite", None);
        t.remove("WindowTitle");
        t.set("Tag", Some("x".into()));
        assert_eq!(t.to_json(), r#"{"Tag":"x","ProcessName":"b","Favorite":null}"#);
        t.remove("Tag");
        t.remove("Favorite");
        t.set("A", None);
        t.set("B", None);
        assert_eq!(t.iter().map(|(k, _)| k).collect::<Vec<_>>(), ["B", "ProcessName", "A"]);
    }

    #[test]
    fn json_like_json_net() {
        let t: Tags = [("k", Some("q\"\\\u{1}\u{1f}\t\u{85}\u{2028}é<>&'"))].into_iter().collect();
        assert_eq!(t.to_json(), "{\"k\":\"q\\\"\\\\\\u0001\\u001f\\t\\u0085\\u2028é<>&'\"}");
        let back = Tags::from_json(&t.to_json()).unwrap().unwrap();
        assert_eq!(back, t);
        assert_eq!(Tags::from_json("").unwrap(), None);
        assert_eq!(Tags::from_json("null").unwrap(), None);
        assert!(Tags::from_json("{}").unwrap().unwrap().is_empty());
        let dup = Tags::from_json(r#"{"a":"1","b":"2","a":"3"}"#).unwrap().unwrap();
        assert_eq!(dup.to_json(), r#"{"a":"3","b":"2"}"#);
    }
}
