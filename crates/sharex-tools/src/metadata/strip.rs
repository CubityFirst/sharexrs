//! `MetadataStripper`: JPEG, PNG and GIF are rewritten without their metadata
//! segments (through a temporary file, keeping the creation time and
//! attributes); WebP, AVI, MP4/MOV, Matroska and ASF are edited in place,
//! metadata turned into padding of the same size (JUNK, free, Void, Padding).

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};

use super::asf;
use super::matroska;
use super::{MetadataFormat, ascii, detect, latin1};

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

/// `MetadataStripper.IsIsoVideoPath`.
pub fn is_iso_video_path(path: &str) -> bool {
    matches!(crate::dotnet::extension(path).to_lowercase().as_str(), ".mp4" | ".m4v" | ".mov" | ".3gp" | ".3g2" | ".mqv")
}

/// `MetadataStripper.Strip` (`MetadataService.StripMetadataAsync`).
pub fn strip(path: &str) -> io::Result<()> {
    if !std::path::Path::new(path).is_file() {
        return Err(io::Error::new(io::ErrorKind::NotFound, sharex_strings::tools::MetadataService_Selected_file_not_found()));
    }
    match detect(path)? {
        MetadataFormat::Jpeg => rewrite(path, rewrite_jpeg),
        MetadataFormat::Png => rewrite(path, rewrite_png),
        MetadataFormat::Gif => rewrite(path, rewrite_gif),
        MetadataFormat::WebP => strip_webp(path),
        MetadataFormat::Avi => strip_avi(path),
        MetadataFormat::IsoBaseMedia if is_iso_video_path(path) => strip_iso(path),
        MetadataFormat::Matroska => strip_matroska(path),
        MetadataFormat::Asf => strip_asf(path),
        _ => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("Metadata stripping is not supported for {} files.", crate::dotnet::extension(path)),
        )),
    }
}

// ------------------------------------------------------------ rewriting

fn rewrite(path: &str, f: fn(&[u8], &mut Vec<u8>) -> io::Result<bool>) -> io::Result<()> {
    let dir = crate::dotnet::directory_name(path).ok_or_else(|| invalid("The file has no parent directory."))?;
    let input = std::fs::read(path)?;
    let mut out = Vec::with_capacity(input.len());
    let changed = f(&input, &mut out)?;
    if !changed {
        return Ok(());
    }
    let meta = std::fs::metadata(path)?;
    let created = meta.created().ok();
    let tmp = crate::dotnet::combine(&dir, &format!(".{}.{:032x}.tmp", crate::dotnet::file_name(path), unique()));
    let result = (|| -> io::Result<()> {
        let mut t = File::options().write(true).create_new(true).open(&tmp)?;
        t.write_all(&out)?;
        t.sync_all()?;
        drop(t);
        let readonly = meta.permissions().readonly();
        if readonly {
            let mut p = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(false);
            std::fs::set_permissions(path, p)?;
        }
        std::fs::rename(&tmp, path)?;
        restore(path, created, &meta)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn unique() -> u128 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    t ^ ((std::process::id() as u128) << 64)
}

/// `File.SetCreationTimeUtc` and `File.SetAttributes` with the original's.
fn restore(path: &str, created: Option<std::time::SystemTime>, meta: &std::fs::Metadata) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::{FileTimesExt, MetadataExt};
        if let Some(c) = created {
            let f = File::options().write(true).open(path)?;
            f.set_times(std::fs::FileTimes::new().set_created(c))?;
        }
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            windows_sys::Win32::Storage::FileSystem::SetFileAttributesW(wide.as_ptr(), meta.file_attributes());
        }
    }
    #[cfg(not(windows))]
    {
        let _ = created;
        std::fs::set_permissions(path, meta.permissions())?;
    }
    Ok(())
}

