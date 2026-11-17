//! What the history windows do with selected items, without the UI:
//! which context menu entries show (`HistoryContextMenuHelpers`), the text
//! the Copy entries put on the clipboard, what Enter/double click opens,
//! the Edit dialog's tag text, Rename's new name, Delete's wording, and the
//! Import folder dialog (`ImportFolder`).

use std::path::Path;

use sharex_helpers::file_helpers::{is_image_file, is_text_file, is_video_file};

use crate::datetime::{DateTime, LocalZone};
use crate::item::{HistoryItem, is_null_or_white_space};
use crate::paths;
use crate::tags::Tags;

/// The context menu's entries (`HistoryWindow.axaml` / `ImageHistoryWindow.axaml`
/// `x:Name`s, as `UpdateVisibility` switches on them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuEntry {
    OpenUrl,
    OpenShortUrl,
    OpenThumbnailUrl,
    OpenDeletionUrl,
    OpenFile,
    OpenFolder,
    Rename,
    CopyUrl,
    CopyShortUrl,
    CopyThumbnailUrl,
    CopyDeletionUrl,
    CopyFile,
    CopyImage,
    CopyText,
    CopyHtmlLink,
    CopyHtmlImage,
    CopyHtmlLinkedImage,
    CopyForumLink,
    CopyForumImage,
    CopyForumLinkedImage,
    CopyMarkdownLink,
    CopyMarkdownImage,
    CopyMarkdownLinkedImage,
    CopyFilePath,
    CopyFileName,
    CopyFileNameExtension,
    CopyFolder,
    Tag,
    Edit,
    Preview,
    Upload,
    EditImage,
    EditImageInPhotoEditor,
    TrimVideo,
    TrimAnimatedGif,
    Pin,
    Analyze,
    DeleteFile,
    /// Entries with no rule of their own (Favorite, Delete): any selection.
    Other,
}

/// Which `HistoryWindowServices` the app provides.
#[derive(Clone, Copy, Debug, Default)]
pub struct Services {
    pub upload_file: bool,
    pub edit_image: bool,
    pub edit_image_in_photo_editor: bool,
    pub trim_video: bool,
    pub trim_animated_gif: bool,
    pub pin_to_screen: bool,
    pub analyze_image: bool,
    pub show_image: bool,
}

fn set(s: &Option<String>) -> bool {
    !is_null_or_white_space(s.as_deref())
}

fn is_gif(path: &str) -> bool {
    paths::extension(path).eq_ignore_ascii_case(".gif")
}

/// `HistoryContextMenuHelpers.UpdateVisibility`'s `IsAvailable`. A submenu
/// shows when any of its entries does, and separators only between shown
/// entries.
pub fn menu_entry_available(
    entry: MenuEntry,
    selected: &[&HistoryItem],
    services: &Services,
    file_exists: impl Fn(&str) -> bool,
) -> bool {
    let single = selected.len() == 1;
    let item = if single { Some(selected[0]) } else { None };
    let path = item.and_then(|i| i.file_path.as_deref()).unwrap_or("");
    let exists = |i: &HistoryItem| i.file_path.as_deref().is_some_and(&file_exists);
    let file = item.is_some_and(exists);
    let image_file = file && is_image_file(path);
    let text_file = file && is_text_file(path);
    let any = |f: &dyn Fn(&HistoryItem) -> bool| selected.iter().any(|i| f(i));
    let has_url = any(&|i| set(&i.url));
    let image_url = |i: &HistoryItem| set(&i.url) && is_image_file(i.url.as_deref().unwrap());
    match entry {
        MenuEntry::OpenUrl => single && has_url,
        MenuEntry::OpenShortUrl => single && any(&|i| set(&i.shortened_url)),
        MenuEntry::OpenThumbnailUrl => single && any(&|i| set(&i.thumbnail_url)),
        MenuEntry::OpenDeletionUrl => single && any(&|i| set(&i.deletion_url)),
        MenuEntry::OpenFile | MenuEntry::OpenFolder | MenuEntry::Rename => file,
        MenuEntry::CopyUrl | MenuEntry::CopyHtmlLink | MenuEntry::CopyForumLink | MenuEntry::CopyMarkdownLink => has_url,
        MenuEntry::CopyShortUrl => any(&|i| set(&i.shortened_url)),
        MenuEntry::CopyThumbnailUrl => any(&|i| set(&i.thumbnail_url)),
        MenuEntry::CopyDeletionUrl => any(&|i| set(&i.deletion_url)),
        MenuEntry::CopyFile | MenuEntry::DeleteFile => any(&exists),
        MenuEntry::CopyImage => image_file,
        MenuEntry::CopyText => text_file,
        MenuEntry::CopyFilePath | MenuEntry::CopyFileName | MenuEntry::CopyFileNameExtension => any(&|i| set(&i.file_path)),
        MenuEntry::CopyFolder => any(&|i| !is_null_or_white_space(i.file_path.as_deref().and_then(paths::directory_name).as_deref())),
        MenuEntry::CopyHtmlImage | MenuEntry::CopyForumImage | MenuEntry::CopyMarkdownImage => any(&image_url),
        MenuEntry::CopyHtmlLinkedImage | MenuEntry::CopyForumLinkedImage | MenuEntry::CopyMarkdownLinkedImage => {
            any(&|i| image_url(i) && set(&i.thumbnail_url))
        }
        MenuEntry::Tag | MenuEntry::Edit => single,
        MenuEntry::Preview => image_file && services.show_image,
        MenuEntry::Upload => file && services.upload_file,
        MenuEntry::EditImage => image_file && services.edit_image,
        MenuEntry::EditImageInPhotoEditor => image_file && services.edit_image_in_photo_editor,
        MenuEntry::TrimVideo => file && is_video_file(path) && services.trim_video,
        MenuEntry::TrimAnimatedGif => file && is_gif(path) && services.trim_animated_gif,
        MenuEntry::Pin => image_file && services.pin_to_screen,
        MenuEntry::Analyze => image_file && services.analyze_image,
        MenuEntry::Other => !selected.is_empty(),
    }
}

