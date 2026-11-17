//! Hash checker: `HashChecker` (ShareX.HelpersLib) as `TaskHelpers.CalculateFileHashAsync`
//! runs it, and the comparison logic of `HashCheckerViewModel`.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use md5::Digest;

/// `HashCheckerAlgorithm` / `HashType`, in the window's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Crc32,
    Md5,
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl HashAlgorithm {
    pub const ALL: [HashAlgorithm; 6] = [
        HashAlgorithm::Crc32,
        HashAlgorithm::Md5,
        HashAlgorithm::Sha1,
        HashAlgorithm::Sha256,
        HashAlgorithm::Sha384,
        HashAlgorithm::Sha512,
    ];

    /// `HashCheckerViewModel.Algorithms`' display names.
    pub fn display_name(self) -> &'static str {
        match self {
            HashAlgorithm::Crc32 => "CRC-32",
            HashAlgorithm::Md5 => "MD5",
            HashAlgorithm::Sha1 => "SHA-1",
            HashAlgorithm::Sha256 => "SHA-256",
            HashAlgorithm::Sha384 => "SHA-384",
            HashAlgorithm::Sha512 => "SHA-512",
        }
    }

    /// The window selects SHA-256 first (`Algorithms[3]`).
    pub const DEFAULT: HashAlgorithm = HashAlgorithm::Sha256;
}

/// ShareX's `Crc32` (`HashAlgorithm`): the reflected 0xEDB88320 polynomial,
/// seed 0xFFFFFFFF, the final value inverted and written big-endian.
pub struct Crc32 {
    table: [u32; 256],
    hash: u32,
}

impl Default for Crc32 {
    fn default() -> Self {
        let mut table = [0u32; 256];
        for (i, t) in table.iter_mut().enumerate() {
            let mut e = i as u32;
            for _ in 0..8 {
                e = if e & 1 == 1 { (e >> 1) ^ 0xEDB8_8320 } else { e >> 1 };
            }
            *t = e;
        }
        Crc32 { table, hash: 0xFFFF_FFFF }
    }
}

impl Crc32 {
    pub fn update(&mut self, data: &[u8]) {
        for &b in data {
            self.hash = (self.hash >> 8) ^ self.table[((b as u32) ^ (self.hash & 0xFF)) as usize];
        }
    }

    pub fn finalize(&self) -> [u8; 4] {
        (!self.hash).to_be_bytes()
    }
}

enum Hasher {
    Crc32(Box<Crc32>),
    Md5(md5::Md5),
    Sha1(sha1::Sha1),
    Sha256(sha2::Sha256),
    Sha384(sha2::Sha384),
    Sha512(sha2::Sha512),
}

impl Hasher {
    fn new(a: HashAlgorithm) -> Self {
        match a {
            HashAlgorithm::Crc32 => Hasher::Crc32(Box::default()),
            HashAlgorithm::Md5 => Hasher::Md5(md5::Md5::new()),
            HashAlgorithm::Sha1 => Hasher::Sha1(sha1::Sha1::new()),
            HashAlgorithm::Sha256 => Hasher::Sha256(sha2::Sha256::new()),
            HashAlgorithm::Sha384 => Hasher::Sha384(sha2::Sha384::new()),
            HashAlgorithm::Sha512 => Hasher::Sha512(sha2::Sha512::new()),
        }
    }

    fn update(&mut self, d: &[u8]) {
        match self {
            Hasher::Crc32(h) => h.update(d),
            Hasher::Md5(h) => h.update(d),
            Hasher::Sha1(h) => h.update(d),
            Hasher::Sha256(h) => h.update(d),
            Hasher::Sha384(h) => h.update(d),
            Hasher::Sha512(h) => h.update(d),
        }
    }

    fn finish(self) -> Vec<u8> {
        match self {
            Hasher::Crc32(h) => h.finalize().to_vec(),
            Hasher::Md5(h) => h.finalize().to_vec(),
            Hasher::Sha1(h) => h.finalize().to_vec(),
            Hasher::Sha256(h) => h.finalize().to_vec(),
            Hasher::Sha384(h) => h.finalize().to_vec(),
            Hasher::Sha512(h) => h.finalize().to_vec(),
        }
    }
}

/// `TranslatorHelper.BytesToHexadecimal`, concatenated: lower-case hex.
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A buffer's hash as `HashChecker` reports it (lower-case hex).
pub fn hash_bytes(data: &[u8], algorithm: HashAlgorithm) -> String {
    let mut h = Hasher::new(algorithm);
    h.update(data);
    to_hex(&h.finish())
}

/// `HashChecker.Start(filePath, hashType)`: the file read 8 KiB at a time,
/// `progress` (0..100) reported at most every 200 ms and 100 at the end.
/// `None` when the file doesn't exist, can't be read, or `cancel` is set
/// (`progress` then gets 0).
pub fn hash_file(path: &str, algorithm: HashAlgorithm, mut progress: impl FnMut(f32), cancel: &AtomicBool) -> Option<String> {
    if path.is_empty() || !std::path::Path::new(path).is_file() {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let mut h = Hasher::new(algorithm);
    let mut buffer = vec![0u8; 8192];
    let mut total: u64 = 0;
    let mut timer = Instant::now();
    loop {
        let n = file.read(&mut buffer).ok()?;
        if n == 0 || cancel.load(Ordering::Relaxed) {
            break;
        }
        h.update(&buffer[..n]);
        total += n as u64;
        if timer.elapsed().as_millis() > 200 {
            progress(total as f32 / length as f32 * 100.0);
            timer = Instant::now();
        }
    }
    if cancel.load(Ordering::Relaxed) {
        progress(0.0);
        return None;
    }
    progress(100.0);
    Some(to_hex(&h.finish()))
}

/// `HashCheckerViewModel`: the result the window shows (upper case), and
/// whether it matches a pasted (or second file's) hash: both non-blank, equal
/// ignoring case after trimming the target.
pub fn display_hash(result: &str) -> String {
    result.to_uppercase()
}

/// `HashCheckerViewModel.HasComparison`.
pub fn has_comparison(result: &str, target: &str) -> bool {
    !result.trim().is_empty() && !target.trim().is_empty()
}

/// `HashCheckerViewModel.HashesMatch`.
pub fn hashes_match(result: &str, target: &str) -> bool {
    has_comparison(result, target) && result.eq_ignore_ascii_case(crate::dotnet::trim(target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        assert_eq!(hash_bytes(b"123456789", HashAlgorithm::Crc32), "cbf43926");
        assert_eq!(hash_bytes(b"", HashAlgorithm::Md5), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hash_bytes(b"abc", HashAlgorithm::Sha1), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert!(hashes_match("ABC", "  abc \n"));
        assert!(!hashes_match("ABC", " "));
    }
}