fn rewrite_jpeg(d: &[u8], out: &mut Vec<u8>) -> io::Result<bool> {
    if !d.starts_with(&[0xFF, 0xD8]) {
        return Err(invalid("Invalid JPEG header."));
    }
    out.extend_from_slice(&[0xFF, 0xD8]);
    let mut changed = false;
    let mut pos = 2usize;
    while let Some(m) = super::image::next_marker(d, &mut pos) {
        if m == 0xD9 {
            out.extend_from_slice(&[0xFF, m]);
            return Ok(changed);
        }
        if m == 0x01 || (0xD0..=0xD7).contains(&m) {
            out.extend_from_slice(&[0xFF, m]);
            continue;
        }
        if pos + 2 > d.len() {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        let len_bytes = [d[pos], d[pos + 1]];
        pos += 2;
        let len = u16::from_be_bytes(len_bytes) as i64 - 2;
        if len < 0 || len as usize > d.len() - pos {
            return Err(invalid("Invalid JPEG segment length."));
        }
        let len = len as usize;
        if matches!(m, 0xE1 | 0xED | 0xFE) {
            changed = true;
        } else {
            out.extend_from_slice(&[0xFF, m]);
            out.extend_from_slice(&len_bytes);
            out.extend_from_slice(&d[pos..pos + len]);
        }
        pos += len;
        if m == 0xDA {
            out.extend_from_slice(&d[pos..]);
            return Ok(changed);
        }
    }
    Err(invalid("JPEG end marker was not found."))
}

fn rewrite_png(d: &[u8], out: &mut Vec<u8>) -> io::Result<bool> {
    if d.len() < 8 || d[..8] != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Err(invalid("Invalid PNG header."));
    }
    out.extend_from_slice(&d[..8]);
    let mut changed = false;
    let mut pos = 8usize;
    while pos + 12 <= d.len() {
        let size = u32::from_be_bytes(d[pos..pos + 4].try_into().unwrap()) as usize;
        let ty = ascii(&d[pos + 4..pos + 8]);
        if size > d.len() - pos - 8 - 4 {
            return Err(invalid("Invalid PNG chunk length."));
        }
        if matches!(ty.as_str(), "eXIf" | "tEXt" | "zTXt" | "iTXt" | "tIME") {
            changed = true;
        } else {
            out.extend_from_slice(&d[pos..pos + 8 + size + 4]);
        }
        pos += 8 + size + 4;
        if ty == "IEND" {
            return Ok(changed);
        }
    }
    Err(invalid("PNG end chunk was not found."))
}

struct Cursor<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn byte(&mut self) -> io::Result<u8> {
        let b = *self.d.get(self.pos).ok_or(io::ErrorKind::UnexpectedEof)?;
        self.pos += 1;
        Ok(b)
    }
    fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        if self.pos + n > self.d.len() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let s = &self.d[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn skip_sub_blocks(&mut self) -> io::Result<()> {
        loop {
            let n = self.byte()? as usize;
            if n == 0 {
                return Ok(());
            }
            self.pos += n;
        }
    }
    fn copy_sub_blocks(&mut self, out: &mut Vec<u8>) -> io::Result<()> {
        loop {
            let n = self.byte()?;
            out.push(n);
            if n == 0 {
                return Ok(());
            }
            let s = self.take(n as usize)?;
            out.extend_from_slice(s);
        }
    }
}

