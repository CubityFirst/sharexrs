//! JSON as ShareX's uploaders see it through Newtonsoft.Json:
//!
//! - [`to_string`] — `JsonConvert.SerializeObject` (compact, Newtonsoft's
//!   string escaping),
//! - [`parse`] — `JToken.Parse`,
//! - [`to_dotnet_string`] — a token converted to `string`, as `(string)token`
//!   and typed deserialisation into a `string` property do (numbers and
//!   booleans as .NET prints them, ISO dates re-formatted as `DateTime`
//!   because Newtonsoft reads them as dates),
//! - [`get`] — a property by name, matched as Newtonsoft matches properties
//!   when deserialising (exact, else ignoring case),
//! - [`select_token`] — `JToken.SelectToken` (JSONPath).

use chrono::{Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde_json::{Map, Value};

/// `JsonConvert.SerializeObject(value)`: compact, with Newtonsoft's default
/// string escaping (control characters, `"`, `\`, and U+0085, U+2028,
/// U+2029, which serde_json leaves alone).
pub fn to_string(value: &Value) -> String {
    let text = serde_json::to_string(value).expect("JSON values serialise");
    // These only ever appear inside strings, so replacing them is safe.
    text.replace('\u{0085}', "\\u0085").replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029")
}

/// `JToken.Parse(text)`. The error for text that doesn't start like JSON
/// (an HTML error page, plain text) is Newtonsoft's message; other errors
/// are serde_json's.
pub fn parse(text: &str) -> Result<Value, String> {
    let text = text.trim_start_matches('\u{feff}');
    serde_json::from_str::<Value>(text).map_err(|e| newtonsoft_start_error(text).unwrap_or_else(|| format!("{e}")))
}

/// `JsonTextReader.ParseValue`'s "Unexpected character encountered while
/// parsing value" for a first character no JSON value starts with, with
/// its line and position (`JsonReaderException.Create`: line 0 while the
/// reader hasn't moved along the line).
fn newtonsoft_start_error(text: &str) -> Option<String> {
    let mut line = 1;
    let mut line_start = 0;
    for (i, c) in text.char_indices() {
        match c {
            '\n' => {
                line += 1;
                line_start = i + 1;
            }
            '\r' => {}
            ' ' | '\t' => {}
            '{' | '[' | '"' | '\'' | 't' | 'f' | 'n' | 'N' | 'I' | 'u' | '-' | '/' | '0'..='9' | '.' => return None,
            c => {
                let position = i - line_start;
                let line = if position == 0 { 0 } else { line };
                return Some(format!("Unexpected character encountered while parsing value: {c}. Path '', line {line}, position {position}."));
            }
        }
    }
    None
}

/// Parses a response for typed deserialisation (`JsonConvert.DeserializeObject<T>`):
/// `None` for an empty/whitespace response (Newtonsoft returns null), an
/// error for invalid JSON.
pub fn deserialize(text: &str) -> Result<Option<Value>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let value = parse(text)?;
    Ok(if value.is_null() { None } else { Some(value) })
}

/// A property of an object, matched as Newtonsoft matches a class's
/// properties: the exact name, else the first ignoring case.
pub fn get<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    let map = value.as_object()?;
    map.get(name).or_else(|| map.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v))
}

/// [`get`] then [`to_dotnet_string`]: a `string` property of a deserialised
/// class (`None` for null, missing, or a value that doesn't convert).
pub fn get_str(value: &Value, name: &str) -> Option<String> {
    get(value, name).and_then(|v| to_dotnet_string(v).ok().flatten())
}

/// A `bool` property (`false` when missing or null).
pub fn get_bool(value: &Value, name: &str) -> bool {
    match get(value, name) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s.eq_ignore_ascii_case("true"),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        _ => false,
    }
}

