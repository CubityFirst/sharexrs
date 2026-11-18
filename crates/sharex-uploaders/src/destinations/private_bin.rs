//! `TextUploaders/PrivateBin.cs` and `Helpers/Base58Converter.cs`: the
//! paste encrypted on this side (PBKDF2-SHA256 key, AES-256-CBC), the key
//! after `#` in the link.

use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use num_bigint::BigUint;
use serde::Deserialize;
use serde_json::Value;
use sharex_settings::dotnet_enum;
use sharex_strings::uploaders_lib as strings;

use super::{NULL_REFERENCE, deserialize_object, uploader_base};
use crate::args;
use crate::dpapi::Secret;
use crate::enums::HttpMethod;
use crate::http;
use crate::json::{self, get_i64, get_str};
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Headers, UploadError, Uploader, WritesBackConfig};
use crate::url_helpers;

dotnet_enum! {
    /// `PrivateBinExpiration`.
    pub enum PrivateBinExpiration { M5, M10, H1, D1, W1, M1, Y1, N }
}

dotnet_enum! {
    /// `PrivateBinFormat`.
    pub enum PrivateBinFormat { PlainText, SyntaxHighlighting, Markdown }
}

impl PrivateBinExpiration {
    /// `[Description]`.
    pub fn description(&self) -> &'static str {
        match self {
            Self::M5 => "5min",
            Self::M10 => "10min",
            Self::H1 => "1hour",
            Self::D1 => "1day",
            Self::W1 => "1week",
            Self::M1 => "1month",
            Self::Y1 => "1year",
            Self::N => "never",
            _ => "1week",
        }
    }
}

impl PrivateBinFormat {
    /// `[Description]`.
    pub fn description(&self) -> &'static str {
        match self {
            Self::SyntaxHighlighting => "syntaxhighlighting",
            Self::Markdown => "markdown",
            _ => "plaintext",
        }
    }
}

fn default_expiration() -> PrivateBinExpiration {
    PrivateBinExpiration::W1
}

fn default_format() -> PrivateBinFormat {
    PrivateBinFormat::PlainText
}

fn default_custom_url() -> Option<String> {
    Some("https://privatebin.net/".into())
}

/// `PrivateBinSettings` (`UploadersConfig.PrivateBinSettings`).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct PrivateBinSettings {
    #[serde(default)]
    pub Username: Option<String>,
    #[serde(default)]
    pub Password: Secret,
    #[serde(default)]
    pub BurnAfterReading: bool,
    #[serde(default = "default_expiration")]
    pub Expiration: PrivateBinExpiration,
    #[serde(default = "default_format")]
    pub Format: PrivateBinFormat,
    #[serde(default = "default_custom_url")]
    pub CustomUrl: Option<String>,
    #[serde(default)]
    pub PastePassword: Secret,
}

impl PrivateBinSettings {
    pub fn from_value(v: &Value) -> Self {
        serde_json::from_value(v.clone()).unwrap_or_else(|_| serde_json::from_value(Value::Object(Default::default())).expect("defaults"))
    }
}

const BASE58: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// `Base58Converter.Encode`.
pub fn base58_encode(data: &[u8]) -> String {
    let mut n = BigUint::from_bytes_be(data);
    let zero = BigUint::from(0u8);
    let b58 = BigUint::from(58u8);
    let mut out = Vec::new();
    while n > zero {
        let rem = (&n % &b58).to_u32_digits().first().copied().unwrap_or(0);
        n /= &b58;
        out.push(BASE58[rem as usize]);
    }
    out.extend(std::iter::repeat_n(b'1', data.iter().take_while(|b| **b == 0).count()));
    out.reverse();
    String::from_utf8(out).expect("ASCII")
}

/// `Base58Converter.Decode` (`None` for a character outside the alphabet).
pub fn base58_decode(data: &str) -> Option<Vec<u8>> {
    let mut n = BigUint::from(0u8);
    for c in data.bytes() {
        let digit = BASE58.iter().position(|b| *b == c)?;
        n = n * 58u32 + digit as u32;
    }
    let zeros = data.bytes().take_while(|c| *c == b'1').count();
    let mut out = vec![0u8; zeros];
    if n > BigUint::from(0u8) {
        out.extend(n.to_bytes_be());
    }
    Some(out)
}

const CIPHER_ITERATION_COUNT: u32 = 100_000;
const CIPHER_BLOCK_BITS: usize = 256;
const CIPHER_TAG_BITS: usize = 128;

fn random_bytes(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).expect("OS random numbers");
    b
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// `PrivateBin`.
pub struct PrivateBin {
    pub base: Uploader,
    pub settings: PrivateBinSettings,
}

uploader_base!(PrivateBin, TextUploader);
impl WritesBackConfig for PrivateBin {}

/// What `PrivateBinApiHelper` makes for one paste.
pub struct PrivateBinCipher {
    pub adata: Value,
    pub ct: String,
    pub public_key: String,
}

/// `PrivateBinApiHelper` + `EncryptMessage`: random key, IV and salt.
pub fn encrypt_paste(settings: &PrivateBinSettings, message: &str) -> PrivateBinCipher {
    encrypt_paste_with(settings, message, &random_bytes(CIPHER_BLOCK_BITS / 8), &random_bytes(CIPHER_TAG_BITS / 8), &random_bytes(8))
}