/// The Copy entries' text formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyFormat {
    Url,
    ShortUrl,
    ThumbnailUrl,
    DeletionUrl,
    HtmlLink,
    HtmlImage,
    HtmlLinkedImage,
    ForumLink,
    ForumImage,
    ForumLinkedImage,
    MarkdownLink,
    MarkdownImage,
    MarkdownLinkedImage,
    FilePath,
    FileName,
    FileNameWithExtension,
    Folder,
}

/// `CopyValues`: each selected item's text (skipping the ones the format
/// doesn't apply to and blank results), one per line (`Environment.NewLine`).
/// `None` when there's nothing to copy.
pub fn copy_text(format: CopyFormat, selected: &[&HistoryItem]) -> Option<String> {
    let s = |o: &Option<String>| o.clone().unwrap_or_default();
    let has_url = |i: &HistoryItem| set(&i.url);
    let has_image_url = |i: &HistoryItem| has_url(i) && is_image_file(i.url.as_deref().unwrap());
    let has_linked = |i: &HistoryItem| has_image_url(i) && set(&i.thumbnail_url);
    let has_path = |i: &HistoryItem| set(&i.file_path);
    let values: Vec<String> = selected
        .iter()
        .filter(|i| match format {
            CopyFormat::HtmlLink | CopyFormat::ForumLink | CopyFormat::MarkdownLink => has_url(i),
            CopyFormat::HtmlImage | CopyFormat::ForumImage | CopyFormat::MarkdownImage => has_image_url(i),
            CopyFormat::HtmlLinkedImage | CopyFormat::ForumLinkedImage | CopyFormat::MarkdownLinkedImage => has_linked(i),
            CopyFormat::FilePath | CopyFormat::FileName | CopyFormat::FileNameWithExtension | CopyFormat::Folder => has_path(i),
            _ => true,
        })
        .filter_map(|i| {
            let (url, thumb, name, path) = (s(&i.url), s(&i.thumbnail_url), s(&i.file_name), s(&i.file_path));
            Some(match format {
                CopyFormat::Url => url,
                CopyFormat::ShortUrl => s(&i.shortened_url),
                CopyFormat::ThumbnailUrl => thumb,
                CopyFormat::DeletionUrl => s(&i.deletion_url),
                CopyFormat::HtmlLink => format!("<a href=\"{url}\">{url}</a>"),
                CopyFormat::HtmlImage => format!("<img src=\"{url}\"/>"),
                CopyFormat::HtmlLinkedImage => format!("<a href=\"{url}\"><img src=\"{thumb}\"/></a>"),
                CopyFormat::ForumLink => format!("[url]{url}[/url]"),
                CopyFormat::ForumImage => format!("[img]{url}[/img]"),
                CopyFormat::ForumLinkedImage => format!("[url={url}][img]{thumb}[/img][/url]"),
                CopyFormat::MarkdownLink => format!("[{name}]({url})"),
                CopyFormat::MarkdownImage => format!("![{name}]({url})"),
                CopyFormat::MarkdownLinkedImage => format!("[![{name}]({thumb})]({url})"),
                CopyFormat::FilePath => path,
                CopyFormat::FileName => paths::file_name_without_extension(&path).to_string(),
                CopyFormat::FileNameWithExtension => paths::file_name(&path).to_string(),
                CopyFormat::Folder => paths::directory_name(&path)?,
            })
        })
        .filter(|v| !is_null_or_white_space(Some(v)))
        .collect();
    (!values.is_empty()).then(|| values.join(crate::stats::NEW_LINE))
}

