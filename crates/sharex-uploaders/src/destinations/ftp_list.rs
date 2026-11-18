//! FluentFTP's directory listings as ShareX's remote storage browser gets
//! them (`FtpClient.GetListing(path)`): the listing parsers
//! (`FluentFTP.Helpers.Parsers`: Unix, Windows/IIS, MLSD, VMS, IBM z/OS,
//! IBM OS/400, Tandem NonStop) with FluentFTP's auto-detection, its path
//! sanitizer (`SanitizerModule`, whose default mode throws), and the FTP
//! server handlers that change what a listing or a delete sends.
//!
//! Dates are parsed as .NET's `DateTime.ParseExact` with the invariant
//! culture parses FluentFTP's formats ([`parse_exact`]).

use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime};

// ---------------------------------------------------------------------------
// DateTime.ParseExact (invariant culture, DateTimeStyles.None) for the
// formats FluentFTP uses.

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const MONTHS_FULL: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

enum Token {
    Field(char, usize),
    Literal(char),
}

fn tokens(format: &str) -> Vec<Token> {
    let chars: Vec<char> = format.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != c {
                out.push(Token::Literal(chars[i]));
                i += 1;
            }
            i += 1;
        } else if c == '\\' && i + 1 < chars.len() {
            out.push(Token::Literal(chars[i + 1]));
            i += 2;
        } else if "yMdHhmsft".contains(c) {
            let mut n = 1;
            while i + n < chars.len() && chars[i + n] == c {
                n += 1;
            }
            out.push(Token::Field(c, n));
            i += n;
        } else {
            out.push(Token::Literal(c));
            i += 1;
        }
    }
    out
}

/// `ParseDigits(ref str, digitLen)`: 1 means one or two digits, more means
/// exactly that many.
fn digits(s: &[char], pos: &mut usize, len: usize) -> Option<u32> {
    let (min, max) = if len == 1 { (1, 2) } else { (len, len) };
    let mut n = 0;
    let mut v: u32 = 0;
    while n < max && *pos + n < s.len() && s[*pos + n].is_ascii_digit() {
        v = v.checked_mul(10)?.checked_add(s[*pos + n].to_digit(10)?)?;
        n += 1;
    }
    if n < min {
        return None;
    }
    *pos += n;
    Some(v)
}

fn match_name(s: &[char], pos: &mut usize, names: &[&str]) -> Option<u32> {
    // The longest name that matches, ignoring case.
    let mut best: Option<(usize, u32)> = None;
    for (i, name) in names.iter().enumerate() {
        let n: Vec<char> = name.chars().collect();
        if *pos + n.len() <= s.len() && s[*pos..*pos + n.len()].iter().zip(&n).all(|(a, b)| a.to_lowercase().eq(b.to_lowercase())) && best.is_none_or(|(l, _)| n.len() > l) {
            best = Some((n.len(), i as u32 + 1));
        }
    }
    let (l, m) = best?;
    *pos += l;
    Some(m)
}

/// The year of a two-digit year (`Calendar.ToFourDigitYear`, 2049 as
/// `TwoDigitYearMax`).
fn four_digit_year(y: u32) -> i32 {
    if y > 49 { 1900 + y as i32 } else { 2000 + y as i32 }
}

fn parse_one(s: &str, format: &str) -> Option<NaiveDateTime> {
    let s: Vec<char> = s.chars().collect();
    let mut pos = 0;
    let (mut year, mut month, mut day) = (None, None, None);
    let (mut hour, mut minute, mut second, mut nanos) = (0u32, 0u32, 0u32, 0u32);
    let mut twelve = false;
    let mut pm: Option<bool> = None;
    for t in tokens(format) {
        match t {
            Token::Literal(c) => {
                if s.get(pos) != Some(&c) {
                    return None;
                }
                pos += 1;
            }
            Token::Field(c, n) => match c {
                'y' => {
                    let v = digits(&s, &mut pos, n.min(5))?;
                    year = Some(if n <= 2 { four_digit_year(v) } else { v as i32 });
                }
                'M' if n >= 4 => month = Some(match_name(&s, &mut pos, &MONTHS_FULL)?),
                'M' if n == 3 => month = Some(match_name(&s, &mut pos, &MONTHS)?),
                'M' => month = Some(digits(&s, &mut pos, n)?),
                'd' => day = Some(digits(&s, &mut pos, n.min(2))?),
                'H' => hour = digits(&s, &mut pos, n.min(2))?,
                'h' => {
                    hour = digits(&s, &mut pos, n.min(2))?;
                    twelve = true;
                }
                'm' => minute = digits(&s, &mut pos, n.min(2))?,
                's' => second = digits(&s, &mut pos, n.min(2))?,
                'f' => {
                    // Exactly `n` digits.
                    if n > 7 || pos + n > s.len() || !s[pos..pos + n].iter().all(char::is_ascii_digit) {
                        return None;
                    }
                    let v: u32 = s[pos..pos + n].iter().collect::<String>().parse().ok()?;
                    pos += n;
                    nanos = v * 10u32.pow(9 - n as u32);
                }
                't' => {
                    let rest: String = s[pos..].iter().take(2).collect::<String>().to_ascii_uppercase();
                    if n == 1 {
                        match rest.chars().next()? {
                            'A' => pm = Some(false),
                            'P' => pm = Some(true),
                            _ => return None,
                        }
                        pos += 1;
                    } else {
                        match rest.as_str() {
                            "AM" => pm = Some(false),
                            "PM" => pm = Some(true),
                            _ => return None,
                        }
                        pos += 2;
                    }
                }
                _ => return None,
            },
        }
    }
    if pos != s.len() {
        return None;
    }
    if twelve {
        if hour > 12 {
            return None;
        }
        if pm == Some(true) && hour < 12 {
            hour += 12;
        } else if pm == Some(false) && hour == 12 {
            hour = 0;
        }
    } else if let Some(p) = pm {
        // `tt` with `HH`: .NET checks the designator agrees.
        if (p && hour < 12) || (!p && hour >= 12) {
            return None;
        }
    }
    let today = Local::now().date_naive();
    let date = match (year, month, day) {
        (None, None, None) => today,
        (y, m, d) => NaiveDate::from_ymd_opt(y.unwrap_or(today.year()), m.unwrap_or(1), d.unwrap_or(1))?,
    };
    Some(NaiveDateTime::new(date, NaiveTime::from_hms_nano_opt(hour, minute, second, nanos)?))
}

/// `DateTime.ParseExact(s, formats, InvariantCulture, None)`: the first
/// format that matches.
pub fn parse_exact(s: &str, formats: &[&str]) -> Option<NaiveDateTime> {
    formats.iter().find_map(|f| parse_one(s, f))
}

/// `DateTime.Now.AddDays(2)`.
fn now_plus_two_days() -> NaiveDateTime {
    Local::now().naive_local() + chrono::Duration::days(2)
}

// ---------------------------------------------------------------------------
// Strings as FluentFTP splits and checks them.

/// `char.IsDigit` (Unicode decimal digits).
fn is_digit(c: char) -> bool {
    c.is_ascii_digit() || (!c.is_ascii() && c.is_numeric())
}

/// `string.SplitString()`: split on white space, empty fields dropped.
pub fn split_string(s: &str) -> Vec<String> {
    s.split(char::is_whitespace).filter(|p| !p.trim().is_empty()).map(str::to_owned).collect()
}

/// `string.IsNumeric()`: only digits and dots.
fn is_numeric(s: &str) -> bool {
    s.chars().filter(|c| *c != '.').all(is_digit)
}

/// What a .NET `Parse` throws.
#[derive(Debug, PartialEq, Eq)]
enum NumberError {
    Format,
    Overflow,
}

