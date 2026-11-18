//! Versions as ShareX's update check reads and compares them:
//! `System.Version` (`new Version(string)`, `CompareTo`, `ToString`),
//! `VersionExtensions.Normalize` and `Helpers.CompareVersion`
//! (`ShareX.HelpersLib/Helpers/Helpers.cs`, `Extensions/Extensions.cs`).
//!
//! ShareX's tags are plain `v<major>.<minor>.<build>`. ShareXrs's release
//! workflow also tags pre-releases with a SemVer suffix (`v22.5.0-beta.1`,
//! docs/RELEASING.md), which `new Version` can't read, so [`AppVersion`]
//! adds (a ShareXrs addition) a pre-release part: it's split off before the
//! numbers are parsed as ShareX parses them, and only decides between two
//! versions whose numbers ShareX would call equal.

use std::cmp::Ordering;
use std::fmt;

/// `System.Version`: two to four components, the missing ones -1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DotNetVersion {
    pub major: i32,
    pub minor: i32,
    /// -1 when the string had two components.
    pub build: i32,
    /// -1 when the string had two or three components.
    pub revision: i32,
}

impl DotNetVersion {
    pub const fn new(major: i32, minor: i32, build: i32, revision: i32) -> Self {
        Self { major, minor, build, revision }
    }

    /// `new Version(string)` / `Version.Parse`: 2 to 4 parts separated by
    /// `.`, each `int.Parse(part, NumberStyles.Integer, InvariantCulture)`
    /// (white space around it and a sign allowed) and not negative. The
    /// error is the exception's message.
    pub fn parse(text: &str) -> Result<Self, String> {
        let parts: Vec<&str> = text.split('.').collect();
        if !(2..=4).contains(&parts.len()) {
            return Err("Version string portion was too short or too long. (Parameter 'input')".to_owned());
        }
        let mut values = [-1i32; 4];
        for (i, part) in parts.iter().enumerate() {
            values[i] = parse_component(part)?;
        }
        Ok(Self::new(values[0], values[1], values[2], values[3]))
    }

    /// `Version.Normalize(ignoreRevision)`: undefined parts become 0, and
    /// the revision too when it's ignored.
    pub fn normalize(self, ignore_revision: bool) -> Self {
        Self::new(
            self.major.max(0),
            self.minor.max(0),
            self.build.max(0),
            if ignore_revision { 0 } else { self.revision.max(0) },
        )
    }

    /// `Version.CompareTo`: part by part, an undefined part (-1) before 0.
    pub fn compare(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.build, self.revision).cmp(&(other.major, other.minor, other.build, other.revision))
    }

    /// `Helpers.CompareVersion(version1, version2, ignoreRevision)`.
    pub fn compare_normalized(&self, other: &Self, ignore_revision: bool) -> Ordering {
        self.normalize(ignore_revision).compare(&other.normalize(ignore_revision))
    }
}

/// `Version.ToString()`: the defined parts.
impl fmt::Display for DotNetVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)?;
        if self.build >= 0 {
            write!(f, ".{}", self.build)?;
            if self.revision >= 0 {
                write!(f, ".{}", self.revision)?;
            }
        }
        Ok(())
    }
}

/// `Version.TryParseComponent`: `int.TryParse` with `NumberStyles.Integer`
/// (leading and trailing white space, a leading `+` or `-`), then `>= 0`.
fn parse_component(part: &str) -> Result<i32, String> {
    let is_white = |c: char| matches!(c, '\u{9}'..='\u{d}' | ' ');
    let trimmed = part.trim_matches(is_white);
    let (negative, digits) = match trimmed.as_bytes().first() {
        Some(b'-') => (true, &trimmed[1..]),
        Some(b'+') => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Input string was not in a correct format.".to_owned());
    }
    // `int.TryParse` fails on overflow; `Version` then reports it as such.
    let overflow = || "Value was either too large or too small for an Int32.".to_owned();
    let significant = digits.trim_start_matches('0');
    if significant.len() > 10 {
        return Err(overflow());
    }
    let magnitude: i64 = if significant.is_empty() { 0 } else { significant.parse().map_err(|_| overflow())? };
    let value = if negative { -magnitude } else { magnitude };
    if value > i32::MAX as i64 || value < i32::MIN as i64 {
        return Err(overflow());
    }
    if value < 0 {
        return Err("Version's parameters must be greater than or equal to zero. (Parameter 'version')".to_owned());
    }
    Ok(value as i32)
}

/// A version as ShareXrs's update check sees it: ShareX's
/// [`DotNetVersion`] plus, as an addition, a SemVer pre-release part
/// (`beta.1` in `22.5.0-beta.1`). Build metadata (`+...`) is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppVersion {
    pub version: DotNetVersion,
    /// The SemVer pre-release identifiers, if any.
    pub pre_release: Option<String>,
}

impl AppVersion {
    pub fn new(version: DotNetVersion) -> Self {
        Self { version, pre_release: None }
    }

    /// The running program's version: `Version.Parse(GetApplicationVersion(true))`
    /// gives ShareX `major.minor.build.revision`; ShareXrs's comes from
    /// Cargo (`22.4.0` → `22.4.0.0`) with Cargo's pre-release part.
    pub fn current() -> Self {
        Self::from_cargo(
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
            env!("CARGO_PKG_VERSION_PRE"),
        )
    }

    fn from_cargo(major: &str, minor: &str, patch: &str, pre: &str) -> Self {
        let n = |s: &str| s.parse::<i32>().unwrap_or(0);
        Self {
            version: DotNetVersion::new(n(major), n(minor), n(patch), 0),
            pre_release: (!pre.is_empty()).then(|| pre.to_owned()),
        }
    }

