//! Actions: `ShareX.HelpersLib/ExternalProgram.cs` (a program run on the
//! task's file, whose output can replace it) and its argument tokens,
//! `NameParser/CodeMenuEntryActions.cs` (`$input`, `$output`, and the
//! `%input`/`%output` forms that add quotes).
//!
//! [`ExternalProgram::run`] starts the program as ShareX's `Process` does
//! (`UseShellExecute = false`, `CreateNoWindow` for a hidden window) and
//! waits for it. On Windows the arguments reach `CreateProcess` as written
//! (`"<path>" <arguments>`); elsewhere they're split as .NET's
//! `Process.Unix` splits them ([`parse_arguments`]).

use std::path::{Path, PathBuf};

use crate::settings;

/// `CodeMenuEntryActions`' prefix (`Prefix = "$"`).
const PREFIX: &str = "$";

/// `CodeMenuEntryActions`: the argument tokens of an action, in the order
/// the arguments box's context menu lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeMenuEntryActions {
    /// `input`: the file's path.
    Input,
    /// `output`: the file's path with the output extension.
    Output,
}

impl CodeMenuEntryActions {
    pub const ALL: [Self; 2] = [Self::Input, Self::Output];

    /// `Value`.
    pub fn value(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }

    /// `Description`.
    pub fn description(self) -> &'static str {
        use sharex_strings::helpers_lib as h;
        match self {
            Self::Input => h::ActionsCodeMenuEntry_FilePath_File_path(),
            Self::Output => h::ActionsCodeMenuEntry_OutputFilePath_File_path_without_extension____Output_file_name_extension_(),
        }
    }

    /// `ToPrefixString()`: `$input`.
    pub fn to_prefix_string(self) -> String {
        self.to_prefix_string_with(PREFIX)
    }

    /// `ToPrefixString(prefix)`.
    pub fn to_prefix_string_with(self, prefix: &str) -> String {
        format!("{prefix}{}", self.value())
    }

    /// `CodeMenuEntryActions.Parse`: `%input` becomes the quoted input
    /// path, then `$input` the bare one; the same for the output.
    pub fn parse(pattern: &str, input_path: Option<&str>, output_path: Option<&str>) -> String {
        let mut result = pattern.to_owned();
        if let Some(input) = input_path {
            result = result.replace(&Self::Input.to_prefix_string_with("%"), &format!("\"{input}\""));
            result = result.replace(&Self::Input.to_prefix_string(), input);
        }
        if let Some(output) = output_path {
            result = result.replace(&Self::Output.to_prefix_string_with("%"), &format!("\"{output}\""));
            result = result.replace(&Self::Output.to_prefix_string(), output);
        }
        result
    }
}

settings! {
    /// `ShareX.HelpersLib.ExternalProgram`: an action.
    pub struct ExternalProgram {
        IsActive: bool = false,
        Name: Option<String> = None,
        Path: Option<String> = None,
        /// `'"' + CodeMenuEntryActions.input.ToPrefixString() + '"'`.
        Args: Option<String> = Some("\"$input\"".to_owned()),
        OutputExtension: Option<String> = None,
        Extensions: Option<String> = None,
        HiddenWindow: bool = false,
        DeleteInputFile: bool = false,
    }
}

/// What [`ExternalProgram::run`] gives: `Run`'s return value, and the
/// `pendingInputFilePath` it left.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunResult {
    /// The output file (or the input, when the output wasn't made); `None`
    /// when the action didn't run.
    pub path: Option<String>,
    /// The input file to delete once the output is taken
    /// (`DeletePendingInputFile`).
    pub pending_input_file: Option<String>,
}

impl ExternalProgram {
    /// `new ExternalProgram(name, path)`.
    pub fn new(name: &str, path: &str) -> Self {
        Self { Name: Some(name.to_owned()), Path: Some(path.to_owned()), ..Default::default() }
    }

    /// `GetFullPath()`: `FileHelpers.ExpandFolderVariables(Path)`.
    pub fn full_path(&self) -> Option<String> {
        self.Path.as_deref().map(crate::paths::expand_folder_variables)
    }