/// `long.Parse(s)` (`NumberStyles.Integer`).
fn parse_long(s: &str) -> Result<i64, NumberError> {
    let t = s.trim();
    let body = t.strip_prefix(['+', '-']).unwrap_or(t);
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit()) {
        return Err(NumberError::Format);
    }
    t.parse::<i64>().map_err(|_| NumberError::Overflow)
}

const OVERFLOW_LONG: &str = "Value was either too large or too small for an Int64.";
const OVERFLOW_INT: &str = "Value was either too large or too small for an Int32.";
const INDEX_OUT_OF_RANGE: &str = "Index was outside the bounds of the array.";
const NULL_REFERENCE: &str = "Object reference not set to an instance of an object.";

fn at(values: &[String], i: usize) -> Result<&str, String> {
    values.get(i).map(String::as_str).ok_or_else(|| INDEX_OUT_OF_RANGE.to_owned())
}

fn first(s: &str) -> Result<char, String> {
    s.chars().next().ok_or_else(|| INDEX_OUT_OF_RANGE.to_owned())
}

// ---------------------------------------------------------------------------
// FtpListItem and the paths.

/// `FtpObjectType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FtpObjectType {
    #[default]
    File,
    Directory,
    Link,
}

/// `FtpObjectSubType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FtpObjectSubType {
    #[default]
    Unknown,
    SubDirectory,
    SelfDirectory,
    ParentDirectory,
}

/// `FtpListItem` (what ShareX reads of it).
#[derive(Debug, Clone, PartialEq)]
pub struct FtpListItem {
    /// The line it was parsed from.
    pub input: String,
    name: Option<String>,
    pub full_name: Option<String>,
    pub kind: FtpObjectType,
    pub sub_type: FtpObjectSubType,
    /// `-1` when the listing doesn't give it.
    pub size: i64,
    /// `None` for `DateTime.MinValue`.
    pub modified: Option<NaiveDateTime>,
    /// The date is UTC (`DateTimeKind.Utc`: MLSD's); otherwise the server's
    /// local time, read as this machine's.
    pub modified_utc: bool,
    pub link_target: Option<String>,
}

impl Default for FtpListItem {
    fn default() -> Self {
        Self {
            input: String::new(),
            name: None,
            full_name: None,
            kind: FtpObjectType::File,
            sub_type: FtpObjectSubType::Unknown,
            size: -1,
            modified: None,
            modified_utc: false,
            link_target: None,
        }
    }
}

impl FtpListItem {
    /// `new FtpListItem(record, name, size, isDir, lastModifiedTime)`.
    fn new(record: &str, name: Option<String>, size: i64, is_dir: bool, modified: Option<NaiveDateTime>) -> Self {
        Self {
            input: record.to_owned(),
            name,
            size,
            kind: if is_dir { FtpObjectType::Directory } else { FtpObjectType::File },
            modified,
            ..Default::default()
        }
    }

    /// `Name`: the name given, else the last part of `FullName`.
    pub fn name(&self) -> Option<String> {
        match (&self.name, &self.full_name) {
            (None, Some(f)) => Some(get_ftp_file_name(f)),
            (n, _) => n.clone(),
        }
    }
}

/// `RemotePaths.IsAbsolutePath`.
fn is_absolute_path(p: &str) -> bool {
    p.starts_with('/') || p.starts_with("./") || p.starts_with("../")
}

/// `RemotePaths.IsFtpRootDirectory`.
pub fn is_ftp_root_directory(p: &str) -> bool {
    matches!(p, "." | "./" | "/")
}

fn collapse_slashes_and_trim(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for c in p.replace('\\', "/").chars() {
        if c == '/' && out.ends_with('/') {
            continue;
        }
        out.push(c);
    }
    out.trim_end_matches('/').to_owned()
}

/// `RemotePaths.AppendFtpPath(path, segment)`.
pub fn append_ftp_path(path: &str, segment: &str) -> String {
    let mut path = if path.is_empty() { "/".to_owned() } else { path.to_owned() };
    if !path.is_empty() && !path.ends_with('/') {
        path.push('/');
    }
    path.push_str(&collapse_slashes_and_trim(segment));
    let path = collapse_slashes_and_trim(&path);
    if path.is_empty() { "/".into() } else { path }
}

/// `RemotePaths.GetFtpFileName`.
pub fn get_ftp_file_name(p: &str) -> String {
    let i = match p.rfind('/') {
        Some(i) => i,
        None => match p.rfind('\\') {
            Some(i) => i,
            None => return p.to_owned(),
        },
    };
    if i + 1 >= p.len() { p.to_owned() } else { p[i + 1..].to_owned() }
}

/// `RemotePaths.GetFtpDirectoryName`.
pub fn get_ftp_directory_name(p: &str) -> String {
    let t = sanitize_slashes(p);
    if t.is_empty() || t == "/" {
        return "/".into();
    }
    match t.rfind('/') {
        None => ".".into(),
        Some(0) => "/".into(),
        Some(i) => t[..i].to_owned(),
    }
}

/// `SanitizerModule.SanitizeSlashes`.
pub fn sanitize_slashes(p: &str) -> String {
    if p.is_empty() {
        return String::new();
    }
    let mut p = p.replace('\\', "/");
    if p.contains("//") {
        p = collapse(&p);
    }
    if p.len() > 1 && p.ends_with('/') {
        p = p.trim_end_matches('/').to_owned();
    }
    if p.starts_with(char::is_whitespace) || p.ends_with(char::is_whitespace) {
        p = p.trim().to_owned();
    }
    if p.len() > 1 && p.ends_with('/') {
        p = p.trim_end_matches('/').to_owned();
    }
    p
}

fn collapse(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for c in p.chars() {
        if c == '/' && out.ends_with('/') {
            continue;
        }
        out.push(c);
    }
    out
}

/// `FtpSanitizeException`'s message.
fn sanitize_error(kind: &str, path: &str, option: &str) -> String {
    format!(
        "The FTP path \"{path}\" contains {kind} that might be used for FTP injection attacks! Set `{option}` to `false` to allow {kind}, or set `SanitizerMode` to `FtpSanitize.Rename` to silently cleanup such sequences."
    )
}

/// `SanitizerModule.SanitizePath` with FluentFTP's defaults (every check
/// on, `FtpSanitize.Throw`): the path, or the `FtpSanitizeException`'s
/// message.
pub fn sanitize_path(path: &str) -> Result<String, String> {
    if path.is_empty() {
        return Ok("/".into());
    }
    let mut p = path.replace('\\', "/");
    if p.contains("//") {
        p = collapse(&p);
    }
    if p.len() > 1 && p.ends_with('/') {
        p = p.trim_end_matches('/').to_owned();
    }
    if p.starts_with(char::is_whitespace) || p.ends_with(char::is_whitespace) {
        p = p.trim().to_owned();
    }
    if p.contains('%') {
        return Err(sanitize_error("URL encoded chars", &p, "SanitizeUrlEncoding"));
    }
    if p.chars().any(|c| c < ' ' || c == '\u{7f}' || c == ';' || c == '|') {
        return Err(sanitize_error("unix commands or newlines", &p, "SanitizeControlChars"));
    }
    if p.contains('\\') {
        p = p.replace('\\', "/");
    }
    if p.contains("//") {
        p = collapse(&p);
    }
    if p.chars().any(|c| ('\u{202a}'..='\u{202e}').contains(&c) || ('\u{2066}'..='\u{2069}').contains(&c)) {
        return Err(sanitize_error("unicode spoofing", &p, "SanitizeUnicodeSpoofing"));
    }
    if p.contains("..") {
        return Err(sanitize_error("FTP path traversal", &p, "SanitizeTraversal"));
    }
    if p.is_empty() {
        p = "/".into();
    }
    if p.len() > 1 && p.ends_with('/') {
        p = p.trim_end_matches('/').to_owned();
    }
    Ok(if p.is_empty() { "/".into() } else { p })
}