/// What Enter or a double click opens in the History window (`TryOpen`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenTarget {
    Url(String),
    File(String),
}

/// `TryOpen`: the shortened URL, else the URL, else the file if it exists.
pub fn try_open(item: &HistoryItem, file_exists: impl Fn(&str) -> bool) -> Option<OpenTarget> {
    if set(&item.shortened_url) {
        Some(OpenTarget::Url(item.shortened_url.clone().unwrap()))
    } else if set(&item.url) {
        Some(OpenTarget::Url(item.url.clone().unwrap()))
    } else {
        item.file_path.as_deref().filter(|p| !is_null_or_white_space(Some(p)) && file_exists(p)).map(|p| OpenTarget::File(p.into()))
    }
}

/// What the Image history window does on Enter or a double click
/// (`OpenSelectedItem`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageOpenAction {
    /// In the image viewer.
    ShowImage,
    /// With the system's app.
    OpenFile,
    /// After asking (`ImageHistoryWindow_ConfirmOpenFileFormat`).
    ConfirmOpenFile,
}

pub fn image_history_open(item: &HistoryItem, file_exists: impl Fn(&str) -> bool) -> Option<ImageOpenAction> {
    let path = item.file_path.as_deref().filter(|p| file_exists(p))?;
    Some(if is_image_file(path) {
        ImageOpenAction::ShowImage
    } else if is_text_file(path) || is_video_file(path) {
        ImageOpenAction::OpenFile
    } else {
        ImageOpenAction::ConfirmOpenFile
    })
}

/// The Edit dialog's tags box (`OnEditItemClick`): `key=value` lines sorted
/// by key (`OrderBy`, the current culture), a null value as nothing.
pub fn tags_to_edit_text(tags: Option<&Tags>) -> String {
    let Some(tags) = tags else { return String::new() };
    let mut pairs: Vec<(&str, Option<&str>)> = tags.iter().collect();
    pairs.sort_by(|a, b| crate::culture::current_culture_cmp(a.0, b.0));
    pairs.iter().map(|(k, v)| format!("{k}={}", v.unwrap_or(""))).collect::<Vec<_>>().join(crate::stats::NEW_LINE)
}

/// `HistoryWindow.ParseTags`: one `key=value` per line (CRLF or LF), the
/// key trimmed, a line without `=` a key with a null value, blank keys
/// skipped, a repeated key's last value kept.
pub fn parse_tags(text: &str) -> Tags {
    let mut tags = Tags::new();
    for line in text.split("\r\n").flat_map(|l| l.split('\n')).filter(|l| !l.is_empty()) {
        let (key, value) = match line.find('=') {
            Some(i) => (line[..i].trim(), Some(line[i + 1..].to_string())),
            None => (line.trim(), None),
        };
        if !is_null_or_white_space(Some(key)) {
            tags.set(key, value);
        }
    }
    tags
}

/// `OnRenameFileClick`: the new file name (with the old extension), or
/// `None` when the name is blank or the same ignoring case.
pub fn rename_target(file_path: &str, new_name: &str) -> Option<String> {
    let old = paths::file_name_without_extension(file_path);
    let new_name = new_name.trim();
    if is_null_or_white_space(Some(new_name)) || crate::culture::ordinal_ignore_case_eq(new_name, old) {
        return None;
    }
    Some(if paths::has_extension(file_path) { format!("{new_name}{}", paths::extension(file_path)) } else { new_name.to_string() })
}

