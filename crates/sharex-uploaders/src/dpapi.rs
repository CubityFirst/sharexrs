//! `[JsonEncrypt]` properties (`DPAPIEncryptedStringValueProvider`,
//! `HelpersLib/Cryptographic/DPAPI.cs`): API keys, passwords and tokens in
//! `UploadersConfig.json` are written as `$DPAPIEncrypted$` + base 64 of
//! `ProtectedData.Protect(UTF-8, CurrentUser)` on Windows. Elsewhere .NET
//! can't encrypt, so ShareX writes them as plain text; so does ShareXrs.
//!
//! A [`Secret`] remembers what it was read from, so a value that didn't
//! change is written back exactly as read (DPAPI output is different every
//! time) and a ShareX profile round-trips byte for byte.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

const ENCRYPTED_TAG: &str = "$DPAPIEncrypted$";

/// A `[JsonEncrypt] string`.
#[derive(Clone, Default)]
pub struct Secret {
    value: Option<String>,
    /// What was read, and the value it gave.
    read: Option<(Value, Option<String>)>,
}

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self { value: Some(value.into()), read: None }
    }

    pub fn null() -> Self {
        Self::default()
    }

    /// The value (`None` for null).
    pub fn get(&self) -> Option<&str> {
        self.value.as_deref()
    }

    /// The value, `""` for null.
    pub fn as_str(&self) -> &str {
        self.value.as_deref().unwrap_or("")
    }

    pub fn set(&mut self, value: Option<String>) {
        self.value = value;
    }

    /// `string.IsNullOrEmpty`.
    pub fn is_empty(&self) -> bool {
        self.value.as_deref().is_none_or(str::is_empty)
    }
}

impl From<&str> for Secret {
    fn from(s: &str) -> Self {
        Secret::new(s)
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.value {
            None => f.write_str("Secret(null)"),
            Some(v) if v.is_empty() => f.write_str("Secret(\"\")"),
            Some(_) => f.write_str("Secret(***)"),
        }
    }
}

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if let Some((raw, value)) = &self.read
            && *value == self.value
        {
            return raw.serialize(s);
        }
        match &self.value {
            None => s.serialize_none(),
            Some(v) if v.is_empty() => s.serialize_str(""),
            Some(v) => match encrypt(v) {
                Some(e) => s.serialize_str(&format!("{ENCRYPTED_TAG}{e}")),
                None => s.serialize_str(v),
            },
        }
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(d)?;
        let value = match &raw {
            Value::Null => None,
            Value::String(text) => match text.strip_prefix(ENCRYPTED_TAG) {
                // A value another user or machine encrypted reads as null.
                Some(encrypted) if !text.is_empty() => decrypt(encrypted),
                _ => Some(text.clone()),
            },
            other => crate::json::to_dotnet_string(other).ok().flatten(),
        };
        Ok(Secret { value: value.clone(), read: Some((raw, value)) })
    }
}

/// `DPAPI.Encrypt`: `None` where DPAPI isn't available.
#[cfg(windows)]
pub fn encrypt(text: &str) -> Option<String> {
    use base64::Engine;
    let data = protect(text.as_bytes(), true)?;
    Some(base64::engine::general_purpose::STANDARD.encode(data))
}

/// `DPAPI.Decrypt`: `None` when it can't be decrypted.
#[cfg(windows)]
pub fn decrypt(encrypted: &str) -> Option<String> {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.decode(encrypted).ok()?;
    let plain = protect(&data, false)?;
    Some(String::from_utf8_lossy(&plain).into_owned())
}

#[cfg(windows)]
fn protect(data: &[u8], encrypt: bool) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData, CryptUnprotectData};

    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: valid input blob; DPAPI allocates the output, freed below.
    let ok = unsafe {
        if encrypt {
            CryptProtectData(&input, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut output)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut output)
        }
    };
    if ok == 0 || output.pbData.is_null() {
        return None;
    }
    // SAFETY: DPAPI returned `cbData` bytes at `pbData`.
    let out = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: allocated by DPAPI with LocalAlloc.
    unsafe { LocalFree(output.pbData.cast()) };
    Some(out)
}

#[cfg(not(windows))]
pub fn encrypt(_text: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn decrypt(_encrypted: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_secrets_write_back_as_read() {
        let s: Secret = serde_json::from_str("\"$DPAPIEncrypted$garbage\"").unwrap();
        assert_eq!(s.get(), None);
        assert_eq!(serde_json::to_string(&s).unwrap(), "\"$DPAPIEncrypted$garbage\"");
        let s: Secret = serde_json::from_str("\"plain\"").unwrap();
        assert_eq!(s.get(), Some("plain"));
        assert_eq!(serde_json::to_string(&s).unwrap(), "\"plain\"");
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_round_trips() {
        let secret = Secret::new("key-123");
        let json = serde_json::to_string(&secret).unwrap();
        assert!(json.starts_with("\"$DPAPIEncrypted$"));
        let back: Secret = serde_json::from_str(&json).unwrap();
        assert_eq!(back.get(), Some("key-123"));
    }
}