/// `CalculateFullFtpPath(client, path)` (the working directory when the
/// path is empty).
fn calculate_full_ftp_path(item: &mut FtpListItem, path: &str, working_directory: &str) {
    let mut path = path.to_owned();
    if get_ftp_file_name(&path).contains('*') {
        path = get_ftp_directory_name(&path);
    }
    if path.is_empty() {
        path = working_directory.to_owned();
    }
    if let Some(name) = item.name.clone() {
        if is_absolute_path(&name) {
            item.name = Some(get_ftp_file_name(&name));
            item.full_name = Some(name);
        } else {
            item.full_name = Some(append_ftp_path(&path, &name));
        }
    }
    if let Some(target) = item.link_target.clone()
        && !target.starts_with('/')
    {
        item.link_target = Some(if let Some(rest) = target.strip_prefix("./") {
            append_ftp_path(&path, rest).trim().to_owned()
        } else {
            append_ftp_path(&path, &target).trim().to_owned()
        });
    }
}

/// `ListingModule.IsItemSelf`: `.`, `..`, or the listed directory itself.
fn is_item_self(path: &str, item: &FtpListItem) -> Result<bool, String> {
    let name = item.name();
    if name.as_deref() == Some(".")
        || name.as_deref() == Some("..")
        || matches!(item.sub_type, FtpObjectSubType::ParentDirectory | FtpObjectSubType::SelfDirectory)
    {
        return Ok(true);
    }
    let full = item.full_name.as_deref().ok_or_else(|| NULL_REFERENCE.to_owned())?;
    Ok(if full.ends_with('/') { full == path } else { format!("{full}/") == path })
}

// ---------------------------------------------------------------------------
// Server handlers (`FtpHandlerIndex.AllServers`).

/// `FtpServer`: the server software FluentFTP recognises, by its welcome
/// message or its `SYST` reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FtpServer {
    #[default]
    Unknown,
    Apache,
    BFtpd,
    Cerberus,
    CrushFtp,
    FileZilla,
    FritzBox,
    Ftp2S3Gateway,
    GlFtpd,
    GlobalScapeEft,
    Homegate,
    IbmOs400,
    IbmZos,
    Idal,
    NonStopTandem,
    OpenVms,
    ProFtpd,
    PureFtpd,
    PyFtpdLib,
    Rumpus,
    ServU,
    Solaris,
    Titan,
    VsFtpd,
    WindowsCe,
    WindowsIis,
    WsFtp,
    WuFtpd,
    XLight,
}

impl FtpServer {
    const ALL: [FtpServer; 28] = [
        FtpServer::Apache,
        FtpServer::BFtpd,
        FtpServer::Cerberus,
        FtpServer::CrushFtp,
        FtpServer::FileZilla,
        FtpServer::FritzBox,
        FtpServer::Ftp2S3Gateway,
        FtpServer::GlFtpd,
        FtpServer::GlobalScapeEft,
        FtpServer::Homegate,
        FtpServer::IbmOs400,
        FtpServer::IbmZos,
        FtpServer::Idal,
        FtpServer::NonStopTandem,
        FtpServer::OpenVms,
        FtpServer::ProFtpd,
        FtpServer::PureFtpd,
        FtpServer::PyFtpdLib,
        FtpServer::Rumpus,
        FtpServer::ServU,
        FtpServer::Solaris,
        FtpServer::Titan,
        FtpServer::VsFtpd,
        FtpServer::WindowsCe,
        FtpServer::WindowsIis,
        FtpServer::WsFtp,
        FtpServer::WuFtpd,
        FtpServer::XLight,
    ];

    fn detect_by_welcome(self, m: &str) -> bool {
        match self {
            FtpServer::BFtpd => m.contains("bftpd "),
            FtpServer::Cerberus => m.contains("Cerberus FTP"),
            FtpServer::CrushFtp => m.contains("CrushFTP Server"),
            FtpServer::FileZilla => m.contains("FileZilla Server"),
            FtpServer::FritzBox => m.contains("FRITZ!Box"),
            FtpServer::Ftp2S3Gateway => m.contains("FTP2S3 gateway"),
            FtpServer::GlFtpd => m.contains("glFTPd "),
            FtpServer::GlobalScapeEft => m.contains("EFT Server"),
            FtpServer::Homegate => m.contains("Homegate FTP Server"),
            FtpServer::IbmOs400 => m.contains("QTCP at"),
            FtpServer::IbmZos => m.contains("IBM FTP CS"),
            FtpServer::Idal => m.contains("IDAL FTP server"),
            FtpServer::NonStopTandem => m.contains("FTP SERVER ") && m.contains(" TANDEM "),
            FtpServer::OpenVms => m.contains("OpenVMS FTPD"),
            FtpServer::ProFtpd => m.contains("ProFTPD"),
            FtpServer::PureFtpd => m.contains("Pure-FTPd"),
            FtpServer::PyFtpdLib => m.contains("pyftpdlib "),
            FtpServer::Rumpus => m.contains("Welcome To Rumpus"),
            FtpServer::ServU => m.contains("Serv-U FTP"),
            FtpServer::Titan => m.contains("Titan FTP"),
            FtpServer::VsFtpd => m.contains("(vsFTPd"),
            FtpServer::WindowsIis => m.contains("Microsoft FTP Service"),
            FtpServer::WsFtp => m.contains("WS_FTP Server"),
            FtpServer::WuFtpd => m.contains("Version wuftpd") || m.contains("Version wu-"),
            FtpServer::XLight => m.contains("Xlight FTP Server"),
            _ => false,
        }
    }

    fn detect_by_syst(self, m: &str) -> bool {
        match self {
            FtpServer::Apache => m.contains("Apache FtpServer"),
            FtpServer::IbmOs400 => m.contains("OS/400"),
            FtpServer::OpenVms => m.contains("OpenVMS"),
            FtpServer::Solaris => m.contains("SUNOS"),
            FtpServer::WindowsCe => m.contains("Windows_CE"),
            _ => false,
        }
    }

    /// `ServerModule.DetectFtpServer` (the welcome message).
    pub fn by_welcome(message: &str) -> FtpServer {
        FtpServer::ALL.into_iter().find(|s| s.detect_by_welcome(message)).unwrap_or_default()
    }

    /// `ServerModule.DetectFtpServerBySyst`.
    pub fn by_syst(system: &str) -> FtpServer {
        FtpServer::ALL.into_iter().find(|s| s.detect_by_syst(system)).unwrap_or_default()
    }

    /// `RecursiveList()`: the server lists recursively with `LIST -R`.
    pub fn recursive_list(self) -> bool {
        matches!(self, FtpServer::ProFtpd | FtpServer::PureFtpd)
    }

    /// `GetParser()`.
    pub fn parser(self) -> FtpParser {
        match self {
            FtpServer::IbmOs400 => FtpParser::IbmOs400,
            FtpServer::IbmZos => FtpParser::IbmZos,
            FtpServer::NonStopTandem => FtpParser::NonStop,
            FtpServer::OpenVms => FtpParser::Vms,
            FtpServer::WindowsCe | FtpServer::WindowsIis => FtpParser::Windows,
            _ => FtpParser::Auto,
        }
    }
}

/// `FtpOperatingSystem`, from `SYST`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FtpOperatingSystem {
    #[default]
    Unknown,
    Windows,
    Unix,
    Vms,
    IbmZos,
    IbmOs400,
    SunOs,
}

impl FtpOperatingSystem {
    /// `ServerModule.DetectFtpOSBySyst`.
    pub fn by_syst(system: &str) -> Self {
        let t = system.to_uppercase();
        if t.starts_with("WINDOWS") {
            Self::Windows
        } else if t.contains("Z/OS") {
            Self::IbmZos
        } else if t.contains("UNIX") || t.contains("AIX") {
            Self::Unix
        } else if t.contains("VMS") {
            Self::Vms
        } else if t.contains("OS/400") {
            Self::IbmOs400
        } else if t.contains("SUNOS") {
            Self::SunOs
        } else {
            Self::Unknown
        }
    }
}