    /// `CheckExtension(path)`: the extensions filter.
    pub fn check_extension(&self, path: &str) -> bool {
        check_extension(path, self.Extensions.as_deref())
    }

    /// The output path `Run` uses for this input (already trimmed of its
    /// quotes): the input, or with `OutputExtension` when there are arguments.
    pub fn output_path(&self, input_path: &str) -> String {
        if self.Args.as_deref().is_none_or(str::is_empty) {
            return input_path.to_owned();
        }
        match self.OutputExtension.as_deref().filter(|e| !e.trim().is_empty()) {
            Some(ext) => {
                let mut out = path_combine(&get_directory_name(input_path), &get_file_name_without_extension(input_path));
                if !ext.starts_with('.') {
                    out.push('.');
                }
                out.push_str(ext);
                out
            }
            None => input_path.to_owned(),
        }
    }

    /// The arguments `Run` passes for this input.
    pub fn arguments(&self, input_path: &str) -> String {
        match self.Args.as_deref().filter(|a| !a.is_empty()) {
            None => format!("\"{input_path}\""),
            Some(args) => CodeMenuEntryActions::parse(args, Some(input_path), Some(&self.output_path(input_path))),
        }
    }

    /// `Run(inputPath)`: runs the program on the file when it exists and
    /// the file passes the extensions filter, waits for it to exit, and
    /// gives the output file when the program made it (the input
    /// otherwise). Nothing runs, and the path is `None`, when the program
    /// or the input is missing, the filter rejects the file or the program
    /// can't start.
    pub fn run(&self, input_path: &str) -> RunResult {
        let mut out = RunResult::default();
        let Some(path) = self.full_path().filter(|p| !p.is_empty()) else { return out };
        if !Path::new(&path).is_file() || input_path.trim().is_empty() {
            return out;
        }
        let input = input_path.trim_matches('"');
        if !self.check_extension(input) {
            return out;
        }
        let output = self.output_path(input);
        let arguments = self.arguments(input);
        eprintln!("Action input: \"{input}\" [{}]", file_size_readable(input));
        eprintln!("Action run: \"{path}\" {arguments}");
        if let Err(e) = start_and_wait(&path, &arguments, self.HiddenWindow) {
            eprintln!("Action failed: {e}");
            return out;
        }
        if !output.is_empty() && Path::new(&output).is_file() {
            eprintln!("Action output: \"{output}\" [{}]", file_size_readable(&output));
            if self.DeleteInputFile && !equals_ignore_case(input, &output) {
                out.pending_input_file = Some(input.to_owned());
            }
            out.path = Some(output);
        } else {
            out.path = Some(input.to_owned());
        }
        out
    }
}

/// `TaskHelpers.AddDefaultExternalPrograms`: the image editors ShareX
/// knows, each added (by name, once) when `search` finds it
/// (`RegistryHelpers.SearchProgramPath`).
pub fn add_default_external_programs(list: &mut Vec<ExternalProgram>, search: &dyn Fn(&str) -> Option<String>) {
    const DEFAULTS: [(&str, &str); 5] = [
        ("Paint", "mspaint.exe"),
        ("Paint.NET", "PaintDotNet.exe"),
        ("Adobe Photoshop", "Photoshop.exe"),
        ("IrfanView", "i_view32.exe"),
        ("XnView", "xnview.exe"),
    ];
    for (name, file_name) in DEFAULTS {
        if list.iter().any(|x| x.Name.as_deref() == Some(name)) {
            continue;
        }
        if let Some(path) = search(file_name).filter(|p| !p.is_empty()) {
            list.push(ExternalProgram::new(name, &path));
        }
    }
}

/// `StringExtensions.ParseQuoteString`: what's inside the first pair of
/// quotes (the rest after a lone quote), else the trimmed text.
pub fn parse_quote_string(text: &str) -> String {
    let s = text.trim();
    match s.find('"') {
        Some(first) => {
            let rest = &s[first + 1..];
            match rest.find('"') {
                Some(second) => rest[..second].to_owned(),
                None => rest.to_owned(),
            }
        }
        None => s.to_owned(),
    }
}

