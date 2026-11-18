//! `ShareX/NativeMessageInput.cs` as `TaskHelpers.HandleNativeMessagingInput`
//! reads it: `JsonHelpers.DeserializeFromFile<NativeMessagingInput>`, Json.NET
//! with a `StringEnumConverter` and every error handled. That reader is
//! lenient, and its error handling decides what survives a bad message, so
//! this follows what ShareX's own code returned for each case in
//! `tests/fixtures/cases.json` (tools/cli-harness):
//!
//! - property names in double or single quotes or none, matched ignoring
//!   case; the last of a repeated one wins; others are skipped;
//! - comments, a BOM, a trailing comma and anything after the object are
//!   ignored; a message cut off keeps what was read before the cut (unless
//!   it's cut between a name and its colon);
//! - `Action` takes a number (a 64-bit integer cut to 32 bits, any value) or
//!   a string: a number, or names ignoring case, joined with commas (OR'd,
//!   as `Enum.Parse` does). Anything else leaves it `None`;
//! - `URL` and `Text` take a string, or a number's text, `true`/`false`,
//!   `NaN`/`Infinity` as written. An array or object there, a missing comma,
//!   or a bad token (`undefined`, a bad escape) loses the whole message.

/// `ShareX.NativeMessagingAction`, or any other number Json.NET let in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeMessagingAction {
    #[default]
    None,
    UploadImage,
    UploadVideo,
    UploadAudio,
    UploadText,
    ShortenURL,
    /// A value with no name (the `default:` branch, as `None`).
    Other(i32),
}

const NAMES: [&str; 6] = ["None", "UploadImage", "UploadVideo", "UploadAudio", "UploadText", "ShortenURL"];

impl NativeMessagingAction {
    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::None,
            1 => Self::UploadImage,
            2 => Self::UploadVideo,
            3 => Self::UploadAudio,
            4 => Self::UploadText,
            5 => Self::ShortenURL,
            v => Self::Other(v),
        }
    }

    pub fn value(self) -> i32 {
        match self {
            Self::None => 0,
            Self::UploadImage => 1,
            Self::UploadVideo => 2,
            Self::UploadAudio => 3,
            Self::UploadText => 4,
            Self::ShortenURL => 5,
            Self::Other(v) => v,
        }
    }

    /// `Enum.ToString()`: the name, or the number.
    pub fn name(self) -> String {
        match self {
            Self::Other(v) => v.to_string(),
            a => NAMES[a.value() as usize].to_owned(),
        }
    }

    /// `EnumUtils.ParseEnum` (numbers allowed): a trimmed number, or names
    /// separated by commas.
    fn parse(text: &str) -> Option<Self> {
        let t = text.trim();
        if t.is_empty() {
            return None;
        }
        if t.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+') {
            return t.parse::<i32>().ok().map(Self::from_i32);
        }
        let mut value = 0;
        for part in t.split(',') {
            let part = part.trim();
            let i = NAMES.iter().position(|n| n.eq_ignore_ascii_case(part))?;
            value |= i as i32;
        }
        Some(Self::from_i32(value))
    }
}

/// `NativeMessagingInput`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeMessagingInput {
    pub action: NativeMessagingAction,
    pub url: Option<String>,
    pub text: Option<String>,
}

impl NativeMessagingInput {
    /// `JsonHelpers.DeserializeFromFile<NativeMessagingInput>` of a file
    /// holding `json` (an empty file is `None`, as is anything that isn't an
    /// object).
    pub fn parse(json: &str) -> Option<Self> {
        let json = json.strip_prefix('\u{feff}').unwrap_or(json);
        let mut r = Reader { s: json.as_bytes(), i: 0 };
        r.skip_trivia().ok()?;
        if r.peek() != Some(b'{') {
            return None;
        }
        r.i += 1;
        let mut input = Self::default();
        let mut first = true;
        loop {
            match r.skip_trivia() {
                Ok(()) => {}
                Err(Cut) => return Some(input),
            }
            match r.peek() {
                None => return Some(input),
                Some(b'}') => return Some(input),
                Some(b',') if !first => {
                    r.i += 1;
                    match r.skip_trivia() {
                        Ok(()) => {}
                        Err(Cut) => return Some(input),
                    }
                    match r.peek() {
                        None | Some(b'}') => return Some(input),
                        _ => {}
                    }
                }
                Some(_) if !first => return None,
                Some(_) => {}
            }
            first = false;
            let name = match r.name() {
                Ok(Some(n)) => n,
                Ok(None) => return None,
                // Cut inside the name, or before its colon.
                Err(Cut) => return None,
            };
            if r.skip_trivia().is_err() || r.peek() != Some(b':') {
                return None;
            }
            r.i += 1;
            if r.skip_trivia().is_err() {
                return Some(input);
            }
            let value = match r.value() {
                Ok(Some(v)) => v,
                Ok(None) => return None,
                Err(Cut) => return Some(input),
            };
            match name.to_ascii_lowercase().as_str() {
                "action" => {
                    input.action = match value {
                        Value::Number { int: Some(v), .. } => NativeMessagingAction::from_i32(v as i32),
                        Value::String(s) => NativeMessagingAction::parse(&s).unwrap_or_default(),
                        _ => NativeMessagingAction::None,
                    }
                }
                "url" | "text" => {
                    let v = match value {
                        Value::String(s) | Value::Raw(s) | Value::Number { text: s, .. } => Some(s),
                        Value::Null => None,
                        Value::Container => return None,
                    };
                    if name.eq_ignore_ascii_case("url") {
                        input.url = v;
                    } else {
                        input.text = v;
                    }
                }
                _ => {}
            }
        }
    }
}