/// `FtpParser`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FtpParser {
    #[default]
    Auto,
    Machine,
    Windows,
    Unix,
    UnixAlt,
    Vms,
    IbmZos,
    IbmOs400,
    NonStop,
}

/// `FtpCapability`: the features a `FEAT` line gives
/// (`ServerFeatureModule.Detect`).
pub fn detect_capability(line: &str) -> Option<&'static str> {
    let mut t = line.trim().to_uppercase();
    if let Some(r) = t.strip_prefix("211-") {
        t = r.to_owned();
    }
    const ORDER: &[(&str, &str)] = &[
        ("MLST", "MLST"),
        ("MLSD", "MLST"),
        ("MDTM", "MDTM"),
        ("REST STREAM", "REST"),
        ("SIZE", "SIZE"),
        ("UTF8", "UTF8"),
        ("PRET", "PRET"),
        ("MFMT", "MFMT"),
        ("MFCT", "MFCT"),
        ("MFF", "MFF"),
        ("MMD5", "MMD5"),
        ("XMD5", "XMD5"),
        ("XCRC", "XCRC"),
        ("XSHA1", "XSHA1"),
        ("XSHA256", "XSHA256"),
        ("XSHA512", "XSHA512"),
        ("EPSV", "EPSV"),
        ("CPSV", "CPSV"),
        ("NOOP", "NOOP"),
        ("CLNT", "CLNT"),
        ("SSCN", "SSCN"),
        ("SITE MKDIR", "SITE_MKDIR"),
        ("SITE RMDIR", "SITE_RMDIR"),
        ("SITE UTIME", "SITE_UTIME"),
        ("SITE SYMLINK", "SITE_SYMLINK"),
        ("AVBL", "AVBL"),
        ("THMB", "THMB"),
        ("RMDA", "RMDA"),
        ("DSIZ", "DSIZ"),
        ("HOST", "HOST"),
        ("CCC", "CCC"),
        ("MODE Z", "MODE_Z"),
        ("LANG", "LANG"),
        ("HASH", "HASH"),
    ];
    ORDER.iter().find(|(prefix, _)| t.starts_with(prefix)).map(|(_, c)| *c)
}

// ---------------------------------------------------------------------------
// The parsers.

/// What a parser needs of the client.
pub struct ParseContext<'a> {
    /// The working directory (`GetWorkingDirectory`).
    pub working_directory: &'a str,
    /// z/OS state.
    pub zos: &'a mut ZosState,
}

/// The z/OS listing realm and record length (`Status.zOSListingRealm`,
/// `zOSListingLRECL`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ZosState {
    pub realm: ZosRealm,
    pub lrecl: u16,
}

/// `FtpZOSListRealm`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ZosRealm {
    #[default]
    Invalid,
    Unix,
    Dataset,
    Member,
    MemberU,
    Jes2,
}

/// What a z/OS listing line asks of the server (`XDSS`).
pub enum ZosRequest {
    Xdss(String),
}

/// A line's result: an item, nothing, or (`FtpListParseException`) start
/// the listing again.
pub enum Parsed {
    Item(FtpListItem),
    Nothing,
    Restart,
}

mod unix {
    use super::*;

    const DATE1: [&str; 2] = ["MMM'-'d'-'yyyy", "MMM'-'dd'-'yyyy"];
    const DATE2: [&str; 5] =
        ["MMM'-'d'-'yyyy'-'HH':'mm", "MMM'-'dd'-'yyyy'-'HH':'mm", "MMM'-'d'-'yyyy'-'H':'mm", "MMM'-'dd'-'yyyy'-'H':'mm", "MMM'-'dd'-'yyyy'-'H'.'mm"];
    const ALT1: [&str; 2] = ["MMM'-'d'-'yyyy", "MMM'-'dd'-'yyyy"];
    const ALT2: [&str; 4] = ["MMM'-'d'-'yyyy'-'HH':'mm:ss", "MMM'-'dd'-'yyyy'-'HH':'mm:ss", "MMM'-'d'-'yyyy'-'H':'mm:ss", "MMM'-'dd'-'yyyy'-'H':'mm:ss"];

    pub fn is_valid(records: &[String]) -> bool {
        let (mut perm_first, mut perm_rest) = (false, false);
        for text in records.iter().take(10) {
            if text.trim().is_empty() {
                continue;
            }
            let a = split_string(text);
            if a.len() < 7 {
                continue;
            }
            let chars: Vec<char> = a[0].chars().collect();
            let c = chars[0].to_lowercase().next().unwrap_or(chars[0]);
            if c == '-' || c == 'l' || c == 'd' {
                perm_first = true;
            }
            if chars.len() > 1 {
                let c2 = chars[1].to_lowercase().next().unwrap_or(chars[1]);
                if c2 == 'r' || c2 == '-' {
                    perm_rest = true;
                }
            }
            if !perm_rest && chars.len() > 2 && chars[2..].contains(&'-') {
                perm_rest = true;
            }
        }
        perm_first && perm_rest
    }

    fn year_now() -> i32 {
        Local::now().year()
    }

    fn parse_date_time_year(stamp: &str, formats: &[&str]) -> Option<NaiveDateTime> {
        parse_exact(stamp, formats)
    }

    fn parse_date_time(stamp: &str, formats: &[&str]) -> Option<NaiveDateTime> {
        let mut dt = parse_exact(stamp, formats);
        if let Some(d) = dt
            && d > now_plus_two_days()
        {
            dt = d.with_year(d.year() - 1);
        }
        dt
    }

    /// `ParseName`: what follows the date fields in the record.
    fn parse_name(record: &str, values: &[String], is_link: bool, day_of_month: i32, date_pos: usize) -> Result<(Option<String>, Option<String>), String> {
        let mut num = 0usize;
        let count = if day_of_month > 0 { 2 } else { 3 };
        for i in date_pos..date_pos + count {
            let v = at(values, i)?;
            match record[num..].find(v) {
                Some(j) => num += j + v.len(),
                None => return Ok((None, None)),
            }
        }
        let text = record[num..].trim();
        if !is_link {
            return Ok((Some(text.to_owned()), None));
        }
        match text.find("->") {
            Some(i) if i > 0 => {
                let name = text[..i].trim().to_owned();
                let linked = if i + 2 < text.len() { Some(text[i + 2..].to_owned()) } else { None };
                Ok((Some(name), linked))
            }
            _ => Ok((Some(text.to_owned()), None)),
        }
    }