/// `ConfirmDelete`'s message (`HistoryWindow_ConfirmDeleteFormat` around
/// this file/item or these N files/items) and title.
pub fn delete_confirmation(count: usize, delete_files: bool) -> (String, String) {
    use sharex_strings::history_lib as s;
    let noun = match (count == 1, delete_files) {
        (true, true) => s::HistoryWindow_ThisFile().to_string(),
        (true, false) => s::HistoryWindow_ThisItem().to_string(),
        (false, true) => s::HistoryWindow_TheseFilesFormat().replace("{0}", &count.to_string()),
        (false, false) => s::HistoryWindow_TheseItemsFormat().replace("{0}", &count.to_string()),
    };
    let title = if delete_files { s::HistoryWindow_DeleteFiles() } else { s::HistoryWindow_DeleteHistoryItems() };
    (title.to_string(), s::HistoryWindow_ConfirmDeleteFormat().replace("{0}", &noun))
}

/// The Import dialog (`ImportFolder`): every file under `folder` (images
/// only if asked), oldest change first, skipping paths already in history
/// (ignoring case) when asked, as new items: the name, the path, the last
/// write time, and `Image`, `Text` or `File`. Append them with
/// `append_history_items`.
pub fn import_folder(
    folder: &Path,
    only_images: bool,
    existing: &std::collections::HashSet<String>,
    zone: &dyn LocalZone,
) -> std::io::Result<Vec<HistoryItem>> {
    // `Directory.EnumerateFiles(..., AllDirectories)`: a folder's files,
    // then its subfolders' (breadth first).
    let mut files: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();
    let mut pending = std::collections::VecDeque::from([folder.to_path_buf()]);
    while let Some(dir) = pending.pop_front() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push_back(entry.path());
            } else if kind.is_file() {
                let path = entry.path();
                if only_images && !is_image_file(&path.to_string_lossy()) {
                    continue;
                }
                files.push((path, entry.metadata()?.modified()?));
            }
        }
    }
    files.sort_by_key(|(_, t)| *t);
    let existing: std::collections::HashSet<String> = existing.iter().map(|p| crate::culture::ordinal_upper(p)).collect();
    Ok(files
        .into_iter()
        .filter(|(p, _)| existing.is_empty() || !existing.contains(&crate::culture::ordinal_upper(&p.to_string_lossy())))
        .map(|(p, t)| {
            let path = p.to_string_lossy().into_owned();
            let ty = if is_image_file(&path) {
                "Image"
            } else if is_text_file(&path) {
                "Text"
            } else {
                "File"
            };
            HistoryItem {
                file_name: Some(paths::file_name(&path).to_string()),
                date_time: DateTime::from_system_time(t, zone),
                file_path: Some(path),
                r#type: Some(ty.to_string()),
                ..Default::default()
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_formats_and_menu() {
        let a = HistoryItem {
            file_name: Some("a.png".into()),
            file_path: Some(r"C:\s\a.png".into()),
            url: Some("https://i.example/a.png".into()),
            thumbnail_url: Some("https://t.example/a.png".into()),
            ..Default::default()
        };
        let b = HistoryItem { file_name: Some("b.txt".into()), url: Some("https://x/b".into()), ..Default::default() };
        let sel = [&a, &b];
        assert_eq!(copy_text(CopyFormat::MarkdownLinkedImage, &sel).unwrap(), "[![a.png](https://t.example/a.png)](https://i.example/a.png)");
        assert_eq!(
            copy_text(CopyFormat::ForumLink, &sel).unwrap(),
            format!("[url]https://i.example/a.png[/url]{}[url]https://x/b[/url]", crate::stats::NEW_LINE)
        );
        assert!(menu_entry_available(MenuEntry::CopyUrl, &sel, &Services::default(), |_| false));
        assert!(!menu_entry_available(MenuEntry::OpenUrl, &sel, &Services::default(), |_| false));
        assert!(menu_entry_available(MenuEntry::CopyImage, &[&a], &Services::default(), |_| true));
        assert_eq!(try_open(&b, |_| true), Some(OpenTarget::Url("https://x/b".into())));
    }

    #[test]
    fn tags_text() {
        let t = parse_tags("WindowTitle= x \r\nFavorite\n\n  =skip\nTag=a=b");
        assert_eq!(t.to_json(), r#"{"WindowTitle":" x ","Favorite":null,"Tag":"a=b"}"#);
        assert_eq!(tags_to_edit_text(Some(&t)), ["Favorite=", "Tag=a=b", "WindowTitle= x "].join(crate::stats::NEW_LINE));
        assert_eq!(rename_target(r"C:\s\a.png", " b "), Some("b.png".into()));
        assert_eq!(rename_target(r"C:\s\a.png", "A"), None);
    }
}