/// The text ended where more was needed.
struct Cut;

enum Value {
    String(String),
    /// A number as written, and its value when it's an integer that fits
    /// 64 bits.
    Number { text: String, int: Option<i64> },
    /// `true`, `false`, `NaN`, `Infinity`, `-Infinity`, as written.
    Raw(String),
    Null,
    /// An array or object (skipped).
    Container,
}

struct Reader<'a> {
    s: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    /// White space and `//` or `/* */` comments.
    fn skip_trivia(&mut self) -> Result<(), Cut> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r' | b'\n') => self.i += 1,
                Some(b'/') => match self.s.get(self.i + 1) {
                    Some(b'/') => {
                        while !matches!(self.peek(), None | Some(b'\n' | b'\r')) {
                            self.i += 1;
                        }
                    }
                    Some(b'*') => {
                        self.i += 2;
                        loop {
                            match self.peek() {
                                None => return Err(Cut),
                                Some(b'*') if self.s.get(self.i + 1) == Some(&b'/') => {
                                    self.i += 2;
                                    break;
                                }
                                _ => self.i += 1,
                            }
                        }
                    }
                    _ => return Ok(()),
                },
                _ => return Ok(()),
            }
        }
    }

    /// A property name: quoted, or bare letters, digits, `_` and `$`.
    fn name(&mut self) -> Result<Option<String>, Cut> {
        match self.peek() {
            None => Err(Cut),
            Some(q @ (b'"' | b'\'')) => {
                self.i += 1;
                self.string(q)
            }
            Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80 => {
                let start = self.i;
                while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80) {
                    self.i += 1;
                }
                Ok(Some(String::from_utf8_lossy(&self.s[start..self.i]).into_owned()))
            }
            Some(_) => Ok(None),
        }
    }

    /// The rest of a string after its opening quote. `None`: a bad escape.
    fn string(&mut self, quote: u8) -> Result<Option<String>, Cut> {
        let mut out: Vec<u16> = Vec::new();
        let mut run = Vec::new();
        let flush = |run: &mut Vec<u8>, out: &mut Vec<u16>| {
            out.extend(String::from_utf8_lossy(run).encode_utf16());
            run.clear();
        };
        loop {
            let Some(c) = self.peek() else { return Err(Cut) };
            self.i += 1;
            if c == quote {
                flush(&mut run, &mut out);
                return Ok(Some(String::from_utf16_lossy(&out)));
            }
            if c != b'\\' {
                run.push(c);
                continue;
            }
            flush(&mut run, &mut out);
            let Some(e) = self.peek() else { return Err(Cut) };
            self.i += 1;
            let unit = match e {
                b'"' => b'"' as u16,
                b'\'' => b'\'' as u16,
                b'\\' => b'\\' as u16,
                b'/' => b'/' as u16,
                b'b' => 8,
                b'f' => 12,
                b'n' => b'\n' as u16,
                b'r' => b'\r' as u16,
                b't' => b'\t' as u16,
                b'u' => {
                    let Some(hex) = self.s.get(self.i..self.i + 4) else { return Err(Cut) };
                    let Some(v) = std::str::from_utf8(hex).ok().and_then(|h| u16::from_str_radix(h, 16).ok()) else {
                        return Ok(None);
                    };
                    self.i += 4;
                    v
                }
                _ => return Ok(None),
            };
            out.push(unit);
        }
    }

    /// A value. `None`: a token Json.NET rejects.
    fn value(&mut self) -> Result<Option<Value>, Cut> {
        let Some(c) = self.peek() else { return Err(Cut) };
        match c {
            b'"' | b'\'' => {
                self.i += 1;
                Ok(self.string(c)?.map(Value::String))
            }
            b'[' | b'{' => {
                self.skip_container()?;
                Ok(Some(Value::Container))
            }
            b'-' | b'0'..=b'9' | b'.' => {
                if self.s[self.i..].starts_with(b"-Infinity") {
                    self.i += 9;
                    return Ok(Some(Value::Raw("-Infinity".into())));
                }
                let start = self.i;
                while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'+' | b'.')) {
                    self.i += 1;
                }
                let text = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
                let digits = text.strip_prefix('-').unwrap_or(&text);
                let is_integer = !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit());
                if is_integer || text.parse::<f64>().is_ok() {
                    let int = if is_integer { text.parse::<i64>().ok() } else { None };
                    Ok(Some(Value::Number { text, int }))
                } else {
                    Ok(None)
                }
            }
            _ => {
                for word in ["true", "false", "NaN", "Infinity", "null"] {
                    if self.s[self.i..].starts_with(word.as_bytes()) {
                        self.i += word.len();
                        return Ok(Some(if word == "null" { Value::Null } else { Value::Raw(word.into()) }));
                    }
                }
                Ok(None)
            }
        }
    }

    /// An array or object, nested, strings and all.
    fn skip_container(&mut self) -> Result<(), Cut> {
        let mut depth = 0usize;
        loop {
            let Some(c) = self.peek() else { return Err(Cut) };
            self.i += 1;
            match c {
                b'[' | b'{' => depth += 1,
                b']' | b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                b'"' | b'\'' => {
                    let _ = self.string(c)?;
                }
                _ => {}
            }
        }
    }
}