    pub fn parse(record: &str) -> Result<Option<FtpListItem>, String> {
        let c = first(record)?;
        if c != '-' && c != 'd' && c != 'l' {
            return Ok(None);
        }
        let values = split_string(record);
        if values.len() < 7 {
            return Ok(None);
        }
        let mut index = 0;
        let permissions = at(&values, index)?.to_owned();
        index += 1;
        let pc = first(&permissions)?;
        let (is_dir, is_link) = (pc == 'd', pc == 'l');
        // ParseLinkCount
        let mut link_count = 0i32;
        let v = at(&values, index)?;
        if is_digit(first(v)?) {
            match parse_long(v) {
                Ok(n) if i32::try_from(n).is_ok() => link_count = n as i32,
                Ok(_) | Err(NumberError::Overflow) => return Err(OVERFLOW_INT.into()),
                Err(NumberError::Format) => {}
            }
            index += 1;
        } else if first(v)? == '-' {
            index += 1;
        }
        // ParseOwnerGroup
        if is_numeric(at(&values, index + 2)?) && values.len() as isize - (index as isize + 2) > 4 {
            index += 2;
        } else if is_numeric(at(&values, index + 1)?) && values.len() as isize - (index as isize + 1) > 4 {
            index += 1;
        }
        // ParseFileSize
        let size_text = at(&values, index)?.replace('.', "");
        index += 1;
        let size = match parse_long(&size_text) {
            Ok(n) => n,
            Err(NumberError::Format) => 0,
            Err(NumberError::Overflow) => return Err(OVERFLOW_LONG.into()),
        };
        // ParseDayOfMonth
        let mut day_of_month = -1i32;
        let v = at(&values, index)?;
        if is_numeric(v) {
            match parse_long(v) {
                Ok(n) => {
                    if !(i32::MIN as i64..=i32::MAX as i64).contains(&n) {
                        return Err(OVERFLOW_INT.into());
                    }
                    day_of_month = n as i32;
                    if day_of_month > 31 {
                        day_of_month = -1;
                    }
                    if !first(at(&values, index + 1)?)?.is_alphabetic() {
                        day_of_month = -1;
                    }
                    if at(&values, index + 2)?.find(':').is_none_or(|i| i == 0) {
                        day_of_month = -1;
                    }
                }
                Err(NumberError::Format) => {}
                Err(NumberError::Overflow) => return Err(OVERFLOW_INT.into()),
            }
            index += 1;
        }
        let date_pos = index;
        // ParseDateTime
        let mut stamp = at(&values, index)?.to_owned();
        index += 1;
        stamp.push('-');
        if day_of_month > 0 {
            stamp.push_str(&day_of_month.to_string());
        } else {
            stamp.push_str(at(&values, index)?);
            index += 1;
        }
        stamp.push('-');
        let text = at(&values, index)?;
        let modified = if !text.contains(':') && !text.contains('.') {
            stamp.push_str(text);
            parse_date_time_year(&stamp, &DATE1)
        } else {
            stamp.push_str(&format!("{}-{text}", year_now()));
            parse_date_time(&stamp, &DATE2)
        };
        let (name, linked) = parse_name(record, &values, is_link, day_of_month, date_pos)?;
        let mut item = FtpListItem::new(record, name, size, is_dir, modified);
        if is_link {
            item.kind = FtpObjectType::Link;
            let _ = link_count;
            item.link_target = Some(linked.as_deref().map(str::trim).ok_or_else(|| NULL_REFERENCE.to_owned())?.to_owned());
        }
        Ok(Some(item))
    }

    /// `ParseUnixAlt` (a parser only chosen by hand; kept for completeness).
    pub fn parse_alt(record: &str) -> Result<Option<FtpListItem>, String> {
        let c = first(record)?;
        if c != '-' && c != 'd' && c != 'l' {
            return Ok(None);
        }
        let values = split_string(record);
        if values.len() < 8 {
            return Err(format!("Unexpected number of fields in listing '{record}' - expected minimum 8 fields but found {} fields", values.len()));
        }
        let mut num = 0;
        let perms = at(&values, num)?;
        num += 1;
        let pc = first(perms)?;
        let (is_dir, is_link) = (pc == 'd', pc == 'l');
        num += 1; // group
        if is_digit(first(at(&values, num)?)?) {
            num += 1;
        }
        num += 1; // owner
        let size = parse_long(at(&values, num)?).unwrap_or(0);
        num += 1;
        let date_pos = num;
        let mut stamp = format!("{}-{}-", at(&values, num)?, at(&values, num + 1)?);
        num += 2;
        let t = at(&values, num)?;
        let modified = if !t.contains(':') {
            stamp.push_str(t);
            parse_date_time_year(&stamp, &ALT1)
        } else {
            stamp.push_str(&format!("{}-{t}", year_now()));
            parse_date_time(&stamp, &ALT2)
        };
        let mut pos = 0usize;
        let mut found = true;
        for i in date_pos..date_pos + 3 {
            let v = at(&values, i)?;
            match record[pos..].find(v) {
                Some(j) => pos += j + v.len(),
                None => {
                    found = false;
                    break;
                }
            }
        }
        let name = found.then(|| record[pos..].trim().to_owned());
        let mut item = FtpListItem::new(record, name, size, is_dir, modified);
        if is_link {
            item.kind = FtpObjectType::Link;
        }
        Ok(Some(item))
    }
}

mod windows {
    use super::*;

    const FORMATS: [&str; 6] =
        ["MM'-'dd'-'yy hh':'mmtt", "MM'-'dd'-'yy HH':'mm", "MM'-'dd'-'yyyy hh':'mmtt", "yyyy'-'MM'-'dd hh':'mmtt", "yyyy'-'MM'-'dd HH':'mm", "yyyy'-'MM'-'dd hh':'mmtt"];

    pub fn is_valid(records: &[String]) -> bool {
        let (mut date, mut time, mut dir_or_size) = (false, false, false);
        for text in records.iter().take(10) {
            if text.trim().is_empty() {
                continue;
            }
            let a = split_string(text);
            if a.len() >= 4 {
                let c: Vec<char> = a[0].chars().collect();
                if is_digit(c[0]) && is_digit(c[c.len() - 1]) {
                    date = true;
                }
                if a[1].find(':').is_some_and(|i| i > 0) {
                    time = true;
                }
                if a[2].to_uppercase() == "<DIR>" || a[2].chars().next().is_some_and(is_digit) {
                    dir_or_size = true;
                }
            }
        }
        date && time && dir_or_size
    }

    pub fn parse(record: &str) -> Result<Option<FtpListItem>, String> {
        let values = split_string(record);
        if values.len() < 4 {
            return Ok(None);
        }
        let modified = parse_exact(&format!("{} {}", values[0], values[1]), &FORMATS);
        let (is_dir, size) = if values[2].to_uppercase() == "<DIR>" {
            (true, 0)
        } else {
            match parse_long(&values[2].replace(',', "")) {
                Ok(n) => (false, n),
                Err(NumberError::Format) => (false, 0),
                Err(NumberError::Overflow) => return Err(OVERFLOW_LONG.into()),
            }
        };
        let mut text = record.trim().to_owned();
        for _ in 0..3 {
            text = match text.find(' ') {
                Some(i) => text[i + 1..].trim().to_owned(),
                None => text.trim().to_owned(),
            };
        }
        Ok(Some(FtpListItem::new(record, Some(text), size, is_dir, modified)))
    }
}

mod machine {
    use super::*;

    const FTP_DATE_FORMATS: [&str; 8] =
        ["yyyyMMddHHmmss", "yyyyMMddHHmmss'.'f", "yyyyMMddHHmmss'.'ff", "yyyyMMddHHmmss'.'fff", "MMM dd  yyyy", "MMM  d  yyyy", "MMM dd HH:mm", "MMM  d HH:mm"];

    pub fn is_valid(records: &[String]) -> bool {
        records[0].to_ascii_lowercase().contains("type=")
    }

    /// `(?<name>.+?);` after `key=` (case-insensitive key).
    fn fact<'a>(record: &'a str, key: &str, digits_only: bool) -> Option<&'a str> {
        let lower = record.to_ascii_lowercase();
        let mut from = 0;
        while let Some(i) = lower[from..].find(key) {
            let start = from + i + key.len();
            let rest = &record[start..];
            if let Some(end) = rest.find(';') {
                let v = &rest[..end];
                if !v.is_empty() && (!digits_only || v.chars().all(|c| c.is_ascii_digit())) && !v.contains('\n') {
                    return Some(v);
                }
            }
            from = start;
        }
        None
    }

    pub fn parse(record: &str) -> Option<FtpListItem> {
        let mut item = FtpListItem::default();
        let kind = fact(record, "type=", false)?.to_lowercase();
        match kind.as_str() {
            "pdir" => {
                item.kind = FtpObjectType::Directory;
                item.sub_type = FtpObjectSubType::ParentDirectory;
            }
            "cdir" => {
                item.kind = FtpObjectType::Directory;
                item.sub_type = FtpObjectSubType::SelfDirectory;
            }
            "dir" => {
                item.kind = FtpObjectType::Directory;
                item.sub_type = FtpObjectSubType::SubDirectory;
            }
            "file" => item.kind = FtpObjectType::File,
            "link" | "slink" | "symlink" | "os.unix=link" | "os.unix=slink" | "os.unix=symlink" => item.kind = FtpObjectType::Link,
            _ => return None,
        }
        let first_line = record.split('\n').next().unwrap_or(record);
        let i = first_line.find("; ")?;
        item.name = Some(first_line[i + 2..].to_owned());
        if let Some(m) = fact(record, "modify=", false) {
            item.modified = parse_exact(m, &FTP_DATE_FORMATS);
            item.modified_utc = true;
        }
        if let Some(s) = fact(record, "size=", true)
            && let Ok(n) = parse_long(s)
        {
            item.size = n;
        }
        Some(item)
    }
}