/// An `int` property (0 when missing or null).
pub fn get_i64(value: &Value, name: &str) -> i64 {
    match get(value, name) {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

/// `(string)token`: `Ok(None)` for null, the text of a value, an error
/// for an object or array ("Can not convert Object to String.").
pub fn to_dotnet_string(value: &Value) -> Result<Option<String>, String> {
    match value {
        Value::Null => Ok(None),
        Value::Bool(b) => Ok(Some(if *b { "True" } else { "False" }.to_owned())),
        Value::Number(n) => Ok(Some(if let Some(i) = n.as_i64() {
            i.to_string()
        } else if let Some(u) = n.as_u64() {
            u.to_string()
        } else {
            format_double(n.as_f64().unwrap_or(0.0))
        })),
        Value::String(s) => Ok(Some(parse_date(s).unwrap_or_else(|| s.clone()))),
        Value::Array(_) => Err("Can not convert Array to String.".to_owned()),
        Value::Object(_) => Err("Can not convert Object to String.".to_owned()),
    }
}

/// .NET's `double.ToString(CultureInfo.InvariantCulture)`: the shortest
/// round-tripping digits, in scientific notation (`1E+15`, `1E-05`) when
/// the exponent is 15 or more or below -4.
pub fn format_double(f: f64) -> String {
    if f.is_nan() {
        return "NaN".to_owned();
    }
    if f.is_infinite() {
        return if f > 0.0 { "∞" } else { "-∞" }.to_owned();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    let sci = format!("{:e}", f.abs());
    let (mantissa, exp) = sci.split_once('e').expect("{:e} has an exponent");
    let exp: i32 = exp.parse().expect("exponent");
    let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    let sign = if f < 0.0 { "-" } else { "" };
    if !(-4..15).contains(&exp) {
        let (first, rest) = digits.split_at(1);
        let rest = if rest.is_empty() { String::new() } else { format!(".{rest}") };
        let esign = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{first}{rest}E{esign}{:02}", exp.abs());
    }
    let n = digits.len() as i32;
    let body = if exp < 0 {
        format!("0.{}{}", "0".repeat((-exp - 1) as usize), digits)
    } else if exp + 1 >= n {
        format!("{digits}{}", "0".repeat((exp + 1 - n) as usize))
    } else {
        let (a, b) = digits.split_at((exp + 1) as usize);
        format!("{a}.{b}")
    };
    format!("{sign}{body}")
}

fn two(s: &[u8], i: usize) -> Option<u32> {
    let a = (*s.get(i)? as char).to_digit(10)?;
    let b = (*s.get(i + 1)? as char).to_digit(10)?;
    Some(a * 10 + b)
}

/// Newtonsoft's `DateTimeParser` (ISO 8601) and `/Date(ms)/`, as
/// `JsonTextReader` reads a string that looks like a date
/// (`DateParseHandling.DateTime`, `DateTimeZoneHandling.RoundtripKind`), then
/// `Convert.ToString(DateTime, InvariantCulture)`: `MM/dd/yyyy HH:mm:ss`.
/// A time with an offset becomes local time, as Newtonsoft converts it.
pub fn parse_date(text: &str) -> Option<String> {
    let s = text.as_bytes();
    if let Some(rest) = text.strip_prefix("/Date(").and_then(|r| r.strip_suffix(")/")) {
        let (ms, offset) = match rest.rfind(['+', '-']).filter(|&i| i > 0) {
            Some(i) => (&rest[..i], Some(&rest[i..])),
            None => (rest, None),
        };
        let ms: i64 = ms.parse().ok()?;
        let utc = Utc.timestamp_millis_opt(ms).single()?;
        let dt = if offset.is_some() { utc.with_timezone(&Local).naive_local() } else { utc.naive_utc() };
        return Some(dt.format("%m/%d/%Y %H:%M:%S").to_string());
    }
    if !(19..=40).contains(&s.len()) || !s[0].is_ascii_digit() {
        return None;
    }
    let year = two(s, 0)? * 100 + two(s, 2)?;
    if s[4] != b'-' || s[7] != b'-' || s[10] != b'T' || s[13] != b':' || s[16] != b':' {
        return None;
    }
    let (month, day, hour, minute, second) = (two(s, 5)?, two(s, 8)?, two(s, 11)?, two(s, 14)?, two(s, 17)?);
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(year as i32, month, day)?;
    let mut i = 19;
    if s.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    let naive = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(hour, minute, second)?);
    let mut offset: Option<i32> = None;
    if i < s.len() {
        match s[i] {
            b'Z' | b'z' => {
                i += 1;
                // Utc kind: printed as is.
                if i != s.len() {
                    return None;
                }
                return Some(naive.format("%m/%d/%Y %H:%M:%S").to_string());
            }
            b'+' | b'-' => {
                let sign = if s[i] == b'-' { -1 } else { 1 };
                let h = two(s, i + 1)? as i32;
                i += 3;
                let mut m = 0;
                if s.get(i) == Some(&b':') {
                    i += 1;
                    if let Some(v) = two(s, i) {
                        m = v as i32;
                        i += 2;
                    }
                } else if let Some(v) = two(s, i) {
                    m = v as i32;
                    i += 2;
                }
                offset = Some(sign * (h * 60 + m));
            }
            _ => return None,
        }
    }
    if i != s.len() {
        return None;
    }
    let shown = match offset {
        Some(minutes) => {
            let utc = naive - chrono::Duration::minutes(minutes as i64);
            Utc.from_utc_datetime(&utc).with_timezone(&Local).naive_local()
        }
        None => naive,
    };
    Some(shown.format("%m/%d/%Y %H:%M:%S").to_string())
}

// ---------------------------------------------------------------------------
// JSONPath, as Newtonsoft's `JPath` parses it and `SelectToken` runs it.

#[derive(Debug, Clone)]
enum PathFilter {
    /// `.name` / `['name']`; `None` is `.*` / `[*]`.
    Field(Option<String>),
    Fields(Vec<String>),
    /// `..name`; `None` is `..*`.
    Scan(Option<String>),
    Index(i64),
    Indexes(Vec<i64>),
    Slice(Option<i64>, Option<i64>, Option<i64>),
    Query(Expr),
}

#[derive(Debug, Clone)]
enum Operand {
    Path { root: bool, filters: Vec<PathFilter> },
    Value(Value),
    Regex(String),
}

#[derive(Debug, Clone)]
enum Expr {
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Exists(Operand),
    Cmp(Operand, String, Operand),
}

struct PathParser<'a> {
    s: Vec<char>,
    i: usize,
    _src: &'a str,
}

impl<'a> PathParser<'a> {
    fn new(src: &'a str) -> Self {
        Self { s: src.chars().collect(), i: 0, _src: src }
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek() == Some(' ') {
            self.i += 1;
        }
    }

    fn err<T>(&self, msg: &str) -> Result<T, String> {
        Err(msg.to_owned())
    }

    /// `ParsePath`: filters until the end (or, inside a query, until a
    /// character that ends an operand).
    fn parse_path(&mut self, in_query: bool) -> Result<Vec<PathFilter>, String> {
        let mut filters = Vec::new();
        let mut field = String::new();
        let mut scan = false;
        let mut following_dot = false;
        let mut started = false;
        let flush = |field: &mut String, scan: &mut bool, filters: &mut Vec<PathFilter>| {
            if !field.is_empty() || *scan {
                let name = if field == "*" { None } else { Some(std::mem::take(field)) };
                field.clear();
                filters.push(if *scan { PathFilter::Scan(name) } else { PathFilter::Field(name) });
                *scan = false;
            }
        };
        while let Some(c) = self.peek() {
            match c {
                '[' | '(' => {
                    flush(&mut field, &mut scan, &mut filters);
                    self.i += 1;
                    filters.push(self.parse_indexer(c)?);
                    following_dot = false;
                    started = true;
                }
                ']' | ')' if !in_query => return self.err(&format!("Unexpected character while parsing path: {c}")),
                ' ' | ')' | ']' | '=' | '!' | '<' | '>' | '&' | '|' if in_query => break,
                ' ' => {
                    self.skip_ws();
                    if self.peek().is_some() {
                        return self.err("Unexpected character while parsing path: ");
                    }
                }
                '.' => {
                    if !field.is_empty() || scan {
                        flush(&mut field, &mut scan, &mut filters);
                    } else if following_dot && self.s.get(self.i - 1) == Some(&'.') {
                        scan = true;
                    }
                    if self.s.get(self.i + 1) == Some(&'.') {
                        scan = true;
                        self.i += 1;
                    }
                    self.i += 1;
                    following_dot = true;
                    started = true;
                }
                _ => {
                    if in_query && !started && !matches!(c, '$' | '@') {
                        break;
                    }
                    field.push(c);
                    self.i += 1;
                    following_dot = false;
                    started = true;
                }
            }
        }
        flush(&mut field, &mut scan, &mut filters);
        Ok(filters)
    }

    fn parse_indexer(&mut self, open: char) -> Result<PathFilter, String> {
        let close = if open == '[' { ']' } else { ')' };
        self.skip_ws();
        match self.peek() {
            Some('\'') | Some('"') => {
                let mut names = Vec::new();
                loop {
                    self.skip_ws();
                    let q = self.peek().ok_or("Path ended with open indexer.")?;
                    self.i += 1;
                    let mut name = String::new();
                    loop {
                        let c = self.peek().ok_or("Path ended with an open string.")?;
                        self.i += 1;
                        if c == '\\' {
                            let e = self.peek().ok_or("Path ended with an open string.")?;
                            self.i += 1;
                            name.push(match e {
                                'b' => '\u{8}',
                                't' => '\t',
                                'n' => '\n',
                                'f' => '\u{c}',
                                'r' => '\r',
                                other => other,
                            });
                        } else if c == q {
                            break;
                        } else {
                            name.push(c);
                        }
                    }
                    names.push(name);
                    self.skip_ws();
                    match self.peek() {
                        Some(',') => self.i += 1,
                        Some(c) if c == close => {
                            self.i += 1;
                            break;
                        }
                        _ => return self.err("Unexpected character while parsing path indexer."),
                    }
                }
                Ok(if names.len() == 1 { PathFilter::Field(names.pop()) } else { PathFilter::Fields(names) })
            }
            Some('?') => {
                self.i += 1;
                self.skip_ws();
                if self.peek() != Some('(') {
                    return self.err("Unexpected character while parsing path indexer.");
                }
                self.i += 1;
                let expr = self.parse_expression()?;
                self.skip_ws();
                if self.peek() != Some(')') {
                    return self.err("Path ended with open query.");
                }
                self.i += 1;
                self.skip_ws();
                if self.peek() != Some(close) {
                    return self.err("Unexpected end while parsing path.");
                }
                self.i += 1;
                Ok(PathFilter::Query(expr))
            }
            Some('*') => {
                self.i += 1;
                self.skip_ws();
                if self.peek() != Some(close) {
                    return self.err("Unexpected character while parsing path indexer.");
                }
                self.i += 1;
                Ok(PathFilter::Field(None))
            }
            _ => {
                let mut text = String::new();
                loop {
                    let c = self.peek().ok_or("Path ended with open indexer.")?;
                    self.i += 1;
                    if c == close {
                        break;
                    }
                    text.push(c);
                }
                let text = text.trim();
                let num = |t: &str| -> Result<Option<i64>, String> {
                    let t = t.trim();
                    if t.is_empty() {
                        Ok(None)
                    } else {
                        t.parse::<i64>().map(Some).map_err(|_| format!("Unexpected character while parsing path indexer: {t}"))
                    }
                };
                if text.contains(':') {
                    let parts: Vec<&str> = text.split(':').collect();
                    if parts.len() > 3 {
                        return self.err("Too many colons in slice.");
                    }
                    let step = if parts.len() == 3 { num(parts[2])? } else { None };
                    if step == Some(0) {
                        return self.err("Step cannot be zero.");
                    }
                    Ok(PathFilter::Slice(num(parts[0])?, num(parts[1])?, step))
                } else if text.contains(',') {
                    let indexes = text.split(',').map(|t| num(t).and_then(|n| n.ok_or("Array index expected.".into()))).collect::<Result<Vec<_>, _>>()?;
                    Ok(PathFilter::Indexes(indexes))
                } else {
                    let n = num(text)?.ok_or("Array index expected.")?;
                    Ok(PathFilter::Index(n))
                }
            }
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, String> {
        let mut ors = vec![self.parse_and()?];
        loop {
            self.skip_ws();
            if self.s.get(self.i) == Some(&'|') && self.s.get(self.i + 1) == Some(&'|') {
                self.i += 2;
                ors.push(self.parse_and()?);
            } else {
                break;
            }
        }
        Ok(if ors.len() == 1 { ors.pop().expect("one") } else { Expr::Or(ors) })
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut ands = vec![self.parse_comparison()?];
        loop {
            self.skip_ws();
            if self.s.get(self.i) == Some(&'&') && self.s.get(self.i + 1) == Some(&'&') {
                self.i += 2;
                ands.push(self.parse_comparison()?);
            } else {
                break;
            }
        }
        Ok(if ands.len() == 1 { ands.pop().expect("one") } else { Expr::And(ands) })
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        self.skip_ws();
        if self.peek() == Some('(') {
            self.i += 1;
            let e = self.parse_expression()?;
            self.skip_ws();
            if self.peek() != Some(')') {
                return self.err("Unexpected character while parsing path query.");
            }
            self.i += 1;
            return Ok(e);
        }
        let left = self.parse_operand()?;
        self.skip_ws();
        let ops = ["==", "!=", "<=", ">=", "=~", "<", ">"];
        for op in ops {
            let len = op.chars().count();
            if self.s[self.i..].iter().take(len).collect::<String>() == op {
                self.i += len;
                self.skip_ws();
                let right = self.parse_operand()?;
                return Ok(Expr::Cmp(left, op.to_owned(), right));
            }
        }
        Ok(Expr::Exists(left))
    }

    fn parse_operand(&mut self) -> Result<Operand, String> {
        self.skip_ws();
        match self.peek() {
            Some('@') | Some('$') => {
                let root = self.peek() == Some('$');
                self.i += 1;
                let filters = self.parse_path(true)?;
                Ok(Operand::Path { root, filters })
            }
            Some('\'') | Some('"') => {
                let q = self.peek().expect("quote");
                self.i += 1;
                let mut s = String::new();
                loop {
                    let c = self.peek().ok_or("Path ended with an open string.")?;
                    self.i += 1;
                    if c == '\\' {
                        let e = self.peek().ok_or("Path ended with an open string.")?;
                        self.i += 1;
                        s.push(e);
                    } else if c == q {
                        break;
                    } else {
                        s.push(c);
                    }
                }
                Ok(Operand::Value(Value::String(s)))
            }
            Some('/') => {
                self.i += 1;
                let mut s = String::new();
                loop {
                    let c = self.peek().ok_or("Path ended with an open regex.")?;
                    self.i += 1;
                    if c == '\\' {
                        s.push(c);
                        s.push(self.peek().ok_or("Path ended with an open regex.")?);
                        self.i += 1;
                    } else if c == '/' {
                        break;
                    } else {
                        s.push(c);
                    }
                }
                let mut flags = String::new();
                while let Some(c) = self.peek().filter(|c| c.is_ascii_alphabetic()) {
                    flags.push(c);
                    self.i += 1;
                }
                let prefix = if flags.is_empty() { String::new() } else { format!("(?{flags})") };
                Ok(Operand::Regex(format!("{prefix}{s}")))
            }
            _ => {
                let mut t = String::new();
                while let Some(c) = self.peek() {
                    if matches!(c, ' ' | ')' | ']' | '=' | '!' | '<' | '>' | '&' | '|') {
                        break;
                    }
                    t.push(c);
                    self.i += 1;
                }
                match t.as_str() {
                    "true" => Ok(Operand::Value(Value::Bool(true))),
                    "false" => Ok(Operand::Value(Value::Bool(false))),
                    "null" => Ok(Operand::Value(Value::Null)),
                    _ => serde_json::from_str::<serde_json::Number>(&t)
                        .map(|n| Operand::Value(Value::Number(n)))
                        .map_err(|_| format!("Could not read query value: {t}")),
                }
            }
        }
    }
}

fn eval_filters<'v>(root: &'v Value, current: Vec<&'v Value>, filters: &[PathFilter]) -> Vec<&'v Value> {
    let mut set = current;
    for f in filters {
        let mut next: Vec<&'v Value> = Vec::new();
        for v in set {
            match f {
                PathFilter::Field(Some(name)) => {
                    if let Some(x) = v.as_object().and_then(|m| m.get(name)) {
                        next.push(x);
                    }
                }
                PathFilter::Field(None) => match v {
                    Value::Object(m) => next.extend(m.values()),
                    Value::Array(a) => next.extend(a.iter()),
                    _ => {}
                },
                PathFilter::Fields(names) => {
                    if let Some(m) = v.as_object() {
                        next.extend(names.iter().filter_map(|n| m.get(n)));
                    }
                }
                PathFilter::Scan(name) => scan(v, name.as_deref(), &mut next),
                PathFilter::Index(i) => {
                    if let Some(x) = index(v, *i) {
                        next.push(x);
                    }
                }
                PathFilter::Indexes(is) => next.extend(is.iter().filter_map(|i| index(v, *i))),
                PathFilter::Slice(start, end, step) => {
                    if let Some(a) = v.as_array() {
                        let len = a.len() as i64;
                        let step = step.unwrap_or(1);
                        let norm = |x: i64| if x < 0 { (len + x).max(0) } else { x.min(len) };
                        if step > 0 {
                            let (s, e) = (start.map(norm).unwrap_or(0), end.map(norm).unwrap_or(len));
                            let mut k = s;
                            while k < e {
                                next.push(&a[k as usize]);
                                k += step;
                            }
                        } else {
                            let s = start.map(|x| if x < 0 { len + x } else { x.min(len - 1) }).unwrap_or(len - 1);
                            let e = end.map(|x| if x < 0 { len + x } else { x }).unwrap_or(-1);
                            let mut k = s;
                            while k > e && k >= 0 {
                                next.push(&a[k as usize]);
                                k += step;
                            }
                        }
                    }
                }
                PathFilter::Query(expr) => {
                    let items: Vec<&Value> = match v {
                        Value::Array(a) => a.iter().collect(),
                        Value::Object(m) => m.values().collect(),
                        _ => Vec::new(),
                    };
                    next.extend(items.into_iter().filter(|item| eval_expr(root, item, expr)));
                }
            }
        }
        set = next;
    }
    set
}

fn index(v: &Value, i: i64) -> Option<&Value> {
    let a = v.as_array()?;
    usize::try_from(i).ok().and_then(|i| a.get(i))
}

fn scan<'v>(v: &'v Value, name: Option<&str>, out: &mut Vec<&'v Value>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if name.is_none_or(|n| n == k) {
                    out.push(x);
                }
                scan(x, name, out);
            }
        }
        Value::Array(a) => {
            for x in a {
                if name.is_none() {
                    out.push(x);
                }
                scan(x, name, out);
            }
        }
        _ => {}
    }
}

