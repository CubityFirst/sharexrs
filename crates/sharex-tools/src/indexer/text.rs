//! `IndexerText`.

use chrono::NaiveDateTime;

use super::{FolderInfo, IndexerSettings, repeat, scan, size_string};
use crate::dotnet::{NEW_LINE, trim};
use crate::text::{Arg, format_dotnet};

fn index_folder(out: &mut String, dir: &FolderInfo, level: usize, s: &IndexerSettings) {
    let mut row = format!("{}{}", repeat(&s.indentation_text, level), dir.folder_name());
    if s.show_size_info && dir.size > 0 {
        row.push_str(&format!(" [{}]", size_string(dir.size, s.binary_units)));
    }
    out.push_str(&row);
    out.push_str(NEW_LINE);
    for sub in &dir.folders {
        if s.add_empty_line_after_folders {
            out.push_str(NEW_LINE);
        }
        index_folder(out, sub, level + 1, s);
    }
    if !dir.files.is_empty() {
        if s.add_empty_line_after_folders {
            out.push_str(NEW_LINE);
        }
        for f in &dir.files {
            let mut row = repeat(&s.indentation_text, level + 1) + &f.name;
            if s.show_size_info {
                row.push_str(&format!(" [{}]", size_string(f.length, s.binary_units)));
            }
            out.push_str(&row);
            out.push_str(NEW_LINE);
        }
    }
}

pub(super) fn index(folder_path: &str, s: &IndexerSettings, now: NaiveDateTime) -> String {
    let info = scan(folder_path, s);
    let mut content = String::new();
    index_folder(&mut content, &info, 0, s);
    let mut out = String::new();
    out.push_str(trim(&content));
    out.push_str(NEW_LINE);
    if s.add_footer {
        let footer = format_dotnet(
            sharex_strings::tools::IndexerText_Generated_by(),
            &[Arg::Date(now), Arg::Str(crate::WEBSITE.into())],
        );
        out.push_str(&"_".repeat(footer.encode_utf16().count()));
        out.push_str(NEW_LINE);
        out.push_str(&footer);
        out.push_str(NEW_LINE);
    }
    trim(&out).to_string()
}