mod vms {
    use super::*;

    pub fn is_valid(records: &[String]) -> bool {
        let (mut version, mut open, mut close) = (false, false, false);
        for text in records.iter().take(10) {
            if text.trim().is_empty() {
                continue;
            }
            if let Some(i) = text.find(';')
                && i > 0
                && text[i + 1..].chars().next().is_some_and(is_digit)
            {
                version = true;
            }
            open |= text.contains('[');
            close |= text.contains(']');
        }
        version && open && close
    }

    /// `DateTime.Parse` of VMS's `14-Jun-2004 10:27:08` forms.
    fn parse_date(date: &str, time: &str) -> Option<NaiveDateTime> {
        let mut s = String::new();
        let mut seen = false;
        for c in date.chars() {
            if !c.is_alphabetic() {
                s.push(c);
            } else if !seen {
                s.push(c);
                seen = true;
            } else {
                s.extend(c.to_lowercase());
            }
        }
        let s = format!("{s} {time}");
        parse_exact(&s, &["d-MMM-yyyy H:mm:ss", "d-MMM-yyyy H:mm", "d-MMM-yyyy H:mm:ss.ff", "d-MMM-yyyy H:mm:ss.f", "d-MMM-yy H:mm:ss", "d-MMM-yy H:mm"])
    }

    pub fn parse(record: &str) -> Result<Option<FtpListItem>, String> {
        let values = split_string(record);
        if values.is_empty() || (values.len() >= 2 && values[0] == "Directory") || values[0] == "Total" || values.len() < 4 {
            return Ok(None);
        }
        let text = &values[0];
        let Some(i) = text.rfind(';').filter(|i| *i > 0) else { return Ok(None) };
        let base = &text[..i];
        let (is_dir, name) = match base.strip_suffix(".DIR") {
            Some(n) => (true, n.to_owned()),
            None => (false, base.to_owned()),
        };
        let size_text = &values[1];
        let size = match size_text.find('/') {
            None => parse_long(size_text),
            Some(0) => parse_long(size_text).map(|n| n * 524288),
            Some(j) => parse_long(&size_text[..j]).map(|n| n * 524288),
        }
        .map_err(|e| if e == NumberError::Overflow { OVERFLOW_LONG.to_owned() } else { "The input string '".to_owned() + size_text + "' was not in a correct format." })?;
        let modified = parse_date(&values[2], &values[3]);
        Ok(Some(FtpListItem::new(record, Some(name), size, is_dir, modified)))
    }
}

mod nonstop {
    use super::*;

    fn is_header(line: &str) -> bool {
        line.contains("Code") && line.contains("EOF") && line.contains("RWEP")
    }

    pub fn is_valid(records: &[String]) -> bool {
        is_header(&records[0])
    }

    pub fn parse(record: &str) -> Result<Option<FtpListItem>, String> {
        if is_header(record) {
            return Ok(None);
        }
        let values = split_string(record);
        if values.len() < 7 {
            return Ok(None);
        }
        let modified = parse_exact(&format!("{} {}", values[3], values[4]), &["d'-'MMM'-'yy HH':'mm':'ss"]);
        let size = match parse_long(&values[2]) {
            Ok(n) => n,
            Err(NumberError::Format) => 0,
            Err(NumberError::Overflow) => return Err(OVERFLOW_LONG.into()),
        };
        Ok(Some(FtpListItem::new(record, Some(values[0].clone()), size, false, modified)))
    }
}

mod os400 {
    use super::*;

    const FORMATS: [[&str; 3]; 3] = [
        ["dd'/'MM'/'yy' 'HH':'mm':'ss", "dd'/'MM'/'yyyy' 'HH':'mm':'ss", "dd'.'MM'.'yy' 'HH':'mm':'ss"],
        ["yy'/'MM'/'dd' 'HH':'mm':'ss", "yyyy'/'MM'/'dd' 'HH':'mm':'ss", "yy'.'MM'.'dd' 'HH':'mm':'ss"],
        ["MM'/'dd'/'yy' 'HH':'mm':'ss", "MM'/'dd'/'yyyy' 'HH':'mm':'ss", "MM'.'dd'.'yy' 'HH':'mm':'ss"],
    ];

    /// `formatIndex`: static in FluentFTP, so it lasts for the process.
    static FORMAT_INDEX: AtomicUsize = AtomicUsize::new(0);

    pub fn is_valid(records: &[String]) -> bool {
        const KINDS: [&str; 6] = ["*DIR", "*FILE", "*FLR", "*DDIR", "*STMF", "*LIB"];
        records.iter().take(10).any(|l| KINDS.iter().any(|k| l.find(k).is_some_and(|i| i > 0)))
    }

    fn parse_date(stamp: &str) -> Result<Option<NaiveDateTime>, ()> {
        let start = FORMAT_INDEX.load(Ordering::Relaxed);
        if start >= FORMATS.len() {
            return Ok(None);
        }
        let mut dt = None;
        let mut idx = start;
        while idx < FORMATS.len() {
            match parse_exact(stamp, &FORMATS[idx]) {
                Some(d) if d > now_plus_two_days() => {
                    dt = Some(d);
                    idx += 1;
                }
                Some(d) => {
                    dt = Some(d);
                    break;
                }
                None => idx += 1,
            }
        }
        FORMAT_INDEX.store(idx, Ordering::Relaxed);
        if idx >= FORMATS.len() {
            return Ok(None);
        }
        if idx > start {
            return Err(());
        }
        Ok(dt)
    }

    pub fn parse(record: &str) -> Result<Parsed, String> {
        let values = split_string(record);
        if values.is_empty() {
            return Ok(Parsed::Nothing);
        }
        if values.len() >= 2 && values[1] == "*MEM" {
            let name = at(&values, 2)?.to_owned();
            return Ok(Parsed::Item(FtpListItem::new(record, Some(name), 0, false, None)));
        }
        if values.len() < 5 {
            return Ok(Parsed::Nothing);
        }
        let size = parse_long(&values[1]).map_err(|e| {
            if e == NumberError::Overflow { OVERFLOW_LONG.to_owned() } else { format!("The input string '{}' was not in a correct format.", values[1]) }
        })?;
        let modified = match parse_date(&format!("{} {}", values[2], values[3])) {
            Ok(d) => d,
            Err(()) => return Ok(Parsed::Restart),
        };
        let mut is_dir = values[4] == "*DIR" || values[4] == "*DDIR" || (values.len() == 5 && values[4] == "*FILE");
        let mut name = if values.len() < 6 {
            ".".to_owned()
        } else {
            let trimmed = record.trim_end();
            let i = trimmed.rfind(values[5].as_str()).unwrap_or(0);
            record[i..].to_owned()
        };
        if name.ends_with('/') {
            is_dir = true;
            name.pop();
        }
        Ok(Parsed::Item(FtpListItem::new(record, Some(name), size, is_dir, modified)))
    }
}

mod zos {
    use super::*;