/// `DeletePendingInputFile()`: deletes the input file an action left
/// behind, when it still exists. Returns whether it was deleted.
pub fn delete_pending_input_file(path: Option<&str>) -> bool {
    let Some(path) = path.filter(|p| !p.is_empty() && Path::new(p).is_file()) else { return false };
    eprintln!("Deleting input file: \"{path}\"");
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("{e}");
            false
        }
    }
}

/// `ExternalProgram.CheckExtension(path, extensions)`: no filter passes
/// everything; otherwise the path must end with `.` and one of the runs of
/// letters and digits in `extensions` (any other character separates them),
/// ignoring case.
pub fn check_extension(path: &str, extensions: Option<&str>) -> bool {
    if path.trim().is_empty() {
        return false;
    }
    let Some(extensions) = extensions.filter(|e| !e.trim().is_empty()) else { return true };
    extensions
        .split(|c: char| !c.is_alphanumeric())
        .filter(|e| !e.is_empty())
        .any(|e| ends_with_ignore_case(path, &format!(".{e}")))
}

/// `StringComparison.OrdinalIgnoreCase`: chars compared by their simple
/// upper case.
fn upper(c: char) -> char {
    let mut u = c.to_uppercase();
    match (u.next(), u.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

fn ends_with_ignore_case(text: &str, suffix: &str) -> bool {
    let (t, s): (Vec<char>, Vec<char>) = (text.chars().collect(), suffix.chars().collect());
    t.len() >= s.len() && t[t.len() - s.len()..].iter().zip(&s).all(|(a, b)| upper(*a) == upper(*b))
}

fn equals_ignore_case(a: &str, b: &str) -> bool {
    a.chars().count() == b.chars().count() && a.chars().zip(b.chars()).all(|(x, y)| upper(x) == upper(y))
}

fn is_separator(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

/// The length of the path's root (`C:\`, `\\server\share\`, `/`), as far
/// as `Path.GetDirectoryName` needs it.
fn root_length(path: &str) -> usize {
    let b = path.as_bytes();
    if cfg!(windows) {
        if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
            return if b.len() >= 3 && is_separator(b[2] as char) { 3 } else { 2 };
        }
        if b.len() >= 2 && is_separator(b[0] as char) && is_separator(b[1] as char) {
            // `\\server\share\`.
            let mut seps = 0;
            for (i, c) in path.char_indices().skip(2) {
                if is_separator(c) {
                    seps += 1;
                    if seps == 2 {
                        return i + 1;
                    }
                }
            }
            return path.len();
        }
    }
    usize::from(b.first().is_some_and(|c| is_separator(*c as char)))
}

/// `Path.GetDirectoryName`: everything before the last separator (the root
/// keeps its own); empty for a bare file name.
pub fn get_directory_name(path: &str) -> String {
    let root = root_length(path);
    let Some(i) = path.rfind(is_separator) else { return String::new() };
    if i < root {
        return if root >= path.len() { String::new() } else { path[..root].to_owned() };
    }
    let mut end = i;
    // Separators run together count as one (`a\\b` gives `a`).
    while end > root && path[..end].ends_with(is_separator) {
        end -= 1;
    }
    let dir = &path[..end.max(root)];
    if cfg!(windows) { dir.replace('/', "\\") } else { dir.to_owned() }
}

/// `Path.GetFileNameWithoutExtension`: the file name up to its last dot.
pub fn get_file_name_without_extension(path: &str) -> String {
    let name = path.rsplit(is_separator).next().unwrap_or(path);
    match name.rfind('.') {
        Some(i) => name[..i].to_owned(),
        None => name.to_owned(),
    }
}

/// `Path.Combine(a, b)` for a relative `b`: an empty part adds nothing,
/// not even a separator (so `.hidden`'s output lands beside its folder, as
/// in ShareX).
fn path_combine(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_owned();
    }
    if b.is_empty() {
        return a.to_owned();
    }
    if a.ends_with(is_separator) || (cfg!(windows) && a.len() == 2 && a.ends_with(':')) {
        format!("{a}{b}")
    } else {
        format!("{a}{}{b}", std::path::MAIN_SEPARATOR)
    }
}

/// `FileHelpers.GetFileSizeReadable` for the log.
fn file_size_readable(path: &str) -> String {
    match std::fs::metadata(path) {
        Ok(m) => {
            let size = m.len() as f64;
            let units = ["B", "KB", "MB", "GB", "TB"];
            let place = if size < 1000.0 { 0 } else { ((size.ln() / 1000f64.ln()).floor() as usize).min(units.len() - 1) };
            if place == 0 { format!("{size} B") } else { format!("{:.2} {}", size / 1000f64.powi(place as i32), units[place]) }
        }
        Err(_) => String::new(),
    }
}

/// `Process.Start` with `UseShellExecute = false`, then `WaitForExit()`.
fn start_and_wait(path: &str, arguments: &str, hidden_window: bool) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(PathBuf::from(path));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // `"<FileName>" <Arguments>`, as .NET builds the command line.
        if !arguments.is_empty() {
            cmd.raw_arg(arguments);
        }
        if hidden_window {
            // `CreateNoWindow`.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = hidden_window;
        cmd.args(parse_arguments(arguments));
    }
    cmd.status().map(|_| ())
}