/// [`encrypt_paste`] with given random values.
pub fn encrypt_paste_with(settings: &PrivateBinSettings, message: &str, key: &[u8], iv: &[u8], salt: &[u8]) -> PrivateBinCipher {
    let public_key = base58_encode(key);
    let adata = Value::Array(vec![
        Value::Array(vec![
            Value::String(b64(iv)),
            Value::String(b64(salt)),
            Value::from(CIPHER_ITERATION_COUNT),
            Value::from(CIPHER_BLOCK_BITS),
            Value::from(CIPHER_TAG_BITS),
            Value::String("aes".into()),
            Value::String("cbc".into()),
            Value::String("none".into()),
        ]),
        Value::String(settings.Format.description().into()),
        Value::from(0),
        Value::from(if settings.BurnAfterReading { 1 } else { 0 }),
    ]);
    let mut password = key.to_vec();
    if let Some(p) = settings.PastePassword.get().filter(|p| !p.is_empty()) {
        password.extend_from_slice(p.as_bytes());
    }
    let plain = json::to_string(&json::object([("paste", Value::String(message.to_owned()))]));
    let mut derived = [0u8; CIPHER_BLOCK_BITS / 8];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(&password, salt, CIPHER_ITERATION_COUNT, &mut derived);
    let encryptor = cbc::Encryptor::<aes::Aes256>::new_from_slices(&derived, iv).expect("key and IV sizes");
    let ct = encryptor.encrypt_padded_vec_mut::<Pkcs7>(plain.as_bytes());
    PrivateBinCipher { adata, ct: b64(&ct), public_key }
}

impl PrivateBin {
    pub fn new(settings: PrivateBinSettings) -> Self {
        Self { base: Uploader::new(), settings }
    }

    fn upload_text(&mut self, text: &str, _file_name: &str) -> CoreResult {
        let mut result = UploadResult::default();
        if text.is_empty() {
            return Ok(Some(result));
        }
        let cipher = encrypt_paste(&self.settings, text);
        let payload = json::object([
            ("adata", cipher.adata),
            ("meta", json::object([("expire", Value::String(self.settings.Expiration.description().into()))])),
            ("v", Value::from(2)),
            ("ct", Value::String(cipher.ct)),
        ]);
        let mut headers = Headers::new().with("X-Requested-With", "JSONHttpRequest");
        if let (Some(u), Some(p)) = (self.settings.Username.as_deref().filter(|u| !u.is_empty()), self.settings.Password.get().filter(|p| !p.is_empty())) {
            headers.add("Authorization", format!("Basic {}", b64(format!("{u}:{p}").as_bytes())));
        }
        let custom_url = self.settings.CustomUrl.clone().unwrap_or_default();
        self.base.send_request_text(HttpMethod::POST, &custom_url, &json::to_string(&payload), Some(http::CONTENT_TYPE_JSON), &[], Some(&headers))?;
        let last = self.base.last_response_info.clone().ok_or(UploadError::Exception(NULL_REFERENCE.into()))?;
        if last.is_success() {
            let response = deserialize_object(last.response_text.as_deref())?.ok_or(UploadError::Exception(NULL_REFERENCE.into()))?;
            if get_i64(&response, "Status") == 0 {
                result.url = Some(format!("{}#{}", url_helpers::combine_url(&custom_url, &get_str(&response, "Url").unwrap_or_default()), cipher.public_key));
                let query_args =
                    args![("pasteid", get_str(&response, "Id").unwrap_or_default()), ("deletetoken", get_str(&response, "DeleteToken").unwrap_or_default())];
                let query = url_helpers::create_query_string(&query_args, false);
                result.deletion_url = Some(build_uri_root(&custom_url, &query));
            } else {
                self.base.errors.add(get_str(&response, "Message").unwrap_or_default());
            }
        } else if last.status_code == 401 {
            self.base.errors.add(strings::PrivateBin_Server_requires_authorization());
        }
        Ok(Some(result))
    }
}

/// `URLHelpers.BuildUri(root, null, query)`: the root's scheme, host and
/// port, path `/`, the query.
fn build_uri_root(root: &str, query: &str) -> String {
    let (scheme, rest) = root.split_once("://").unwrap_or(("http", root));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or("").to_ascii_lowercase();
    let scheme = scheme.to_ascii_lowercase();
    let authority = match (scheme.as_str(), authority.rsplit_once(':')) {
        ("http", Some((h, "80"))) | ("https", Some((h, "443"))) => h.to_owned(),
        _ => authority,
    };
    if query.is_empty() { format!("{scheme}://{authority}/") } else { format!("{scheme}://{authority}/?{query}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base58_round_trips() {
        assert_eq!(base58_encode(&[0, 0, 1, 2, 255]), "11LiA");
        assert_eq!(base58_decode("11LiA").unwrap(), vec![0, 0, 1, 2, 255]);
        assert_eq!(base58_encode(b"hello world"), "StV1DL6CwTryKyV");
    }

    #[test]
    fn encrypts_like_privatebin() {
        use aes::cipher::{BlockDecryptMut, KeyIvInit};
        let settings = PrivateBinSettings::from_value(&serde_json::json!({"PastePassword": "pw", "Format": "Markdown"}));
        let c = encrypt_paste_with(&settings, "hi", &[7u8; 32], &[1u8; 16], &[2u8; 8]);
        assert_eq!(c.adata[1], "markdown");
        let mut password = vec![7u8; 32];
        password.extend_from_slice(b"pw");
        let mut key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(&password, &[2u8; 8], 100_000, &mut key);
        use base64::Engine;
        let ct = base64::engine::general_purpose::STANDARD.decode(&c.ct).unwrap();
        let plain = cbc::Decryptor::<aes::Aes256>::new_from_slices(&key, &[1u8; 16]).unwrap().decrypt_padded_vec_mut::<Pkcs7>(&ct).unwrap();
        assert_eq!(plain, br#"{"paste":"hi"}"#);
        assert_eq!(build_uri_root("https://Bin.example.com:443/pb/", "a=1"), "https://bin.example.com/?a=1");
    }
}