    fn is_valid_hfs(record: &str) -> bool {
        let a = split_string(record);
        let Some(f) = a.first() else { return false };
        let c: Vec<char> = f.chars().collect();
        let c0 = c[0].to_lowercase().next().unwrap_or(c[0]);
        let first = c0 == '-' || c0 == 'l' || c0 == 'd';
        let second = c.len() > 1 && {
            let c1 = c[1].to_lowercase().next().unwrap_or(c[1]);
            c1 == 'r' || c1 == '-'
        };
        first && second
    }

    pub fn is_valid(listing: &[String]) -> bool {
        let l = &listing[0];
        l.contains("Volume Unit")
            || l.contains("Volume Referred")
            || l.contains("Name     VV.MM")
            || l.contains("Name      Size     TTR")
            || l.contains("total")
            || l.contains("JOBNAME  JOBID")
            || is_valid_hfs(l)
    }

    fn date(s: &str) -> Result<Option<NaiveDateTime>, String> {
        if s.is_empty() || s == "**NONE**" {
            return Ok(None);
        }
        parse_exact(s, &["yyyy'/'MM'/'dd HH':'mm"]).map(Some).ok_or_else(|| "String '".to_owned() + s + "' was not recognized as a valid DateTime.")
    }

    fn sub(s: &str, start: usize, len: usize) -> Result<&str, String> {
        s.get(start..start + len).ok_or_else(|| "Index and length must refer to a location within the string. (Parameter 'length')".to_owned())
    }

    /// The `XDSS` target of a member listing's path.
    fn xdss_path(path: &str, working_directory: &str) -> String {
        if path.is_empty() {
            working_directory.to_owned()
        } else if path.ends_with(")'") {
            format!("{}'", &path[..path.find('(').unwrap_or(0)])
        } else if path.ends_with(')') {
            path[..path.find('(').unwrap_or(0)].to_owned()
        } else {
            path.to_owned()
        }
    }

