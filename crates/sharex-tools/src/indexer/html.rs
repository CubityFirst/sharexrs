//! `IndexerHtml` and `HtmlHelper`.

use chrono::NaiveDateTime;

use super::{FolderInfo, IndexerSettings, html_encode, scan, size_string};
use crate::dotnet::{self, NEW_LINE, trim};
use crate::text::{Arg, format_dotnet};

/// ShareX's embedded `IndexerDefault.css` (CRLF line ends, as built).
const DEFAULT_CSS: &str = include_str!("IndexerDefault.css");

fn start_tag(tag: &str, style: &str, fields: &str) -> String {
    let css = if style.is_empty() { String::new() } else { format!(" style=\"{style}\"") };
    let fields = if fields.is_empty() { String::new() } else { format!(" {fields}") };
    format!("<{tag}{css}{fields}>")
}

fn end_tag(tag: &str) -> String {
    format!("</{tag}>")
}

fn tag(tag: &str, content: &str, style: &str, fields: &str) -> String {
    start_tag(tag, style, fields) + &html_encode(content) + &end_tag(tag)
}

fn line(out: &mut String, s: &str) {
    out.push_str(s);
    out.push_str(NEW_LINE);
}

fn css(s: &IndexerSettings) -> String {
    let custom = (s.use_custom_css_file && !s.custom_css_file_path.is_empty())
        .then(|| std::fs::read(&s.custom_css_file_path).ok())
        .flatten()
        .map(|b| {
            // File.ReadAllText(path, UTF8): a BOM is skipped, invalid bytes replaced.
            let b = b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&b);
            String::from_utf8_lossy(b).into_owned()
        });
    let css = custom.unwrap_or_else(|| DEFAULT_CSS.replace("\r\n", "\n").replace('\n', "\r\n"));
    format!("<style type=\"text/css\">\r\n{css}\r\n</style>")
}

struct Html<'a> {
    s: &'a IndexerSettings,
    pre_path_trim: usize,
    out: String,
}

impl Html<'_> {
    fn folder_name_row(&self, dir: &FolderInfo, level: usize) -> String {
        let mut row = String::new();
        if !dir.is_empty() {
            if self.s.show_size_info {
                row += &size_string(dir.size, self.s.binary_units);
                row += " ";
            }
            row += "(";
            if dir.total_file_count > 0 {
                let f = if dir.total_file_count == 1 {
                    sharex_strings::tools::IndexerHtml_One_file()
                } else {
                    sharex_strings::tools::IndexerHtml_File_count()
                };
                row += &format_dotnet(f, &[Arg::I64(dir.total_file_count as i64)]);
            }
            if dir.total_folder_count > 0 {
                if dir.total_file_count > 0 {
                    row += ", ";
                }
                let f = if dir.total_folder_count == 1 {
                    sharex_strings::tools::IndexerHtml_One_folder()
                } else {
                    sharex_strings::tools::IndexerHtml_Folder_count()
                };
                row += &format_dotnet(f, &[Arg::I64(dir.total_folder_count as i64)]);
            }
            row += ")";
            row = format!(" {}", tag("span", &row, "", "class=\"FolderInfo\""));
        }
        let title = if self.s.display_path {
            if self.s.display_path_limited {
                dir.folder_path.get(self.pre_path_trim..).unwrap_or("").to_string()
            } else {
                dir.folder_path.clone()
            }
        } else {
            dir.folder_name().to_string()
        };
        let heading = (level + 1).clamp(1, 6);
        format!("{}{}{}{}", start_tag(&format!("h{heading}"), "", ""), html_encode(&title), row, end_tag(&format!("h{heading}")))
    }

    fn index_folder(&mut self, dir: &FolderInfo, level: usize) {
        let row = self.folder_name_row(dir, level);
        line(&mut self.out, &row);
        let class = if level > 0 { "FolderBorder" } else { "MainFolderBorder" };
        line(&mut self.out, &start_tag("div", "", &format!("class=\"{class}\"")));
        if !dir.files.is_empty() {
            line(&mut self.out, &start_tag("ul", "", ""));
            for f in &dir.files {
                let mut r = start_tag("li", "", "") + &html_encode(&f.name);
                if self.s.show_size_info {
                    r += " ";
                    r += &tag("span", &size_string(f.length, self.s.binary_units), "", "class=\"FileSize\"");
                }
                r += &end_tag("li");
                line(&mut self.out, &r);
            }
            line(&mut self.out, &end_tag("ul"));
        }
        for sub in &dir.folders {
            self.index_folder(sub, level + 1);
        }
        line(&mut self.out, &end_tag("div"));
    }
}

/// `Path.GetFullPath(folderPath).TrimEnd('\\')` (the platform's separator).
fn full_path(p: &str) -> String {
    let abs = std::path::absolute(p).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| p.to_string());
    let sep = std::path::MAIN_SEPARATOR;
    abs.trim_end_matches(sep).to_string()
}

pub(super) fn index(folder_path: &str, s: &IndexerSettings, now: NaiveDateTime) -> String {
    let mut out = String::new();
    line(&mut out, "<!DOCTYPE html>");
    line(&mut out, &start_tag("html", "", ""));
    line(&mut out, &start_tag("head", "", ""));
    line(&mut out, "<meta charset=\"UTF-8\">");
    line(&mut out, "<meta http-equiv=\"X-UA-Compatible\" content=\"IE=edge\">");
    let title = format_dotnet(sharex_strings::tools::IndexerHtml_Index_for(), &[Arg::Str(dotnet::file_name(folder_path).into())]);
    line(&mut out, &tag("title", &title, "", ""));
    line(&mut out, &css(s));
    line(&mut out, &end_tag("head"));
    line(&mut out, &start_tag("body", "", ""));
    let folder_path = full_path(folder_path);
    let pre_path_trim = folder_path.rfind(std::path::MAIN_SEPARATOR).map(|i| i + 1).unwrap_or(0);
    let info = scan(&folder_path, s);
    let mut h = Html { s, pre_path_trim, out: String::new() };
    h.index_folder(&info, 0);
    line(&mut out, trim(&h.out));
    if s.add_footer {
        let footer = format_dotnet(
            sharex_strings::tools::IndexerHtml_Generated_by_on(),
            &[Arg::Str(format!("<a href=\"{}\">ShareX Directory Indexer</a>", crate::WEBSITE)), Arg::Date(now)],
        );
        line(&mut out, &(start_tag("div", "", "") + &footer + &end_tag("div")));
    }
    line(&mut out, &end_tag("body"));
    line(&mut out, &end_tag("html"));
    trim(&out).to_string()
}