/// `Process.ParseArgumentsIntoList` (System.Diagnostics.Process on Unix):
/// the arguments string split as the Windows C runtime splits a command
/// line. Spaces and tabs separate arguments outside quotes; backslashes
/// are literal unless they precede a quote (pairs become one, an odd one
/// escapes the quote); `""` inside quotes is a quote.
pub fn parse_arguments(arguments: &str) -> Vec<String> {
    let c: Vec<char> = arguments.chars().collect();
    let mut results = Vec::new();
    let mut i = 0;
    while i < c.len() {
        while i < c.len() && (c[i] == ' ' || c[i] == '\t') {
            i += 1;
        }
        if i == c.len() {
            break;
        }
        // `GetNextArgument`.
        let mut arg = String::new();
        let mut in_quotes = false;
        while i < c.len() {
            let mut backslashes = 0;
            while i < c.len() && c[i] == '\\' {
                i += 1;
                backslashes += 1;
            }
            if backslashes > 0 {
                if i >= c.len() || c[i] != '"' {
                    arg.extend(std::iter::repeat_n('\\', backslashes));
                } else {
                    arg.extend(std::iter::repeat_n('\\', backslashes / 2));
                    if backslashes % 2 != 0 {
                        arg.push('"');
                        i += 1;
                    }
                }
                continue;
            }
            let ch = c[i];
            if ch == '"' {
                if in_quotes && i < c.len() - 1 && c[i + 1] == '"' {
                    arg.push('"');
                    i += 1;
                } else {
                    in_quotes = !in_quotes;
                }
                i += 1;
                continue;
            }
            if (ch == ' ' || ch == '\t') && !in_quotes {
                break;
            }
            arg.push(ch);
            i += 1;
        }
        results.push(arg);
        // The loop's own `i++` skips the separator the argument ended on.
        i += 1;
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_json() {
        let p = ExternalProgram::default();
        assert_eq!(p.Args.as_deref(), Some("\"$input\""));
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(
            json,
            r#"{"IsActive":false,"Name":null,"Path":null,"Args":"\"$input\"","OutputExtension":null,"Extensions":null,"HiddenWindow":false,"DeleteInputFile":false}"#
        );
        let back: ExternalProgram = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn parse_tokens() {
        let p = |s: &str| CodeMenuEntryActions::parse(s, Some("C:\\a b\\x.png"), Some("C:\\a b\\x.jpg"));
        assert_eq!(p("%input %output"), "\"C:\\a b\\x.png\" \"C:\\a b\\x.jpg\"");
        assert_eq!(p("$input -o $output"), "C:\\a b\\x.png -o C:\\a b\\x.jpg");
        assert_eq!(p("\"$input\""), "\"C:\\a b\\x.png\"");
        assert_eq!(CodeMenuEntryActions::parse("$input", None, None), "$input");
    }

    #[test]
    fn default_programs() {
        let mut list = vec![ExternalProgram::new("Paint", "C:\\mine\\paint.exe")];
        let found = |f: &str| match f {
            "mspaint.exe" => Some("C:\\Windows\\mspaint.exe".to_owned()),
            "xnview.exe" => Some("C:\\XnView\\xnview.exe".to_owned()),
            _ => None,
        };
        add_default_external_programs(&mut list, &found);
        let names: Vec<_> = list.iter().map(|x| (x.Name.clone().unwrap(), x.Path.clone().unwrap())).collect();
        assert_eq!(names, [("Paint".into(), "C:\\mine\\paint.exe".into()), ("XnView".into(), "C:\\XnView\\xnview.exe".into())]);
        assert_eq!(list[1].Args.as_deref(), Some("\"$input\""));
        assert!(!list[1].IsActive);
    }

    #[test]
    fn quote_strings() {
        assert_eq!(parse_quote_string(r#" "C:\a b\x.exe" "%1""#), r"C:\a b\x.exe");
        assert_eq!(parse_quote_string(r"C:\x.exe %1"), r"C:\x.exe %1");
        assert_eq!(parse_quote_string(r#""C:\x.exe"#), r"C:\x.exe");
    }

    #[test]
    fn extensions() {
        assert!(check_extension("a.png", None));
        assert!(check_extension("a.png", Some("  ")));
        assert!(check_extension("a.PNG", Some("jpg png")));
        assert!(check_extension("a.png", Some(".jpg,.png;")));
        assert!(!check_extension("a.png", Some("jpg")));
        assert!(!check_extension("apng", Some("png")));
        assert!(!check_extension(" ", None));
        assert!(check_extension("a.tar.gz", Some("tar.gz")));
    }

    #[test]
    fn output_paths() {
        let mut p = ExternalProgram { OutputExtension: Some("jpg".into()), ..Default::default() };
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(p.output_path("/d/x.png").replace(sep, "/"), "/d/x.jpg");
        p.OutputExtension = Some(".webp".into());
        assert_eq!(p.output_path("x.y.png"), "x.y.webp");
        assert_eq!(p.output_path("noext"), "noext.webp");
        p.Args = Some(String::new());
        assert_eq!(p.output_path("x.png"), "x.png");
        assert_eq!(p.arguments("x.png"), "\"x.png\"");
        assert_eq!(get_file_name_without_extension(".hidden"), "");
    }

    #[cfg(windows)]
    #[test]
    fn directory_names() {
        assert_eq!(get_directory_name("C:\\a\\b.png"), "C:\\a");
        assert_eq!(get_directory_name("C:\\b.png"), "C:\\");
        assert_eq!(get_directory_name("C:/a/b.png"), "C:\\a");
        assert_eq!(get_directory_name("b.png"), "");
        assert_eq!(get_directory_name("\\\\srv\\share\\b.png"), "\\\\srv\\share\\");
        assert_eq!(path_combine("C:\\", "b"), "C:\\b");
    }

    #[test]
    fn unix_argument_splitting() {
        assert_eq!(parse_arguments(r#""a b" c"#), ["a b", "c"]);
        assert_eq!(parse_arguments(r#"  x   y  "#), ["x", "y"]);
        assert_eq!(parse_arguments(r#"a\\b \"q\" "c\\" d"#), [r"a\\b", "\"q\"", "c\\", "d"]);
        assert_eq!(parse_arguments(r#""say ""hi""""#), [r#"say "hi""#]);
        assert_eq!(parse_arguments(r#""""#), [""]);
        assert!(parse_arguments("").is_empty());
    }
}