fn rewrite_gif(d: &[u8], out: &mut Vec<u8>) -> io::Result<bool> {
    let mut c = Cursor { d, pos: 0 };
    let header = c.take(13)?.to_vec();
    if &header[..6] != b"GIF87a" && &header[..6] != b"GIF89a" {
        return Err(invalid("Invalid GIF header."));
    }
    out.extend_from_slice(&header);
    if header[10] & 0x80 != 0 {
        out.extend_from_slice(c.take(3 << ((header[10] & 7) + 1))?);
    }
    let mut changed = false;
    while c.pos < d.len() {
        let intro = c.byte()?;
        match intro {
            0x3B => {
                out.push(intro);
                return Ok(changed);
            }
            0x2C => {
                out.push(intro);
                let desc = c.take(9)?.to_vec();
                out.extend_from_slice(&desc);
                if desc[8] & 0x80 != 0 {
                    out.extend_from_slice(c.take(3 << ((desc[8] & 7) + 1))?);
                }
                out.push(c.byte()?);
                c.copy_sub_blocks(out)?;
            }
            0x21 => {
                let label = c.byte()?;
                if label == 0xFE || label == 0x01 {
                    let first = c.byte()? as usize;
                    c.pos += first;
                    c.skip_sub_blocks()?;
                    changed = true;
                } else if label == 0xFF {
                    let size = c.byte()?;
                    let id = c.take(size as usize)?.to_vec();
                    if ascii(&id).starts_with("XMP DataXMP") {
                        c.skip_sub_blocks()?;
                        changed = true;
                    } else {
                        out.extend_from_slice(&[0x21, label, size]);
                        out.extend_from_slice(&id);
                        c.copy_sub_blocks(out)?;
                    }
                } else {
                    out.extend_from_slice(&[0x21, label]);
                    let first = c.byte()?;
                    out.push(first);
                    out.extend_from_slice(c.take(first as usize)?);
                    c.copy_sub_blocks(out)?;
                }
            }
            _ => return Err(invalid("Invalid GIF block.")),
        }
    }
    Err(invalid("GIF trailer was not found."))
}

// ------------------------------------------------------------ in place

fn write_zeros(f: &mut File, mut count: u64) -> io::Result<()> {
    let zeros = [0u8; 64 * 1024];
    while count > 0 {
        let n = count.min(zeros.len() as u64) as usize;
        f.write_all(&zeros[..n])?;
        count -= n as u64;
    }
    Ok(())
}

fn read8(f: &mut File, at: u64) -> io::Result<[u8; 8]> {
    let mut h = [0u8; 8];
    f.seek(SeekFrom::Start(at))?;
    f.read_exact(&mut h)?;
    Ok(h)
}

/// `OverwriteRiffChunkAsJunk`.
fn riff_junk(f: &mut File, offset: u64, total: u64) -> io::Result<()> {
    f.seek(SeekFrom::Start(offset))?;
    f.write_all(b"JUNK")?;
    f.seek(SeekFrom::Start(offset + 8))?;
    write_zeros(f, total - 8)
}

fn strip_webp(path: &str) -> io::Result<()> {
    let mut f = File::options().read(true).write(true).open(path)?;
    let len = f.metadata()?.len();
    let mut ranges = Vec::new();
    let mut flags_offset = None;
    let mut pos = 12u64;
    while pos + 8 <= len {
        let h = read8(&mut f, pos)?;
        let ty = ascii(&h[..4]);
        let size = u32::from_le_bytes(h[4..8].try_into().unwrap()) as u64;
        let end = pos + 8 + size + (size & 1);
        if end > len {
            return Err(invalid("Invalid WebP chunk length."));
        }
        if ty == "EXIF" || ty == "XMP " {
            ranges.push((pos, 8 + size + (size & 1)));
        } else if ty == "VP8X" && size >= 1 {
            flags_offset = Some(pos + 8);
        }
        pos = end;
    }
    for &(o, t) in &ranges {
        riff_junk(&mut f, o, t)?;
    }
    if let Some(fo) = flags_offset {
        if !ranges.is_empty() {
            let mut b = [0u8; 1];
            f.seek(SeekFrom::Start(fo))?;
            f.read_exact(&mut b)?;
            f.seek(SeekFrom::Start(fo))?;
            f.write_all(&[b[0] & !0x0C])?;
        }
    }
    f.sync_all()
}

fn strip_avi(path: &str) -> io::Result<()> {
    let mut f = File::options().read(true).write(true).open(path)?;
    let len = f.metadata()?.len();
    let mut ranges = Vec::new();
    avi_info_ranges(&mut f, 12, len, &mut ranges)?;
    for (o, t) in ranges {
        riff_junk(&mut f, o, t)?;
    }
    f.sync_all()
}