fn operand_values<'v>(root: &'v Value, current: &'v Value, op: &'v Operand) -> Vec<&'v Value> {
    match op {
        Operand::Path { root: is_root, filters } => eval_filters(root, vec![if *is_root { root } else { current }], filters),
        Operand::Value(v) => vec![v],
        Operand::Regex(_) => Vec::new(),
    }
}

fn eval_expr(root: &Value, current: &Value, expr: &Expr) -> bool {
    match expr {
        Expr::And(es) => es.iter().all(|e| eval_expr(root, current, e)),
        Expr::Or(es) => es.iter().any(|e| eval_expr(root, current, e)),
        Expr::Exists(op) => !operand_values(root, current, op).is_empty(),
        Expr::Cmp(l, op, r) => {
            let lv = operand_values(root, current, l);
            if let Operand::Regex(pattern) = r {
                let Ok(re) = fancy_regex::Regex::new(pattern) else { return false };
                return lv.iter().any(|v| v.as_str().is_some_and(|s| re.is_match(s).unwrap_or(false)));
            }
            let rv = operand_values(root, current, r);
            lv.iter().any(|a| rv.iter().any(|b| compare(a, op, b)))
        }
    }
}

fn compare(a: &Value, op: &str, b: &Value) -> bool {
    use std::cmp::Ordering;
    let ord: Option<Ordering> = match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64().partial_cmp(&y.as_f64()),
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        (Value::Null, Value::Null) => Some(Ordering::Equal),
        // Newtonsoft compares a string with another value by its text.
        (Value::String(x), y) | (y, Value::String(x)) if !y.is_null() && !y.is_object() && !y.is_array() => {
            let ys = to_dotnet_string(y).ok().flatten().unwrap_or_default();
            if op == "==" || op == "!=" { Some(if *x == ys { Ordering::Equal } else { Ordering::Less }) } else { None }
        }
        _ => None,
    };
    match op {
        "==" => ord == Some(Ordering::Equal),
        "!=" => ord != Some(Ordering::Equal),
        "<" => ord == Some(Ordering::Less),
        "<=" => matches!(ord, Some(Ordering::Less | Ordering::Equal)),
        ">" => ord == Some(Ordering::Greater),
        ">=" => matches!(ord, Some(Ordering::Greater | Ordering::Equal)),
        _ => false,
    }
}

