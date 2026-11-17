//! The .NET value types ShareX's settings hold, serialised as Newtonsoft
//! does: `System.Drawing` structs through their `TypeConverter`
//! (`ConvertToInvariantString`, so "54, 4" in every culture) and `DateTime`
//! as ISO 8601 in UTC (`DateTimeZoneHandling.Utc`).

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn parse_ints<const N: usize>(text: &str) -> Option<[i32; N]> {
    let mut out = [0; N];
    let mut parts = text.split(',');
    for v in &mut out {
        *v = parts.next()?.trim().parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

macro_rules! drawing_struct {
    ($(#[$meta:meta])* $name:ident { $($field:ident),+ }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $name { $(pub $field: i32,)+ }

        impl $name {
            pub const fn new($($field: i32),+) -> Self {
                Self { $($field),+ }
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let parts: Vec<String> = vec![$(self.$field.to_string()),+];
                s.serialize_str(&parts.join(", "))
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = String::deserialize(d)?;
                let [$($field),+] = parse_ints(&text).ok_or_else(|| D::Error::custom(format!("not a {}: {text}", stringify!($name))))?;
                Ok(Self { $($field),+ })
            }
        }
    };
}

drawing_struct!(
    /// `System.Drawing.Point`: "X, Y".
    Point { x, y }
);
drawing_struct!(
    /// `System.Drawing.Size`: "Width, Height".
    Size { width, height }
);
drawing_struct!(
    /// `System.Drawing.Rectangle`: "X, Y, Width, Height".
    Rectangle { x, y, width, height }
);

/// A `DateTime`, as Newtonsoft writes it with `DateTimeZoneHandling.Utc`:
/// "2026-11-17T13:24:02.3185047Z" (100ns ticks, trailing zeros trimmed).
/// Kept as written, so a round trip doesn't change it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DateTime(pub String);

impl DateTime {
    /// `DateTime.Now`, as it will be written.
    pub fn now() -> Self {
        Self::from_utc(chrono::Utc::now())
    }

    pub fn from_utc(t: chrono::DateTime<chrono::Utc>) -> Self {
        // .NET's round-trip format with `DateTimeZoneHandling.Utc`: seconds
        // with up to 7 fractional digits, trailing zeros removed.
        let ticks = t.timestamp_subsec_nanos() / 100;
        let mut text = t.format("%Y-%m-%dT%H:%M:%S").to_string();
        if ticks > 0 {
            let frac = format!("{ticks:07}");
            text.push('.');
            text.push_str(frac.trim_end_matches('0'));
        }
        text.push('Z');
        Self(text)
    }

    pub fn parse(&self) -> Option<chrono::DateTime<chrono::FixedOffset>> {
        chrono::DateTime::parse_from_rfc3339(&self.0).ok()
    }
}

impl Serialize for DateTime {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Ok(Self(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_structs() {
        assert_eq!(serde_json::to_string(&Point::new(54, 4)).unwrap(), r#""54, 4""#);
        assert_eq!(serde_json::from_str::<Size>(r#""200, 150""#).unwrap(), Size::new(200, 150));
        assert_eq!(serde_json::from_str::<Rectangle>(r#""0, 0, 0, 0""#).unwrap(), Rectangle::default());
        assert!(serde_json::from_str::<Point>(r#""1, 2, 3""#).is_err());
    }

    #[test]
    fn date_time_format() {
        use chrono::TimeZone;
        let t = chrono::Utc.with_ymd_and_hms(2026, 11, 17, 13, 24, 2).unwrap() + chrono::Duration::nanoseconds(318_504_700);
        assert_eq!(DateTime::from_utc(t).0, "2026-11-17T13:24:02.3185047Z");
        let t = chrono::Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap() + chrono::Duration::nanoseconds(500_000_000);
        assert_eq!(DateTime::from_utc(t).0, "2026-01-02T03:04:05.5Z");
    }
}
