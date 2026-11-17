//! Index folder: ShareX's directory indexer (`Indexer`, `IndexerText`,
//! `IndexerHtml`, `IndexerXml`, `IndexerJson`, `FolderInfo`, `IndexerSettings`).
//!
//! Folders and files are sorted as .NET's `string.CompareTo` sorts them (the
//! current culture through NLS on Windows, `sharex_helpers::culture`), with
//! `List<T>.Sort`'s introsort; hidden means `FileAttributes.Hidden` (a leading
//! dot elsewhere, as .NET reports it there). Lines end with
//! `Environment.NewLine`.

mod html;
mod json;
mod text;
mod xml;

use std::path::Path;

use chrono::NaiveDateTime;
use serde_json::Value;

use crate::dotnet;
use crate::options::{enum_name, json_bool, json_i32, json_str_or};

/// `IndexerOutput`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexerOutput {
    Txt,
    Html,
    Xml,
    Json,
}

impl IndexerOutput {
    const NAMES: [&'static str; 4] = ["Txt", "Html", "Xml", "Json"];

    /// The extension the window saves with (`OutputChoices`), also what
    /// `TaskHelpers.OpenDirectoryIndexer` names the uploaded file.
    pub fn extension(self) -> &'static str {
        match self {
            IndexerOutput::Txt => "txt",
            IndexerOutput::Html => "html",
            IndexerOutput::Xml => "xml",
            IndexerOutput::Json => "json",
        }
    }
}

/// `IndexerSettings` (`TaskSettingsTools.IndexerSettings`), plus the
/// application's `BinaryUnits` that `OpenDirectoryIndexer` copies in.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexerSettings {
    pub output: IndexerOutput,
    pub skip_hidden_folders: bool,
    pub skip_hidden_files: bool,
    pub skip_files: bool,
    pub max_depth_level: i32,
    pub show_size_info: bool,
    pub add_footer: bool,
    pub indentation_text: String,
    pub add_empty_line_after_folders: bool,
    pub use_custom_css_file: bool,
    pub display_path: bool,
    pub display_path_limited: bool,
    pub custom_css_file_path: String,
    pub use_attribute: bool,
    pub create_parseable_json: bool,
    pub binary_units: bool,
}

impl Default for IndexerSettings {
    fn default() -> Self {
        IndexerSettings {
            output: IndexerOutput::Html,
            skip_hidden_folders: true,
            skip_hidden_files: true,
            skip_files: false,
            max_depth_level: 0,
            show_size_info: true,
            add_footer: true,
            indentation_text: "|___".into(),
            add_empty_line_after_folders: false,
            use_custom_css_file: false,
            display_path: false,
            display_path_limited: false,
            custom_css_file_path: String::new(),
            use_attribute: true,
            create_parseable_json: true,
            binary_units: false,
        }
    }
}

impl IndexerSettings {
    /// From ShareX's JSON (`BinaryUnits` isn't serialised: pass it separately).
    pub fn from_json(v: &Value, binary_units: bool) -> Self {
        let d = Self::default();
        let b = |k: &str, def: bool| json_bool(&v[k]).unwrap_or(def);
        IndexerSettings {
            output: match enum_name(&v["Output"], &IndexerOutput::NAMES) {
                Some(0) => IndexerOutput::Txt,
                Some(1) => IndexerOutput::Html,
                Some(2) => IndexerOutput::Xml,
                Some(3) => IndexerOutput::Json,
                _ => d.output,
            },
            skip_hidden_folders: b("SkipHiddenFolders", d.skip_hidden_folders),
            skip_hidden_files: b("SkipHiddenFiles", d.skip_hidden_files),
            skip_files: b("SkipFiles", d.skip_files),
            max_depth_level: json_i32(&v["MaxDepthLevel"]).unwrap_or(d.max_depth_level),
            show_size_info: b("ShowSizeInfo", d.show_size_info),
            add_footer: b("AddFooter", d.add_footer),
            indentation_text: json_str_or(&v["IndentationText"], &d.indentation_text),
            add_empty_line_after_folders: b("AddEmptyLineAfterFolders", d.add_empty_line_after_folders),
            use_custom_css_file: b("UseCustomCSSFile", d.use_custom_css_file),
            display_path: b("DisplayPath", d.display_path),
            display_path_limited: b("DisplayPathLimited", d.display_path_limited),
            custom_css_file_path: json_str_or(&v["CustomCSSFilePath"], &d.custom_css_file_path),
            use_attribute: b("UseAttribute", d.use_attribute),
            create_parseable_json: b("CreateParseableJson", d.create_parseable_json),
            binary_units,
        }
    }

    /// ShareX's JSON, in its property order.
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "Output": IndexerOutput::NAMES[self.output as usize],
            "SkipHiddenFolders": self.skip_hidden_folders,
            "SkipHiddenFiles": self.skip_hidden_files,
            "SkipFiles": self.skip_files,
            "MaxDepthLevel": self.max_depth_level,
            "ShowSizeInfo": self.show_size_info,
            "AddFooter": self.add_footer,
            "IndentationText": self.indentation_text,
            "AddEmptyLineAfterFolders": self.add_empty_line_after_folders,
            "UseCustomCSSFile": self.use_custom_css_file,
            "DisplayPath": self.display_path,
            "DisplayPathLimited": self.display_path_limited,
            "CustomCSSFilePath": self.custom_css_file_path,
            "UseAttribute": self.use_attribute,
            "CreateParseableJson": self.create_parseable_json,
        })
    }
}

/// A file in the index (`FileInfo`'s name and length).
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub length: u64,
}