    /// What `new Version(release.tag_name.Substring(1))` reads, after
    /// splitting off a SemVer `-pre.release` and `+build` suffix (an
    /// addition: ShareX's `new Version` throws on them).
    pub fn parse(text: &str) -> Result<Self, String> {
        // A `+` or `-` at the start of a part is a sign `new Version` reads
        // (`+22.5.0` is 22.5.0; `-1.0` is rejected as negative), not a
        // SemVer suffix.
        let suffix_at = |c: char, s: &str| {
            s.match_indices(c).map(|(i, _)| i).find(|&i| !s[..i].trim().is_empty() && !s[..i].ends_with('.'))
        };
        let without_build = suffix_at('+', text).map_or(text, |i| &text[..i]);
        let (core, pre) = match suffix_at('-', without_build) {
            Some(i) => (&without_build[..i], Some(&without_build[i + 1..])),
            None => (without_build, None),
        };
        let version = DotNetVersion::parse(core)?;
        match pre {
            Some(pre) if pre.is_empty() || pre.split('.').any(|id| id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')) => {
                Err(format!("'{pre}' isn't a SemVer pre-release."))
            }
            pre => Ok(Self { version, pre_release: pre.map(str::to_owned) }),
        }
    }

    /// `Helpers.CompareVersion(current, latest, ignoreRevision)`; when that
    /// says equal, SemVer's pre-release precedence decides (an addition):
    /// a release comes after its pre-releases, and pre-releases compare by
    /// their identifiers.
    pub fn compare(&self, other: &Self, ignore_revision: bool) -> Ordering {
        self.version.compare_normalized(&other.version, ignore_revision).then_with(|| {
            match (&self.pre_release, &other.pre_release) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => compare_pre_release(a, b),
            }
        })
    }
}

/// `Version.ToString()`, then `-<pre-release>`.
impl fmt::Display for AppVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.version)?;
        if let Some(pre) = &self.pre_release {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

/// SemVer 2.0.0 §11.4: identifier by identifier; numbers numerically and
/// before text; text in ASCII order; more identifiers come later.
fn compare_pre_release(a: &str, b: &str) -> Ordering {
    let mut a = a.split('.');
    let mut b = b.split('.');
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let numeric = |s: &str| s.bytes().all(|c| c.is_ascii_digit());
                let order = match (numeric(x), numeric(y)) {
                    (true, true) => {
                        let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                        x.len().cmp(&y.len()).then_with(|| x.cmp(y))
                    }
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    (false, false) => x.cmp(y),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> DotNetVersion {
        DotNetVersion::parse(s).unwrap()
    }

    #[test]
    fn parses_as_system_version() {
        assert_eq!(v("22.4.0"), DotNetVersion::new(22, 4, 0, -1));
        assert_eq!(v("1.2"), DotNetVersion::new(1, 2, -1, -1));
        assert_eq!(v("1.2.3.4"), DotNetVersion::new(1, 2, 3, 4));
        assert_eq!(v(" 1 .+2. 03"), DotNetVersion::new(1, 2, 3, -1));
        assert_eq!(v("-0.1"), DotNetVersion::new(0, 1, -1, -1));
        for bad in ["1", "", "1.2.3.4.5", "1..2", "a.1", "1.2-beta", "-1.0", "1.99999999999", "1.2.", "1. .2"] {
            assert!(DotNetVersion::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(v("1.2").to_string(), "1.2");
        assert_eq!(v("1.2.3.4").to_string(), "1.2.3.4");
    }

    #[test]
    fn compares_as_helpers_compare_version() {
        let c = |a: &str, b: &str, ignore| v(a).compare_normalized(&v(b), ignore);
        assert_eq!(c("22.4.0.0", "22.4.0", true), Ordering::Equal);
        assert_eq!(c("22.4.0.0", "22.4", true), Ordering::Equal);
        assert_eq!(c("22.4.0.0", "22.4.0.1", true), Ordering::Equal);
        assert_eq!(c("22.4.0.0", "22.4.0.1", false), Ordering::Less);
        assert_eq!(c("22.4.0.0", "22.10.0", true), Ordering::Less);
        assert_eq!(c("22.4.0.0", "3.0.0", true), Ordering::Greater);
        // Without normalising, an undefined part comes before 0.
        assert_eq!(v("1.2").compare(&v("1.2.0")), Ordering::Less);
    }

    #[test]
    fn reads_semver_pre_releases() {
        let a = |s: &str| AppVersion::parse(s).unwrap();
        assert_eq!(a("22.5.0-beta.1").pre_release.as_deref(), Some("beta.1"));
        assert_eq!(a("22.5.0+build.7"), a("22.5.0"));
        assert!(AppVersion::parse("22.5.0-").is_err());
        assert!(AppVersion::parse("22.5.0-be..ta").is_err());
        assert!(AppVersion::parse("-1.0").is_err());
        let lt = |x: &str, y: &str| a(x).compare(&a(y), true) == Ordering::Less;
        assert!(lt("22.5.0-beta.1", "22.5.0"));
        assert!(lt("22.5.0-alpha", "22.5.0-alpha.1"));
        assert!(lt("22.5.0-alpha.1", "22.5.0-alpha.beta"));
        assert!(lt("22.5.0-beta.2", "22.5.0-beta.11"));
        assert!(lt("22.5.0-rc.1", "22.5.1-alpha"));
        assert!(lt("22.4.0", "22.5.0-beta.1"));
        assert_eq!(a("22.5.0-beta.1").to_string(), "22.5.0-beta.1");
        assert_eq!(AppVersion::from_cargo("22", "4", "0", "").to_string(), "22.4.0.0");
        assert_eq!(AppVersion::from_cargo("22", "5", "0", "rc.1").to_string(), "22.5.0.0-rc.1");
    }
}