/// `JToken.SelectToken(path)`: the one token the path selects, `None` when
/// nothing matches, an error for a bad path or several matches.
pub fn select_token<'v>(root: &'v Value, path: &str) -> Result<Option<&'v Value>, String> {
    let mut p = PathParser::new(path);
    let mut start = 0;
    if p.peek() == Some('$') {
        p.i += 1;
        start = 1;
    }
    let _ = start;
    let filters = p.parse_path(false)?;
    let found = eval_filters(root, vec![root], &filters);
    match found.len() {
        0 => Ok(None),
        1 => Ok(Some(found[0])),
        _ => Err("Path returned multiple tokens.".to_owned()),
    }
}

/// Builds a JSON object in property order (anonymous objects and the
/// request classes ShareX serialises).
pub fn object<const N: usize>(props: [(&str, Value); N]) -> Value {
    let mut m = Map::new();
    for (k, v) in props {
        m.insert(k.to_owned(), v);
    }
    Value::Object(m)
}

/// A `string` property: `null` when `None`.
pub fn str_or_null(s: Option<&str>) -> Value {
    s.map(|s| Value::String(s.to_owned())).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn select_tokens_like_newtonsoft() {
        let v = json!({"files":[{"url":"a"},{"url":"b"}],"data":{"link":"L","n":3,"f":1.5,"t":true,"o":{}}});
        let s = |p: &str| select_token(&v, p).map(|t| t.and_then(|t| to_dotnet_string(t).ok().flatten()));
        assert_eq!(s("$.files[0].url").unwrap().as_deref(), Some("a"));
        assert_eq!(s("files[1].url").unwrap().as_deref(), Some("b"));
        assert_eq!(s("$.data.link").unwrap().as_deref(), Some("L"));
        assert_eq!(s("$['data']['n']").unwrap().as_deref(), Some("3"));
        assert_eq!(s("$.data.f").unwrap().as_deref(), Some("1.5"));
        assert_eq!(s("$.data.t").unwrap().as_deref(), Some("True"));
        assert_eq!(s("$.missing.x").unwrap(), None);
        assert_eq!(s("$.files[5].url").unwrap(), None);
        assert!(s("$.files[*].url").is_err());
        assert_eq!(s("$.files[?(@.url == 'b')].url").unwrap().as_deref(), Some("b"));
        assert_eq!(s("$..link").unwrap().as_deref(), Some("L"));
        assert!(select_token(&v, "$.data.o").map(|t| to_dotnet_string(t.unwrap())).unwrap().is_err());
    }

    #[test]
    fn dates_and_numbers_print_as_dotnet() {
        assert_eq!(parse_date("2024-01-02T03:04:05Z").as_deref(), Some("01/02/2024 03:04:05"));
        assert_eq!(parse_date("2024-01-02T03:04:05.123").as_deref(), Some("01/02/2024 03:04:05"));
        assert_eq!(parse_date("2024-01-02"), None);
        assert_eq!(parse_date("2024-02-30T03:04:05"), None);
        assert_eq!(format_double(1.5), "1.5");
        assert_eq!(format_double(1e15), "1E+15");
        assert_eq!(format_double(1e14), "100000000000000");
        assert_eq!(format_double(0.0001), "0.0001");
        assert_eq!(format_double(0.00001), "1E-05");
        assert_eq!(format_double(-2.0), "-2");
    }

    #[test]
    fn serialises_like_newtonsoft() {
        let v = object([("a", Value::Null), ("b", json!("x\u{2028}\u{1f}")), ("c", json!(false))]);
        assert_eq!(to_string(&v), r#"{"a":null,"b":"x\u2028\u001f","c":false}"#);
    }
}