/// `FolderInfo`.
#[derive(Debug, Clone)]
pub struct FolderInfo {
    pub folder_path: String,
    pub files: Vec<FileEntry>,
    pub folders: Vec<FolderInfo>,
    pub size: u64,
    pub total_file_count: usize,
    pub total_folder_count: usize,
}

impl FolderInfo {
    fn new(path: &str) -> Self {
        FolderInfo { folder_path: path.into(), files: Vec::new(), folders: Vec::new(), size: 0, total_file_count: 0, total_folder_count: 0 }
    }

    /// `FolderName`: `Path.GetFileName(FolderPath)`.
    pub fn folder_name(&self) -> &str {
        dotnet::file_name(&self.folder_path)
    }

    /// `IsEmpty`.
    pub fn is_empty(&self) -> bool {
        self.total_file_count == 0 && self.total_folder_count == 0
    }

    /// `Update`: totals, sub folders sorted by name.
    fn update(&mut self) {
        for f in &mut self.folders {
            f.update();
        }
        dotnet::intro_sort(&mut self.folders, &mut |a, b| {
            sharex_helpers::culture::current_culture_cmp(a.folder_name(), b.folder_name())
        });
        self.size = self.folders.iter().map(|f| f.size).sum::<u64>() + self.files.iter().map(|f| f.length).sum::<u64>();
        self.total_file_count = self.files.len() + self.folders.iter().map(|f| f.total_file_count).sum::<usize>();
        self.total_folder_count = self.folders.len() + self.folders.iter().map(|f| f.total_folder_count).sum::<usize>();
    }
}

/// `FileAttributes.Hidden` as .NET reports it.
fn is_hidden(path: &Path, name: &str) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let _ = name;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        std::fs::symlink_metadata(path).map(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0).unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        name.starts_with('.')
    }
}

/// `Indexer.GetFolderInfo`.
fn folder_info(path: &str, level: i32, s: &IndexerSettings) -> FolderInfo {
    let mut info = FolderInfo::new(path);
    if s.max_depth_level == 0 || level < s.max_depth_level {
        let Ok(read) = std::fs::read_dir(path) else { return info };
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for entry in read.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            let name = entry.file_name().to_string_lossy().into_owned();
            let full = entry.path();
            // Directory symlinks/junctions enumerate as directories, as in .NET.
            let is_dir = kind.is_dir() || (kind.is_symlink() && full.is_dir());
            if is_dir {
                dirs.push((name, full));
            } else {
                files.push((name, full));
            }
        }
        for (name, full) in dirs {
            if s.skip_hidden_folders && is_hidden(&full, &name) {
                continue;
            }
            info.folders.push(folder_info(&full.to_string_lossy(), level + 1, s));
        }
        if !s.skip_files {
            for (name, full) in files {
                if s.skip_hidden_files && is_hidden(&full, &name) {
                    continue;
                }
                let length = std::fs::metadata(&full).map(|m| m.len()).unwrap_or(0);
                info.files.push(FileEntry { name, length });
            }
            dotnet::intro_sort(&mut info.files, &mut |a, b| sharex_helpers::culture::current_culture_cmp(&a.name, &b.name));
        }
    }
    info
}

/// The folder tree an index is made from (`GetFolderInfo` then `Update`).
pub fn scan(folder_path: &str, s: &IndexerSettings) -> FolderInfo {
    let mut info = folder_info(folder_path, 0, s);
    info.update();
    info
}

/// `Indexer.Index(folderPath, settings)`, with the footer's time.
pub fn index_at(folder_path: &str, s: &IndexerSettings, now_utc: NaiveDateTime) -> String {
    match s.output {
        IndexerOutput::Txt => text::index(folder_path, s, now_utc),
        IndexerOutput::Html => html::index(folder_path, s, now_utc),
        IndexerOutput::Xml => xml::index(folder_path, s),
        IndexerOutput::Json => json::index(folder_path, s),
    }
}

/// `Indexer.Index(folderPath, settings)`.
pub fn index(folder_path: &str, s: &IndexerSettings) -> String {
    index_at(folder_path, s, chrono::Utc::now().naive_utc())
}

/// `NumberExtensions.ToSizeString(size, binary)` (2 decimals, the current
/// culture's separator).
pub fn size_string(size: u64, binary: bool) -> String {
    let base: u64 = if binary { 1024 } else { 1000 };
    if size < base {
        return format!("{size} B");
    }
    let place = ((size as f64).ln() / (base as f64).ln()).floor() as i32;
    let num = size as f64 / (base as f64).powi(place);
    const DEC: [&str; 7] = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    const BIN: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let suffix = if binary { BIN[place as usize] } else { DEC[place as usize] };
    format!("{} {suffix}", crate::text::format_number(num, "0.00", false))
}

/// `"x".Repeat(n)`.
fn repeat(s: &str, n: usize) -> String {
    s.repeat(n)
}

/// `URLHelpers.HtmlEncode`: `HttpUtility.HtmlEncode` (which writes a
/// supplementary character as one entity), then every other character above
/// 127 as a decimal entity.
pub fn html_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if (c as u32) > 127 => out.push_str(&format!("&#{};", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(size_string(999, false), "999 B");
        assert_eq!(size_string(1000, false), "1.00 KB");
        assert_eq!(size_string(1536, true), "1.50 KiB");
        assert_eq!(size_string(1_234_567, false), "1.23 MB");
    }

    #[test]
    fn html() {
        assert_eq!(html_encode("a<b>&\"'é😀"), "a&lt;b&gt;&amp;&quot;&#39;&#233;&#128512;");
    }
}
