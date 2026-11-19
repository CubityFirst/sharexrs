//! `ShareX.HelpersLib/Zip/ZipManager.cs` and `ZipEntryInfo.cs`: the zip
//! archives ShareX writes and reads (settings backups, `.sxie` image
//! effect packages).
//!
//! What .NET's `ZipArchive` does that matters is kept: entries deflated
//! (`CompressionLevel.Optimal`), a stream's entry stamped with the local time
//! now (`entry.LastWriteTime = DateTime.Now`) and a file's with its own
//! last write time (`CreateEntryFromFile`), an extracted file's last write
//! time set to its entry's, and `Extract`'s guards: the total uncompressed
//! size, rooted names skipped, names that leave the destination skipped.
//! `tools/settings-harness` checks it against ShareX's own `ZipManager`.

use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use chrono::{Datelike, Local, NaiveDateTime, TimeZone, Timelike};

/// `ZipEntryInfo`: a file to add (its name in the archive defaults to the
/// file's name) or bytes under a name.
#[derive(Debug, Clone)]
pub enum ZipEntryInfo {
    File { source: PathBuf, name: String },
    Data { data: Vec<u8>, name: String },
}

impl ZipEntryInfo {
    /// `new ZipEntryInfo(sourcePath)`: named `Path.GetFileName(sourcePath)`.
    pub fn file(source: impl Into<PathBuf>) -> Self {
        let source = source.into();
        let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Self::File { source, name }
    }

    /// `new ZipEntryInfo(stream, entryName)`.
    pub fn data(data: impl Into<Vec<u8>>, name: impl Into<String>) -> Self {
        Self::Data { data: data.into(), name: name.into() }
    }
}

/// .NET's DOS time stamp of a local time (two-second steps, 1980 to 2107).
fn dos_time(t: NaiveDateTime) -> zip::DateTime {
    zip::DateTime::from_date_and_time(t.year().clamp(1980, 2107) as u16, t.month() as u8, t.day() as u8, t.hour() as u8, t.minute() as u8, t.second() as u8)
        .unwrap_or_default()
}

/// A file's last write time as a local time.
fn local_mtime(path: &Path) -> Option<NaiveDateTime> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    Some(chrono::DateTime::<Local>::from(modified).naive_local())
}

/// `ZipManager.Compress(archivePath, entries)`: the archive is replaced; a
/// file entry whose file is missing is left out (`CreateEntry` returns
/// null).
pub fn compress(archive_path: &Path, entries: &[ZipEntryInfo]) -> io::Result<()> {
    if let Some(dir) = archive_path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    if archive_path.exists() {
        fs::remove_file(archive_path)?;
    }
    let file = fs::File::create(archive_path)?;
    compress_to(file, entries)?;
    Ok(())
}

/// [`compress`] into any writer.
pub fn compress_to<W: Write + Seek>(writer: W, entries: &[ZipEntryInfo]) -> io::Result<W> {
    let mut zip = zip::ZipWriter::new(writer);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in entries {
        match entry {
            ZipEntryInfo::Data { data, name } => {
                zip.start_file(name.as_str(), options.last_modified_time(dos_time(Local::now().naive_local())))?;
                zip.write_all(data)?;
            }
            ZipEntryInfo::File { source, name } => {
                if !source.is_file() {
                    continue;
                }
                let data = fs::read(source)?;
                let time = local_mtime(source).map(dos_time).unwrap_or_default();
                zip.start_file(name.as_str(), options.last_modified_time(time))?;
                zip.write_all(&data)?;
            }
        }
    }
    zip.finish().map_err(io::Error::other)
}

/// Why [`extract`] stopped.
#[derive(Debug)]
pub enum ExtractError {
    Io(io::Error),
    Zip(zip::result::ZipError),
    /// The entries add up to more than the limit: (total, limit).
    TooLarge(u64, u64),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractError::Io(e) => write!(f, "{e}"),
            ExtractError::Zip(e) => write!(f, "{e}"),
            ExtractError::TooLarge(total, max) => write!(f, "{}", too_large_message(*total, *max)),
        }
    }
}

impl std::error::Error for ExtractError {}

impl From<io::Error> for ExtractError {
    fn from(e: io::Error) -> Self {
        ExtractError::Io(e)
    }
}

impl From<zip::result::ZipError> for ExtractError {
    fn from(e: zip::result::ZipError) -> Self {
        ExtractError::Zip(e)
    }
}