fn avi_info_ranges(f: &mut File, start: u64, end: u64, ranges: &mut Vec<(u64, u64)>) -> io::Result<()> {
    let mut pos = start;
    while pos + 8 <= end {
        let h = read8(f, pos)?;
        let ty = ascii(&h[..4]);
        let size = u32::from_le_bytes(h[4..8].try_into().unwrap()) as u64;
        let chunk_end = pos + 8 + size;
        let next = chunk_end + (size & 1);
        if chunk_end > end {
            return Err(invalid("Invalid AVI chunk length."));
        }
        if ty == "LIST" && size >= 4 {
            let mut list = [0u8; 4];
            f.read_exact(&mut list)?;
            if &list == b"INFO" {
                ranges.push((pos, 8 + size + (size & 1)));
            } else if &list == b"hdrl" || &list == b"odml" {
                avi_info_ranges(f, pos + 12, chunk_end, ranges)?;
            }
        }
        pos = next;
    }
    Ok(())
}

/// `TryReadIsoBox`: (type, offset, size, header size).
fn iso_box(f: &mut File, offset: u64, parent_end: u64) -> io::Result<Option<(String, u64, u64, u64)>> {
    f.seek(SeekFrom::Start(offset))?;
    let mut h = [0u8; 16];
    if f.read(&mut h[..8])? != 8 {
        return Ok(None);
    }
    let size32 = u32::from_be_bytes(h[..4].try_into().unwrap());
    let ty = latin1(&h[4..8]);
    let mut header = 8u64;
    let mut size = size32 as u64;
    if size32 == 1 {
        if f.read(&mut h[8..16])? != 8 {
            return Ok(None);
        }
        let ext = u64::from_be_bytes(h[8..16].try_into().unwrap());
        if ext > i64::MAX as u64 {
            return Ok(None);
        }
        size = ext;
        header = 16;
    } else if size32 == 0 {
        size = parent_end - offset;
    }
    if size < header || offset + size > parent_end {
        return Ok(None);
    }
    Ok(Some((ty, offset, size, header)))
}

fn strip_iso(path: &str) -> io::Result<()> {
    let mut f = File::options().read(true).write(true).open(path)?;
    let len = f.metadata()?.len();
    let mut ranges = Vec::new();
    let mut pos = 0u64;
    while pos + 8 <= len {
        let Some((ty, o, size, header)) = iso_box(&mut f, pos, len)? else { break };
        if ty == "moov" {
            iso_ranges(&mut f, o + header, o + size, "moov", &mut ranges)?;
        }
        pos = o + size;
    }
    for (o, size, header) in ranges {
        f.seek(SeekFrom::Start(o + 4))?;
        f.write_all(b"free")?;
        f.seek(SeekFrom::Start(o + header))?;
        write_zeros(&mut f, size - header)?;
    }
    f.sync_all()
}

fn iso_ranges(f: &mut File, start: u64, end: u64, parent: &str, ranges: &mut Vec<(u64, u64, u64)>) -> io::Result<()> {
    let mut pos = start;
    while pos + 8 <= end {
        let Some((ty, o, size, header)) = iso_box(f, pos, end)? else { break };
        if ty == "udta" || (ty == "meta" && (parent == "moov" || parent == "trak")) {
            ranges.push((o, size, header));
        } else if ty == "moov" || ty == "trak" {
            iso_ranges(f, o + header, o + size, &ty, ranges)?;
        }
        pos = o + size;
    }
    Ok(())
}

fn strip_matroska(path: &str) -> io::Result<()> {
    let mut f = File::options().read(true).write(true).open(path)?;
    let len = f.metadata()?.len();
    let Some((_, segment)) = matroska::find_segment(&mut f, len) else { return Ok(()) };
    let seg_end = if segment.unknown { len } else { segment.end() };
    let mut ranges = Vec::new();
    let mut pos = segment.payload;
    while pos < seg_end {
        let Some(e) = matroska::header_at(&mut f, pos, seg_end) else { break };
        if e.id == 0x1254_C367 {
            ranges.push(e);
        } else if !e.unknown && e.id == 0x1549_A966 {
            leaf_ranges(&mut f, &e, &[0x7BA9, 0x4461, 0x4D80, 0x5741, 0x7384, 0x3C83AB, 0x3E83BB], &mut ranges);
        } else if !e.unknown && e.id == 0x1654_AE6B {
            let mut p = e.payload;
            while p < e.end() {
                let Some(track) = matroska::header_at(&mut f, p, e.end()) else { break };
                if track.id == 0xAE && !track.unknown {
                    leaf_ranges(&mut f, &track, &[0x536E], &mut ranges);
                }
                if track.unknown {
                    break;
                }
                p = track.end();
            }
        }
        if e.unknown {
            break;
        }
        pos = e.end();
    }
    for e in ranges {
        void(&mut f, &e)?;
    }
    f.sync_all()
}

