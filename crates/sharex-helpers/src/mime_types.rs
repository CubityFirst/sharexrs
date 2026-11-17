//! `ShareX.HelpersLib/MimeTypes.cs`: the MIME type uploads send for a file,
//! from its extension (Apache's `mime.types` table, then on Windows the
//! registry's `HKEY_CLASSES_ROOT\.ext\Content Type`).

#[path = "mime_table.rs"]
mod table;

/// `MimeTypes.DefaultMimeType`.
pub const DEFAULT_MIME_TYPE: &str = "application/octet-stream";

/// `MimeTypes.GetMimeTypeFromExtension`: `extension` with or without its
/// dot, matched ignoring case.
pub fn get_mime_type_from_extension(extension: &str) -> Option<&'static str> {
    let extension = extension.strip_prefix('.').unwrap_or(extension);
    if extension.is_empty() {
        return None;
    }
    let key = extension.to_ascii_lowercase();
    table::MAPPINGS.binary_search_by(|(k, _)| (*k).cmp(key.as_str())).ok().map(|i| table::MAPPINGS[i].1)
}

/// `MimeTypes.GetMimeTypeFromFileName`.
pub fn get_mime_type_from_file_name(file_name: &str) -> String {
    let extension = path_get_extension(file_name).to_lowercase();
    if !extension.is_empty() {
        if let Some(mime) = get_mime_type_from_extension(&extension) {
            return mime.to_owned();
        }
        if let Some(mime) = registry_content_type(&extension).filter(|m| !m.is_empty()) {
            return mime;
        }
    }
    DEFAULT_MIME_TYPE.to_owned()
}

/// `MimeTypes.IsImageMimeType`.
pub fn is_image_mime_type(mime_type: &str) -> bool {
    ["png", "jpeg", "gif", "bmp"]
        .iter()
        .filter_map(|e| get_mime_type_from_extension(e))
        .any(|m| m.eq_ignore_ascii_case(mime_type))
}

/// .NET's `Path.GetExtension`: from the last `.` of the file name (after the
/// last directory separator) to the end, dot included; empty when there's
/// no dot or the dot is the last character.
pub fn path_get_extension(path: &str) -> &str {
    let separators: &[char] = if cfg!(windows) { &['/', '\\', ':'] } else { &['/'] };
    for (i, c) in path.char_indices().rev() {
        if c == '.' {
            return if i + 1 < path.len() { &path[i..] } else { "" };
        }
        if separators.contains(&c) {
            break;
        }
    }
    ""
}

#[cfg(windows)]
fn registry_content_type(extension: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_CLASSES_ROOT, RRF_RT_REG_SZ, RegGetValueW};

    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let key = wide(extension);
    let name = wide("Content Type");
    let mut size: u32 = 0;
    // SAFETY: valid NUL-terminated strings; a null buffer asks for the size.
    let status = unsafe {
        RegGetValueW(HKEY_CLASSES_ROOT, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut size)
    };
    if status != 0 || size == 0 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: `buf` holds `size` bytes.
    let status = unsafe {
        RegGetValueW(
            HKEY_CLASSES_ROOT,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

#[cfg(not(windows))]
fn registry_content_type(_extension: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_types_come_from_the_table() {
        assert_eq!(get_mime_type_from_file_name("a.PNG"), "image/png");
        assert_eq!(get_mime_type_from_file_name("dir.x/a.jpg"), "image/jpeg");
        assert_eq!(get_mime_type_from_file_name("a.txt"), "text/plain");
        assert_eq!(get_mime_type_from_file_name("noext"), DEFAULT_MIME_TYPE);
        assert_eq!(get_mime_type_from_file_name("a."), DEFAULT_MIME_TYPE);
        assert_eq!(get_mime_type_from_extension(".ZIP"), Some("application/zip"));
        assert!(is_image_mime_type("IMAGE/PNG"));
    }

    #[test]
    fn extensions_are_dotnets() {
        assert_eq!(path_get_extension("a.tar.gz"), ".gz");
        assert_eq!(path_get_extension("a.b/c"), "");
        assert_eq!(path_get_extension(".hidden"), ".hidden");
    }
}