    pub fn parse(record: &str, path: &str, ctx: &mut ParseContext, xdss: &mut dyn FnMut(&str) -> Result<String, String>) -> Result<Option<FtpListItem>, String> {
        if record.contains("Volume Unit") || record.contains("Volume Referred") {
            ctx.zos.realm = ZosRealm::Dataset;
            return Ok(None);
        }
        if record.contains("Name     VV.MM") {
            let message = xdss(&xdss_path(path, ctx.working_directory))?;
            let field = message.split(' ').nth(5).ok_or_else(|| INDEX_OUT_OF_RANGE.to_owned())?;
            let lrecl = field.split('=').nth(1).ok_or_else(|| INDEX_OUT_OF_RANGE.to_owned())?;
            ctx.zos.lrecl = lrecl.parse().map_err(|_| format!("The input string '{lrecl}' was not in a correct format."))?;
            ctx.zos.realm = ZosRealm::Member;
            return Ok(None);
        }
        if record.contains("Name      Size     TTR") {
            ctx.zos.realm = ZosRealm::MemberU;
            return Ok(None);
        }
        if record.contains("JOBNAME  JOBID") {
            ctx.zos.realm = ZosRealm::Jes2;
            return Ok(None);
        }
        if record.contains("total") {
            ctx.zos.realm = ZosRealm::Unix;
            return Ok(None);
        }
        if is_valid_hfs(record) {
            ctx.zos.realm = ZosRealm::Unix;
        }
        match ctx.zos.realm {
            ZosRealm::Unix => unix::parse(record),
            ZosRealm::Dataset => {
                let k1 = sub(record, 53, 4)?.trim();
                if k1 == "PO" || k1 == "PS" || k1 == "PO_E" {
                    let referred = sub(record, 7, 10)?.trim();
                    let used = sub(record, 25, 9)?.trim();
                    let name = record[59.min(record.len())..].trim().split(' ').next().unwrap_or("").to_owned();
                    let is_dir = k1 == "PO" || k1 == "PO-E";
                    let stamp = if referred != "**NONE**" { format!("{referred} 00:00") } else { referred.to_owned() };
                    let modified = date(&stamp)?;
                    let size = parse_long(used).map_err(|_| format!("The input string '{used}' was not in a correct format."))? * 56664;
                    return Ok(Some(FtpListItem::new(record, Some(name), size, is_dir, modified)));
                }
                let k2 = sub(record, 51, 4)?.trim();
                if k2 == "PO" || k2 == "PS" || k2 == "PO-E" {
                    let referred = sub(record, 14, 10)?.trim();
                    let used = sub(record, 27, 5)?.trim();
                    let name = record[56.min(record.len())..].trim().split(' ').next().unwrap_or("").to_owned();
                    let is_dir = k2 == "PO" || k2 == "PO-E";
                    let stamp = if referred != "**NONE**" { format!("{referred} 00:00") } else { referred.to_owned() };
                    let modified = date(&stamp)?;
                    let size = if used != "+++++" {
                        parse_long(used).map_err(|_| format!("The input string '{used}' was not in a correct format."))? * 56664
                    } else {
                        950_664_167_424
                    };
                    return Ok(Some(FtpListItem::new(record, Some(name), size, is_dir, modified)));
                }
                Ok(None)
            }
            ZosRealm::Member => {
                let name = sub(record, 0, 8)?.trim().to_owned();
                let (stamp, count) = if record.trim_end().len() > 8 { (sub(record, 27, 16)?.trim().to_owned(), sub(record, 44, 5)?.trim().to_owned()) } else { (String::new(), "0".into()) };
                let modified = date(&stamp)?;
                let count: u16 = count.parse().map_err(|_| format!("The input string '{count}' was not in a correct format."))?;
                Ok(Some(FtpListItem::new(record, Some(name), i64::from(count) * i64::from(ctx.zos.lrecl), false, modified)))
            }
            ZosRealm::MemberU => {
                let name = sub(record, 0, 8)?.trim().to_owned();
                let hex = sub(record, 10, 6)?;
                let size = i64::from_str_radix(hex.trim(), 16).map_err(|_| format!("The input string '{hex}' was not in a correct format."))?;
                Ok(Some(FtpListItem::new(record, Some(name), size, false, None)))
            }
            ZosRealm::Jes2 => Ok(Some(FtpListItem::new(record, Some(record.trim().replace(' ', "_")), 0, false, None))),
            ZosRealm::Invalid => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// FtpListParser.

/// `FtpListParser`: the parser in use and whether a listing confirmed it.
#[derive(Debug, Clone, Copy, Default)]
pub struct ListParser {
    pub current: FtpParser,
    pub detected: FtpParser,
    pub confirmed: bool,
}

const AUTO_ORDER: [FtpParser; 6] = [FtpParser::Unix, FtpParser::Windows, FtpParser::Vms, FtpParser::IbmZos, FtpParser::IbmOs400, FtpParser::NonStop];

impl ListParser {
    /// `Init(system, forcedParser)`.
    pub fn init(&mut self, system: FtpOperatingSystem, forced: FtpParser) {
        self.confirmed = false;
        self.current = if forced != FtpParser::Auto {
            forced
        } else {
            match system {
                FtpOperatingSystem::Windows => FtpParser::Windows,
                FtpOperatingSystem::Unix | FtpOperatingSystem::SunOs => FtpParser::Unix,
                FtpOperatingSystem::Vms => FtpParser::Vms,
                FtpOperatingSystem::IbmZos => FtpParser::IbmZos,
                FtpOperatingSystem::IbmOs400 => FtpParser::IbmOs400,
                FtpOperatingSystem::Unknown => FtpParser::Unix,
            }
        };
        self.detected = self.current;
    }

    fn is_wrong_machine_listing(&self, has_mlst: bool) -> bool {
        self.current == FtpParser::Machine && !has_mlst
    }

    fn is_wrong_parser(&self, has_mlst: bool) -> bool {
        if self.current != FtpParser::Auto && self.confirmed { self.is_wrong_machine_listing(has_mlst) } else { true }
    }

    fn is_valid(p: FtpParser, files: &[String]) -> bool {
        match p {
            FtpParser::Windows => windows::is_valid(files),
            FtpParser::Unix => unix::is_valid(files),
            FtpParser::Vms => vms::is_valid(files),
            FtpParser::IbmZos => zos::is_valid(files),
            FtpParser::IbmOs400 => os400::is_valid(files),
            FtpParser::NonStop => nonstop::is_valid(files),
            FtpParser::Machine => machine::is_valid(files),
            _ => false,
        }
    }

    fn validate(&mut self, files: &[String], has_mlst: bool) {
        if !self.is_wrong_parser(has_mlst) {
            return;
        }
        if self.detected == FtpParser::Auto {
            self.detected = FtpParser::Unix;
        }
        if self.current == FtpParser::Auto {
            self.current = self.detected;
        }
        if self.is_wrong_machine_listing(has_mlst) {
            self.current = self.detected;
        }
        if Self::is_valid(self.current, files) {
            self.confirmed = true;
            return;
        }
        for p in AUTO_ORDER {
            if Self::is_valid(p, files) {
                self.current = p;
                self.confirmed = true;
                return;
            }
        }
        self.current = FtpParser::Unix;
    }

    /// `ParseSingleLine(path, line, capabilities, isMachineList)`, then the
    /// item's full path.
    pub fn parse_single_line(
        &mut self,
        path: &str,
        line: &str,
        has_mlst: bool,
        machine_list: bool,
        ctx: &mut ParseContext,
        xdss: &mut dyn FnMut(&str) -> Result<String, String>,
    ) -> Result<Parsed, String> {
        let parsed = if machine_list {
            machine::parse(line).map(Parsed::Item).unwrap_or(Parsed::Nothing)
        } else {
            if self.is_wrong_parser(has_mlst) {
                self.validate(&[line.to_owned()], has_mlst);
            }
            let item = match self.current {
                FtpParser::Machine => Ok(machine::parse(line)),
                FtpParser::Windows => windows::parse(line),
                FtpParser::Unix => unix::parse(line),
                FtpParser::UnixAlt => unix::parse_alt(line),
                FtpParser::Vms => vms::parse(line),
                FtpParser::IbmZos => zos::parse(line, path, ctx, xdss),
                FtpParser::IbmOs400 => return os400::parse(line).map(|p| finish(p, path, ctx.working_directory)),
                FtpParser::NonStop => nonstop::parse(line),
                FtpParser::Auto => Ok(None),
            }?;
            item.map(Parsed::Item).unwrap_or(Parsed::Nothing)
        };
        Ok(finish(parsed, path, ctx.working_directory))
    }
}

fn finish(parsed: Parsed, path: &str, working_directory: &str) -> Parsed {
    match parsed {
        Parsed::Item(mut item) => {
            calculate_full_ftp_path(&mut item, path, working_directory);
            Parsed::Item(item)
        }
        p => p,
    }
}

/// `GetListing`'s loop over the raw lines (`LoadBasicListingInfo`): lines
/// that start with a tab or space continue the line before; the
/// directory itself and its parent are left out.
pub fn parse_listing(
    parser: &mut ListParser,
    path: &str,
    raw: &[String],
    has_mlst: bool,
    machine_list: bool,
    ctx: &mut ParseContext,
    xdss: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<Vec<FtpListItem>, String> {
    let mut list = Vec::new();
    let mut i: isize = 0;
    while (i as usize) < raw.len() {
        let mut buf = raw[i as usize].clone();
        if (i as usize) + 1 < raw.len() && (raw[i as usize + 1].starts_with('\t') || raw[i as usize + 1].starts_with(' ')) {
            i += 1;
            buf.push_str(&raw[i as usize]);
        }
        match parser.parse_single_line(path, &buf, has_mlst, machine_list, ctx, xdss)? {
            Parsed::Restart => {
                list.clear();
                i = 0;
                continue;
            }
            Parsed::Item(item) => {
                if !is_item_self(path, &item)? {
                    list.push(item);
                }
            }
            Parsed::Nothing => {}
        }
        i += 1;
    }
    Ok(list)
}

/// `FtpSocketStream.ReadAllLines`: lines ended by `\n` (a last line
/// without one is dropped), `\r`/`\n` trimmed, blank lines left out.
pub fn read_all_lines(data: &[u8], utf8: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (i, b) in data.iter().enumerate() {
        if *b == b'\n' {
            let bytes = &data[start..=i];
            let text = if utf8 { String::from_utf8_lossy(bytes).into_owned() } else { bytes.iter().map(|b| if *b < 0x80 { *b as char } else { '?' }).collect() };
            let text = text.trim_matches(['\r', '\n']).to_owned();
            if !text.trim().is_empty() {
                lines.push(text);
            }
            start = i + 1;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_exact_formats() {
        let d = |y, m, d, h, mi, s| NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, mi, s).unwrap();
        assert_eq!(parse_exact("Mar-4-2025", &["MMM'-'d'-'yyyy"]), Some(d(2025, 3, 4, 0, 0, 0)));
        assert_eq!(parse_exact("Mar-04-2025", &["MMM'-'d'-'yyyy"]), Some(d(2025, 3, 4, 0, 0, 0)));
        assert_eq!(parse_exact("Mar-4-2025", &["MMM'-'dd'-'yyyy"]), None);
        assert_eq!(parse_exact("03-04-25 05:06AM", &["MM'-'dd'-'yy hh':'mmtt"]), Some(d(2025, 3, 4, 5, 6, 0)));
        assert_eq!(parse_exact("12-31-1999 11:59PM", &["MM'-'dd'-'yy hh':'mmtt", "MM'-'dd'-'yyyy hh':'mmtt"]), Some(d(1999, 12, 31, 23, 59, 0)));
        assert_eq!(parse_exact("20250304050607", &["yyyyMMddHHmmss"]), Some(d(2025, 3, 4, 5, 6, 7)));
        assert_eq!(parse_exact("03-04-75 12:00AM", &["MM'-'dd'-'yy hh':'mmtt"]), Some(d(1975, 3, 4, 0, 0, 0)));
    }

    #[test]
    fn sanitizer() {
        assert_eq!(sanitize_path("").unwrap(), "/");
        assert_eq!(sanitize_path("/a//b/").unwrap(), "/a/b");
        assert_eq!(sanitize_path("\\a\\b").unwrap(), "/a/b");
        assert!(sanitize_path("/a%20b").unwrap_err().contains("URL encoded chars"));
        assert!(sanitize_path("/x;y").unwrap_err().contains("unix commands or newlines"));
        assert!(sanitize_path("/../etc").unwrap_err().contains("FTP path traversal"));
    }

    #[test]
    fn unix_lines() {
        let item = unix::parse("drwxr-xr-x    2 1000     1000         4096 Mar 04  2025 dir one").unwrap().unwrap();
        assert_eq!(item.name().as_deref(), Some("dir one"));
        assert_eq!(item.kind, FtpObjectType::Directory);
        let item = unix::parse("-rw-r--r--    1 ftp      ftp      1.234.567 Jul 07 07:07 dotted size.bin").unwrap().unwrap();
        assert_eq!(item.size, 1234567);
        let item = unix::parse("lrwxrwxrwx    1 1000     1000           11 Feb 02  2024 link -> target/file").unwrap().unwrap();
        assert_eq!((item.name().as_deref(), item.link_target.as_deref()), (Some("link"), Some("target/file")));
        assert!(unix::parse("total 12").unwrap().is_none());
    }

    #[test]
    fn last_line_without_newline_is_dropped() {
        assert_eq!(read_all_lines(b"a\r\n\r\n b \nc", true), vec!["a".to_owned(), " b ".to_owned()]);
        assert_eq!(read_all_lines("é\n".as_bytes(), false), vec!["??".to_owned()]);
    }
}