/// The exception's message for an archive over the limit
/// (`ZipManager_Archive_too_large` and the two sizes, `ToSizeString()`).
fn too_large_message(total: u64, max: u64) -> String {
    use sharex_strings::helpers_lib as h;
    format!(
        "{}\r\n\r\n{}\r\n{}",
        h::ZipManager_Archive_too_large(),
        h::ZipManager_Archive_uncompressed_file_size().replace("{0}", &size_string(total)),
        h::ZipManager_Maximum_allowed_file_size().replace("{0}", &size_string(max)),
    )
}

/// `NumberExtensions.ToSizeString(size)` (decimal units, two decimals).
fn size_string(size: u64) -> String {
    const SUFFIXES: [&str; 7] = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    if size < 1000 {
        return format!("{size} B");
    }
    let place = ((size as f64).log(1000.0).floor() as usize).min(SUFFIXES.len() - 1);
    let num = size as f64 / 1000f64.powi(place as i32);
    format!("{num:.2} {}", SUFFIXES[place])
}

/// An entry's name: after its last `/` (and on Windows `\`), as
/// `ZipArchiveEntry.Name`.
pub fn entry_file_name(full_name: &str) -> &str {
    let cut = if cfg!(windows) { full_name.rfind(['/', '\\']) } else { full_name.rfind('/') };
    cut.map_or(full_name, |i| &full_name[i + 1..])
}

/// `Path.IsPathRooted`.
fn is_path_rooted(name: &str) -> bool {
    if cfg!(windows) {
        let b = name.as_bytes();
        name.starts_with(['/', '\\']) || (b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic())
    } else {
        name.starts_with('/')
    }
}

/// `Path.GetFullPath(Path.Combine(base, name))` for a relative `name`:
/// `.` and `..` resolved without touching the disk.
fn full_path(base: &Path, name: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    let parts: Vec<&str> = if cfg!(windows) { name.split(['/', '\\']).collect() } else { name.split('/').collect() };
    let last = parts.len().saturating_sub(1);
    for (i, part) in parts.iter().enumerate() {
        match *part {
            "" | "." => {
                // A trailing separator keeps the path a directory.
                if i == last && !part.is_empty() {
                    continue;
                }
            }
            ".." => {
                // Never above the root.
                if out.components().count() > 1 || !matches!(out.components().next(), Some(Component::RootDir | Component::Prefix(_))) {
                    out.pop();
                }
            }
            p => out.push(p),
        }
    }
    if name.ends_with('/') || (cfg!(windows) && name.ends_with('\\')) {
        // `Path.GetFileName` of a directory path is empty.
        let mut s = out.into_os_string();
        s.push(std::path::MAIN_SEPARATOR_STR);
        return PathBuf::from(s);
    }
    out
}

/// Which entries [`extract`] takes: given the entry's full name and its
/// name (`Func<ZipArchiveEntry, bool>`).
pub type EntryFilter<'a> = &'a dyn Fn(&str, &str) -> bool;

/// What [`extract`] did with each entry (for tests and logs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Extracted {
    File(PathBuf),
    Directory(PathBuf),
    Skipped(String),
}

/// `ZipManager.Extract(archivePath, destination, retainDirectoryStructure,
/// filter, maxUncompressedSize)`: every entry `filter` accepts is written
/// under `destination` (created), keeping its folders or not; a rooted name
/// or one that leaves `destination` is skipped; an entry ending in a
/// separator is a folder (made when empty). With `max_uncompressed_size`
/// above 0, an archive whose entries add up to more fails before anything
/// is written.
pub fn extract(
    archive_path: &Path,
    destination: &Path,
    retain_directory_structure: bool,
    filter: Option<EntryFilter>,
    max_uncompressed_size: u64,
) -> Result<Vec<Extracted>, ExtractError> {
    let file = fs::File::open(archive_path)?;
    extract_from(file, destination, retain_directory_structure, filter, max_uncompressed_size)
}