fn leaf_ranges(f: &mut File, parent: &matroska::Element, ids: &[u64], ranges: &mut Vec<matroska::Element>) {
    let mut p = parent.payload;
    while p < parent.end() {
        let Some(child) = matroska::header_at(f, p, parent.end()) else { break };
        if ids.contains(&child.id) {
            ranges.push(child);
        }
        if child.unknown {
            break;
        }
        p = child.end();
    }
}

/// `OverwriteAsEbmlVoid`: a Void element (0xEC) of the same total size.
fn void(f: &mut File, e: &matroska::Element) -> io::Result<()> {
    let total = e.total() as i64;
    let mut width = 1u32;
    let payload = loop {
        let payload = total - 1 - width as i64;
        let max = if width == 8 { (1u64 << 56) - 2 } else { (1u64 << (width * 7)) - 2 };
        if payload >= 0 && payload as u64 <= max {
            break payload as u64;
        }
        width += 1;
        if width > 8 {
            return Err(invalid("Matroska element is too large to replace with Void."));
        }
    };
    let mut header = vec![0xECu8];
    let encoded = payload | (1u64 << (width * 7));
    for i in (0..width).rev() {
        header.push((encoded >> (i * 8)) as u8);
    }
    f.seek(SeekFrom::Start(e.offset))?;
    f.write_all(&header)?;
    write_zeros(f, payload)
}

fn strip_asf(path: &str) -> io::Result<()> {
    let mut f = File::options().read(true).write(true).open(path)?;
    let len = f.metadata()?.len();
    let mut head = [0u8; 30];
    f.read_exact(&mut head)?;
    if head[..16] != asf::HEADER {
        return Ok(());
    }
    let header_end = asf::le64(&head, 16).min(len);
    let count = u32::from_le_bytes(head[24..28].try_into().unwrap());
    let mut ranges = Vec::new();
    let mut pos = 30u64;
    let mut i = 0;
    while i < count && pos + 24 <= header_end {
        let mut h = [0u8; 24];
        f.seek(SeekFrom::Start(pos))?;
        f.read_exact(&mut h)?;
        let size = asf::le64(&h, 16);
        if size < 24 || size > i64::MAX as u64 || pos + size > header_end {
            break;
        }
        let id = &h[..16];
        if id == asf::CONTENT_DESCRIPTION || id == asf::EXTENDED_CONTENT_DESCRIPTION {
            ranges.push((pos, size));
        } else if id == asf::HEADER_EXTENSION && size - 24 >= 22 {
            let mut sz = [0u8; 4];
            f.seek(SeekFrom::Start(pos + 24 + 18))?;
            f.read_exact(&mut sz)?;
            let ext = u32::from_le_bytes(sz) as u64;
            let mut p = pos + 24 + 22;
            let end = (p + ext).min(pos + size);
            while p + 24 <= end {
                let mut eh = [0u8; 24];
                f.seek(SeekFrom::Start(p))?;
                f.read_exact(&mut eh)?;
                let es = asf::le64(&eh, 16);
                if es < 24 || es > i64::MAX as u64 || p + es > end {
                    break;
                }
                if eh[..16] == asf::METADATA || eh[..16] == asf::METADATA_LIBRARY {
                    ranges.push((p, es));
                }
                p += es;
            }
        }
        pos += size;
        i += 1;
    }
    for (o, size) in ranges {
        f.seek(SeekFrom::Start(o))?;
        f.write_all(&asf::PADDING)?;
        f.seek(SeekFrom::Start(o + 24))?;
        write_zeros(&mut f, size - 24)?;
    }
    f.sync_all()
}
