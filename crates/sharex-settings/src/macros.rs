//! The macros the settings classes are declared with.
//!
//! ShareX serialises its settings with Newtonsoft.Json (`SettingsBase.SaveToStream`
//! and `LoadInternal`): public fields and properties in declaration order,
//! enums by name (`StringEnumConverter`), and on load a value that doesn't
//! convert keeps its default (`Serializer_Error`). Property names match
//! case-insensitively when there's no exact match, as in Newtonsoft.
//!
//! [`settings!`] declares a class the same way: fields named as ShareX names
//! them (so `DefaultTaskSettings`, not `default_task_settings`), each with
//! ShareX's default, plus `extra`, which keeps the properties this version
//! doesn't know (a newer ShareX's), so saving doesn't lose them.

use serde_json::{Map, Value};

/// Removes `name` from a JSON object: the exact key, or failing that one
/// that matches ignoring case (Newtonsoft's property matching).
#[doc(hidden)]
pub fn take(map: &mut Map<String, Value>, name: &str) -> Option<Value> {
    if let Some(v) = map.shift_remove(name) {
        return Some(v);
    }
    let key = map.keys().find(|k| k.eq_ignore_ascii_case(name))?.clone();
    map.shift_remove(&key)
}

/// Declares a settings class: a struct with ShareX's field names and
/// defaults, serialised in field order, read leniently (a value of the wrong
/// type keeps its default), with unknown properties kept in `extra`.
#[macro_export]
macro_rules! settings {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $( $(#[$fmeta:meta])* $field:ident : $ty:ty = $default:expr ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[allow(non_snake_case)]
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name {
            $( $(#[$fmeta])* pub $field: $ty, )*
            /// Properties this version doesn't know, written back unchanged.
            pub extra: ::serde_json::Map<String, ::serde_json::Value>,
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $( $field: $default, )* extra: ::serde_json::Map::new() }
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                use ::serde::ser::SerializeMap;
                let mut m = s.serialize_map(None)?;
                $( m.serialize_entry(stringify!($field), &self.$field)?; )*
                for (k, v) in &self.extra {
                    m.serialize_entry(k, v)?;
                }
                m.end()
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                #[allow(unused_mut)]
                let mut map = ::serde_json::Map::<String, ::serde_json::Value>::deserialize(d)?;
                let mut out = Self::default();
                $(
                    if let Some(v) = $crate::macros::take(&mut map, stringify!($field))
                        && let Ok(v) = <$ty as ::serde::Deserialize>::deserialize(v)
                    {
                        out.$field = v;
                    }
                )*
                out.extra = map;
                Ok(out)
            }
        }
    };
}

/// Declares a .NET enum, written by name as `StringEnumConverter` does and
/// read from a name (any case) or a number. A value this version doesn't
/// know is kept as written (`Unknown`). Variants without values number from
/// 0, as in C#.
#[macro_export]
macro_rules! dotnet_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident { $( $(#[$vmeta:meta])* $variant:ident ),* $(,)? }
    ) => {
        $(#[$meta])*
        #[allow(non_camel_case_types)]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
            /// A value this version doesn't know (a newer ShareX's), as written.
            Unknown(String),
        }

        impl $name {
            /// Every known value, in declaration order (index = value).
            pub const ALL: &'static [$name] = &[ $( $name::$variant, )* ];

            /// The C# name, as ShareX writes it.
            pub fn name(&self) -> &str {
                match self {
                    $( $name::$variant => stringify!($variant), )*
                    $name::Unknown(s) => s,
                }
            }

            /// The numeric value.
            pub fn value(&self) -> Option<i64> {
                Self::ALL.iter().position(|v| v == self).map(|i| i as i64)
            }

            fn from_value(value: i64) -> Option<Self> {
                usize::try_from(value).ok().and_then(|i| Self::ALL.get(i)).cloned()
            }

            fn from_name(name: &str) -> Option<Self> {
                Self::ALL.iter().find(|v| v.name().eq_ignore_ascii_case(name)).cloned()
            }
        }

        $crate::macros::impl_enum_serde!($name);
    };
}

/// [`dotnet_enum!`] for enums with explicit values.
#[macro_export]
macro_rules! dotnet_enum_values {
    (
        $(#[$meta:meta])*
        pub enum $name:ident { $( $(#[$vmeta:meta])* $variant:ident = $value:expr ),* $(,)? }
    ) => {
        $(#[$meta])*
        #[allow(non_camel_case_types)]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
            /// A value this version doesn't know (a newer ShareX's), as written.
            Unknown(String),
        }

        impl $name {
            pub const ALL: &'static [$name] = &[ $( $name::$variant, )* ];

            pub fn name(&self) -> &str {
                match self {
                    $( $name::$variant => stringify!($variant), )*
                    $name::Unknown(s) => s,
                }
            }

            pub fn value(&self) -> Option<i64> {
                match self {
                    $( $name::$variant => Some($value), )*
                    $name::Unknown(_) => None,
                }
            }

            fn from_value(value: i64) -> Option<Self> {
                Self::ALL.iter().find(|v| v.value() == Some(value)).cloned()
            }

            fn from_name(name: &str) -> Option<Self> {
                Self::ALL.iter().find(|v| v.name().eq_ignore_ascii_case(name)).cloned()
            }
        }

        $crate::macros::impl_enum_serde!($name);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __impl_enum_serde {
    ($name:ident) => {
        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                match self {
                    // An undefined value Newtonsoft read from a number goes back as one.
                    $name::Unknown(raw) => match raw.parse::<i64>() {
                        Ok(n) => s.serialize_i64(n),
                        Err(_) => s.serialize_str(raw),
                    },
                    known => s.serialize_str(known.name()),
                }
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                match ::serde_json::Value::deserialize(d)? {
                    ::serde_json::Value::String(s) => Ok(Self::from_name(s.trim())
                        .or_else(|| s.trim().parse::<i64>().ok().and_then(Self::from_value))
                        .unwrap_or($name::Unknown(s))),
                    ::serde_json::Value::Number(n) => {
                        let n = n.as_i64().ok_or_else(|| ::serde::de::Error::custom("not an integer"))?;
                        Ok(Self::from_value(n).unwrap_or_else(|| $name::Unknown(n.to_string())))
                    }
                    _ => Err(::serde::de::Error::custom(concat!("not a ", stringify!($name)))),
                }
            }
        }
    };
}

#[doc(hidden)]
pub use crate::__impl_enum_serde as impl_enum_serde;

/// Declares a .NET `[Flags]` enum: a bit set, written as `Enum.ToString()`
/// writes it ("CopyImageToClipboard, SaveImageToFile"), read from names
/// (any case, aliases too) separated by commas, or a number.
///
/// `names` is every distinct value with the name .NET picks for it (for
/// duplicate values, the one `ToString()` prints), `aliases` the other names.
#[macro_export]
macro_rules! dotnet_flags {
    (
        $(#[$meta:meta])*
        pub struct $name:ident: $int:ty {
            $( $(#[$vmeta:meta])* const $flag:ident = $value:expr; )*
        }
        $( aliases { $( const $alias:ident = $avalue:expr; )* } )?
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $name(pub $int);

        #[allow(non_upper_case_globals)]
        impl $name {
            $( $(#[$vmeta])* pub const $flag: $name = $name($value as $int); )*
            $( $( pub const $alias: $name = $name($avalue as $int); )* )?

            /// (value, name) for every distinct value, as .NET formats them.
            pub const NAMES: &'static [(u64, &'static str)] = &[ $( ($value as $int as u64, stringify!($flag)), )* ];
            /// The other names values go by, accepted when reading.
            pub const ALIASES: &'static [(u64, &'static str)] = &[ $( $( ($avalue as $int as u64, stringify!($alias)), )* )? ];

            pub fn contains(self, other: $name) -> bool {
                self.0 & other.0 == other.0
            }

            pub fn insert(&mut self, other: $name) {
                self.0 |= other.0;
            }

            pub fn remove(&mut self, other: $name) {
                self.0 &= !other.0;
            }

            /// `Enum.ToString()`.
            pub fn to_dotnet_string(self) -> String {
                $crate::macros::format_flags(self.0 as u64, Self::NAMES)
            }

            /// `Enum.Parse(text, ignoreCase: true)`.
            pub fn parse(text: &str) -> Option<Self> {
                $crate::macros::parse_flags(text, Self::NAMES, Self::ALIASES).map(|v| $name(v as $int))
            }
        }

        impl std::ops::BitOr for $name {
            type Output = $name;
            fn bitor(self, rhs: $name) -> $name {
                $name(self.0 | rhs.0)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let text = self.to_dotnet_string();
                match text.parse::<i64>() {
                    Ok(n) => s.serialize_i64(n),
                    Err(_) => s.serialize_str(&text),
                }
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                match ::serde_json::Value::deserialize(d)? {
                    ::serde_json::Value::String(s) => {
                        Self::parse(&s).ok_or_else(|| ::serde::de::Error::custom(format!("not a {}: {s}", stringify!($name))))
                    }
                    ::serde_json::Value::Number(n) => n
                        .as_i64()
                        .map(|v| $name(v as $int))
                        .ok_or_else(|| ::serde::de::Error::custom("not an integer")),
                    _ => Err(::serde::de::Error::custom(concat!("not a ", stringify!($name)))),
                }
            }
        }
    };
}

/// .NET's `Enum.ToString()` for a `[Flags]` enum (`EnumInfo` formatting): the
/// name of a defined value, else the names whose bits make it up, picked
/// from the highest value down and listed lowest first, else the number.
pub fn format_flags(value: u64, names: &[(u64, &str)]) -> String {
    if let Some((_, name)) = names.iter().find(|(v, _)| *v == value) {
        return (*name).to_owned();
    }
    let mut sorted: Vec<(u64, &str)> = names.to_vec();
    sorted.sort_by_key(|(v, _)| *v);
    let mut rest = value;
    let mut found = Vec::new();
    for (v, name) in sorted.iter().rev() {
        if *v != 0 && rest & v == *v {
            rest -= v;
            found.push(*name);
        }
        if rest == 0 {
            break;
        }
    }
    if rest != 0 || found.is_empty() {
        // .NET prints the signed value of the underlying type.
        return (value as u32 as i32).to_string();
    }
    found.reverse();
    found.join(", ")
}

/// `Enum.Parse(text, ignoreCase: true)` for a `[Flags]` enum.
pub fn parse_flags(text: &str, names: &[(u64, &str)], aliases: &[(u64, &str)]) -> Option<u64> {
    let text = text.trim();
    if let Ok(n) = text.parse::<i64>() {
        return Some(n as u64);
    }
    let mut value = 0u64;
    for part in text.split(',') {
        let part = part.trim();
        let v = names
            .iter()
            .chain(aliases)
            .find(|(_, n)| n.eq_ignore_ascii_case(part))
            .map(|(v, _)| *v)?;
        value |= v;
    }
    Some(value)
}
