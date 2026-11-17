//! `ShareX.OCR/ModelConfig.cs`: the few values the pipeline reads from a PaddleOCR
//! `inference.yml`. The files are plain YAML written by PyYAML, so `key: value`
//! lines and `- item` lists are read directly, as ShareX does (no YAML parser).

use std::path::Path;

use crate::OcrError;

pub(crate) struct ModelConfig {
    lines: Vec<String>,
}

impl ModelConfig {
    /// `ModelConfig.Load`: `File.ReadAllLines` (UTF-8, BOM skipped; lines end at
    /// `\r\n`, `\n` or `\r`).
    pub fn load(path: &Path) -> Result<Self, OcrError> {
        let bytes = std::fs::read(path).map_err(|_| OcrError::MissingFile {
            what: "The OCR model configuration file is missing.",
            path: path.to_path_buf(),
        })?;
        Ok(Self::parse(&String::from_utf8_lossy(&bytes)))
    }

    pub fn parse(text: &str) -> Self {
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
        let mut lines = Vec::new();
        let mut rest = text;
        while !rest.is_empty() {
            match rest.find(['\r', '\n']) {
                Some(i) => {
                    lines.push(rest[..i].to_owned());
                    let skip = if rest[i..].starts_with("\r\n") { 2 } else { 1 };
                    rest = &rest[i + skip..];
                }
                None => {
                    lines.push(rest.to_owned());
                    rest = "";
                }
            }
        }
        Self { lines }
    }

    /// `GetFloat`: `float.TryParse(…, NumberStyles.Float, InvariantCulture)`.
    pub fn get_float(&self, key: &str, default: f32) -> f32 {
        self.get_scalar(key)
            .and_then(|v| parse_float(&v))
            .unwrap_or(default)
    }

    /// `GetInt`: `int.TryParse(…, NumberStyles.Integer, InvariantCulture)`.
    pub fn get_int(&self, key: &str, default: i32) -> i32 {
        self.get_scalar(key)
            .and_then(|v| parse_int(&v))
            .unwrap_or(default)
    }

    /// `GetFloats`: the list, or the default if an item isn't a number or the
    /// count differs.
    pub fn get_floats<const N: usize>(&self, key: &str, default: [f32; N]) -> [f32; N] {
        let items = self.get_list(key);
        let mut values = Vec::with_capacity(items.len());
        for item in &items {
            match parse_float(item) {
                Some(v) => values.push(v),
                None => return default,
            }
        }
        values.try_into().unwrap_or(default)
    }

    /// `GetScalar`: the first `key: value` line with a value, at any depth.
    pub fn get_scalar(&self, key: &str) -> Option<String> {
        let prefix = format!("{key}:");
        for line in &self.lines {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix(&prefix) {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(unquote(value));
                }
            }
        }
        None
    }

    /// `GetList`: the `- item` lines after the first `key:` line (PyYAML writes the
    /// items at the key's indentation).
    pub fn get_list(&self, key: &str) -> Vec<String> {
        let header = format!("{key}:");
        let mut items = Vec::new();
        for (i, line) in self.lines.iter().enumerate() {
            if line.trim() != header {
                continue;
            }
            let indent = indent_of(line);
            for item_line in &self.lines[i + 1..] {
                let item_line = item_line.trim_end_matches(['\r', '\n']);
                let item_indent = indent_of(item_line);
                let b = item_line.as_bytes();
                // `itemLine.Length < itemIndent + 2` counts UTF-16 units; the indent and
                // "- " are ASCII, so bytes give the same answer.
                if item_indent < indent
                    || b.len() < item_indent + 2
                    || b[item_indent] != b'-'
                    || b[item_indent + 1] != b' '
                {
                    break;
                }
                items.push(unquote(&item_line[item_indent + 2..]));
            }
            break;
        }
        items
    }
}

fn indent_of(line: &str) -> usize {
    line.bytes().take_while(|&b| b == b' ').count()
}

/// `Unquote`: single quotes with `''` for a quote, double quotes through
/// `Regex.Unescape`, plain scalars without trailing spaces.
fn unquote(value: &str) -> String {
    let n = value.chars().count();
    if n >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return value[1..value.len() - 1].replace("''", "'");
    }
    if n >= 2 && value.starts_with('"') && value.ends_with('"') {
        return regex_unescape(&value[1..value.len() - 1]);
    }
    value.trim_end().to_owned()
}

/// `Regex.Unescape` for the escapes PyYAML writes.
fn regex_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(e) = chars.next() else {
            out.push('\\');
            break;
        };
        let hex = |chars: &mut std::iter::Peekable<std::str::Chars>, n: usize| {
            let digits: String = (0..n).filter_map(|_| chars.next()).collect();
            u32::from_str_radix(&digits, 16)
                .ok()
                .and_then(char::from_u32)
        };
        match e {
            'a' => out.push('\u{7}'),
            'b' => out.push('\u{8}'),
            't' => out.push('\t'),
            'n' => out.push('\n'),
            'v' => out.push('\u{B}'),
            'f' => out.push('\u{C}'),
            'r' => out.push('\r'),
            'e' => out.push('\u{1B}'),
            '0' => out.push('\0'),
            'x' => out.extend(hex(&mut chars, 2)),
            'u' => out.extend(hex(&mut chars, 4)),
            other => out.push(other),
        }
    }
    out
}

/// .NET's invariant `float.TryParse` for the numbers in these files (`0.45`,
/// `1e-5`, `-3`); whitespace around the number is allowed.
pub(crate) fn parse_float(s: &str) -> Option<f32> {
    let t = s.trim();
    if t.is_empty()
        || t.contains(['_', 'x', 'X'])
        || t.eq_ignore_ascii_case("inf")
        || t.eq_ignore_ascii_case("nan")
    {
        return None;
    }
    t.parse::<f32>().ok()
}

/// .NET's invariant `int.TryParse`.
pub(crate) fn parse_int(s: &str) -> Option<i32> {
    let t = s.trim();
    let t = t.strip_prefix('+').unwrap_or(t);
    t.parse::<i32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const YML: &str = "PostProcess:\r\n  box_thresh: 0.45\r\n  max_candidates: 3000\r\n  character_dict:\r\n  - '!'\r\n  - ''''\r\n  - a\r\n  - '  '\r\nPreProcess:\r\n  - NormalizeImage:\r\n      mean:\r\n      - 0.485\r\n      - 0.456\r\n      - 0.406\r\n";

    #[test]
    fn reads_scalars_and_lists_as_sharex_does() {
        let c = ModelConfig::parse(YML);
        assert_eq!(c.get_float("box_thresh", 0.6), 0.45);
        assert_eq!(c.get_float("thresh", 0.3), 0.3);
        assert_eq!(c.get_int("max_candidates", 1000), 3000);
        assert_eq!(c.get_list("character_dict"), vec!["!", "'", "a", "  "]);
        assert_eq!(c.get_floats("mean", [0.0; 3]), [0.485, 0.456, 0.406]);
        assert_eq!(
            c.get_floats("std", [0.229, 0.224, 0.225]),
            [0.229, 0.224, 0.225]
        );
    }
}