/// [`extract`] from any reader. `filter` gets the entry's full name and
/// its name.
pub fn extract_from<R: Read + Seek>(
    reader: R,
    destination: &Path,
    retain_directory_structure: bool,
    filter: Option<EntryFilter>,
    max_uncompressed_size: u64,
) -> Result<Vec<Extracted>, ExtractError> {
    let mut archive = zip::ZipArchive::new(reader)?;
    if max_uncompressed_size > 0 {
        let mut total: u64 = 0;
        for i in 0..archive.len() {
            total = total.saturating_add(archive.by_index_raw(i)?.size());
        }
        if total > max_uncompressed_size {
            return Err(ExtractError::TooLarge(total, max_uncompressed_size));
        }
    }
    fs::create_dir_all(destination)?;
    let full_name = std::path::absolute(destination)?;
    let mut prefix = full_name.clone().into_os_string();
    if !full_name.as_os_str().to_string_lossy().ends_with(std::path::MAIN_SEPARATOR) {
        prefix.push(std::path::MAIN_SEPARATOR_STR);
    }
    let prefix = prefix.to_string_lossy().into_owned();
    let mut out = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_owned();
        let short = entry_file_name(&name).to_owned();
        if filter.is_some_and(|f| !f(&name, &short)) {
            out.push(Extracted::Skipped(name));
            continue;
        }
        let entry_name = if retain_directory_structure { name.clone() } else { short };
        if is_path_rooted(&entry_name) {
            out.push(Extracted::Skipped(name));
            continue;
        }
        let path = full_path(&full_name, &entry_name);
        let text = path.to_string_lossy();
        if !text.starts_with(&prefix) {
            out.push(Extracted::Skipped(name));
            continue;
        }
        if text.ends_with(std::path::MAIN_SEPARATOR) {
            if entry.size() == 0 {
                fs::create_dir_all(&path)?;
                out.push(Extracted::Directory(path));
            } else {
                out.push(Extracted::Skipped(name));
            }
            continue;
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        // `MaxLengthStream(es, source.Length)`: no more than the entry says.
        let size = entry.size();
        (&mut entry).take(size).read_to_end(&mut data)?;
        fs::write(&path, &data)?;
        // `File.SetLastWriteTime(path, source.LastWriteTime.DateTime)`.
        if let Some(t) = entry.last_modified()
            && let Some(local) = chrono::NaiveDate::from_ymd_opt(t.year() as i32, t.month() as u32, t.day() as u32)
                .and_then(|d| d.and_hms_opt(t.hour() as u32, t.minute() as u32, t.second() as u32))
            && let Some(when) = Local.from_local_datetime(&local).earliest()
        {
            let _ = fs::File::options().write(true).open(&path).and_then(|f| f.set_modified(when.into()));
        }
        out.push(Extracted::File(path));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sharex-zip-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn round_trip_with_folders_and_guards() {
        let dir = scratch("round");
        let src = dir.join("History.db");
        fs::write(&src, b"sqlite").unwrap();
        let archive = dir.join("out").join("backup.sxb");
        compress(
            &archive,
            &[
                ZipEntryInfo::data(b"{}".to_vec(), "ApplicationConfig.json"),
                ZipEntryInfo::file(&src),
                ZipEntryInfo::file(dir.join("missing.db")),
                ZipEntryInfo::data(b"x".to_vec(), "sub/inner.json"),
                ZipEntryInfo::data(b"y".to_vec(), "../evil.json"),
            ],
        )
        .unwrap();
        let dest = dir.join("dest");
        let filter = |_: &str, name: &str| crate::file_helpers::check_extension(name, &["json", "xml"]);
        let got = extract(&archive, &dest, true, Some(&filter), 1_000_000_000).unwrap();
        assert_eq!(fs::read(dest.join("ApplicationConfig.json")).unwrap(), b"{}");
        assert_eq!(fs::read(dest.join("sub").join("inner.json")).unwrap(), b"x");
        assert!(!dest.join("History.db").exists());
        assert!(!dir.join("evil.json").exists());
        assert_eq!(got.iter().filter(|e| matches!(e, Extracted::File(_))).count(), 2);
        // The size limit counts every entry, filtered or not.
        assert!(matches!(extract(&archive, &dest, true, Some(&filter), 5), Err(ExtractError::TooLarge(10, 5))));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sizes_as_dotnet_writes_them() {
        assert_eq!(size_string(999), "999 B");
        assert_eq!(size_string(1_000_000_000), "1.00 GB");
        assert_eq!(size_string(1536), "1.54 KB");
    }

    #[test]
    fn rooted_names() {
        assert!(is_path_rooted("/a.json"));
        assert!(!is_path_rooted("a/b.json"));
        assert_eq!(entry_file_name("a/b.json"), "b.json");
    }
}
