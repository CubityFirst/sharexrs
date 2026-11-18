//! `FileUploaders/Mega.cs`: MEGA's own API and client-side encryption.
//! Uploads use the session `LoginAsync` saved (`MegaSessionID`,
//! `MegaMasterKey`): the file encrypted in AES-128-CTR with a CBC-MAC per
//! chunk (128 KiB growing to 1 MiB), the node created with its encrypted
//! name and key, and a public link `https://mega.nz/file/<handle>#<key>`.

use aes::cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray};
use num_bigint::BigUint;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::uploader_base;
use crate::enums::HttpMethod;
use crate::http::{self, SharedStream};
use crate::json;
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Headers, UploadError, Uploader, WritesBackConfig};

const API_URL: &str = "https://g.api.mega.co.nz/cs";
const PUBLIC_FILE_URL: &str = "https://mega.nz/file/";
const INITIAL_CHUNK_SIZE: u64 = 128 * 1024;
const MAXIMUM_CHUNK_SIZE: u64 = 1024 * 1024;
const UPLOAD_TOKEN_LENGTH: usize = 36;

fn fail<T>(message: impl Into<String>) -> Result<T, String> {
    Err(message.into())
}

/// `ToBase64URL`.
pub fn to_base64_url(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// `FromBase64URL`.
pub fn from_base64_url(value: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    if value.trim().is_empty() {
        return fail("Invalid MEGA Base64 value.");
    }
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value.trim_end_matches('='))
        .map_err(|_| "The input is not a valid Base-64 string as it contains a non-base 64 character, more than two padding characters, or an illegal character among the padding characters.".to_owned())
}

fn aes(key: &[u8]) -> aes::Aes128 {
    aes::Aes128::new_from_slice(key).expect("16-byte key")
}

fn ecb_block(cipher: &aes::Aes128, block: &mut [u8]) {
    cipher.encrypt_block(GenericArray::from_mut_slice(block));
}

/// `TransformEcb`: AES-128-ECB over whole blocks.
fn transform_ecb(data: &[u8], key: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    use aes::cipher::BlockDecrypt;
    if data.is_empty() || !data.len().is_multiple_of(16) {
        return fail("Invalid MEGA AES block data.");
    }
    let c = aes(key);
    let mut out = data.to_vec();
    for block in out.chunks_mut(16) {
        if encrypt {
            c.encrypt_block(GenericArray::from_mut_slice(block));
        } else {
            c.decrypt_block(GenericArray::from_mut_slice(block));
        }
    }
    Ok(out)
}

/// `EncryptAttributes`: `MEGA{"n":"name"}` zero-padded, AES-CBC (zero IV).
fn encrypt_attributes(file_name: &str, file_key: &[u8]) -> Vec<u8> {
    let mut data = format!("MEGA{}", json::to_string(&json::object([("n", Value::String(file_name.to_owned()))]))).into_bytes();
    data.resize(data.len().div_ceil(16) * 16, 0);
    let c = aes(file_key);
    let mut prev = [0u8; 16];
    for block in data.chunks_mut(16) {
        for (b, p) in block.iter_mut().zip(prev) {
            *b ^= p;
        }
        ecb_block(&c, block);
        prev.copy_from_slice(block);
    }
    data
}

/// `GetChunkSizes`.
pub fn chunk_sizes(file_size: u64) -> Vec<u64> {
    if file_size == 0 {
        return vec![0];
    }
    let mut out = Vec::new();
    let (mut remaining, mut size) = (file_size, INITIAL_CHUNK_SIZE);
    while remaining > 0 {
        let current = size.min(remaining);
        out.push(current);
        remaining -= current;
        size = (size + INITIAL_CHUNK_SIZE).min(MAXIMUM_CHUNK_SIZE);
    }
    out
}

/// `MegaFileCipher`: AES-CTR with a CBC-MAC per chunk.
struct FileCipher {
    file_key: [u8; 16],
    iv: [u8; 8],
    file_mac: [u8; 16],
    meta_mac: [u8; 8],
    counter: u64,
}

impl FileCipher {
    fn new() -> Self {
        let mut file_key = [0u8; 16];
        let mut iv = [0u8; 8];
        getrandom::fill(&mut file_key).expect("OS random numbers");
        getrandom::fill(&mut iv).expect("OS random numbers");
        Self { file_key, iv, file_mac: [0; 16], meta_mac: [0; 8], counter: 0 }
    }

    fn encrypt_chunk(&mut self, data: &mut [u8]) {
        let c = aes(&self.file_key);
        let mut chunk_mac = [0u8; 16];
        chunk_mac[..8].copy_from_slice(&self.iv);
        chunk_mac[8..].copy_from_slice(&self.iv);
        for offset in (0..data.len()).step_by(16) {
            let len = 16.min(data.len() - offset);
            let mut plain = [0u8; 16];
            plain[..len].copy_from_slice(&data[offset..offset + len]);
            let mut ks = [0u8; 16];
            ks[..8].copy_from_slice(&self.iv);
            ks[8..].copy_from_slice(&self.counter.to_be_bytes());
            self.counter += 1;
            ecb_block(&c, &mut ks);
            for (m, p) in chunk_mac.iter_mut().zip(plain) {
                *m ^= p;
            }
            ecb_block(&c, &mut chunk_mac);
            for i in 0..len {
                data[offset + i] = plain[i] ^ ks[i];
            }
        }
        for (f, m) in self.file_mac.iter_mut().zip(chunk_mac) {
            *f ^= m;
        }
        ecb_block(&c, &mut self.file_mac);
        for i in 0..4 {
            self.meta_mac[i] = self.file_mac[i] ^ self.file_mac[i + 4];
            self.meta_mac[i + 4] = self.file_mac[i + 8] ^ self.file_mac[i + 12];
        }
    }

    fn full_file_key(&self) -> [u8; 32] {
        let mut k = [0u8; 32];
        for i in 0..8 {
            k[i] = self.file_key[i] ^ self.iv[i];
            k[i + 16] = self.iv[i];
        }
        for i in 8..16 {
            k[i] = self.file_key[i] ^ self.meta_mac[i - 8];
            k[i + 16] = self.meta_mac[i - 8];
        }
        k
    }
}

/// `MegaApiException`'s message.
pub fn api_error_message(code: i64) -> String {
    match code {
        -3 => "MEGA reported a temporary error. Please try again.".into(),
        -4 => "MEGA is rate limiting requests. Please try again later.".into(),
        -9 => "The requested MEGA item was not found.".into(),
        -11 => "MEGA denied access to the requested item.".into(),
        -14 => "MEGA rejected an encryption key.".into(),
        -15 => "The MEGA session is invalid or expired.".into(),
        -16 => "The MEGA account is blocked.".into(),
        -17 => "The MEGA storage quota has been exceeded.".into(),
        -18 => "MEGA is temporarily unavailable.".into(),
        -26 => "MEGA requires a two-factor authentication code.".into(),
        -27 => "The MEGA two-factor authentication code is invalid.".into(),
        -29 => "MEGA requires additional proof-of-work verification for this request.".into(),
        _ => format!("MEGA API error: {code}."),
    }
}

/// `MegaFolderInfo`: a folder of the picker (`MegaSelectedFolder` in
/// `UploadersConfig`). The root is the empty one (`Mega.RootFolder`).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct MegaFolderInfo {
    #[serde(rename = "ID")]
    pub id: Option<String>,
    #[serde(rename = "Name")]
    pub name: Option<String>,
}

impl MegaFolderInfo {
    /// `Mega.RootFolder`.
    pub fn root() -> Self {
        Self { id: Some(String::new()), name: Some(String::new()) }
    }
}

/// `DecryptNodeName`: the node's `n` attribute, its key decrypted with the
/// master key (the first of its `user:key` entries that works).
fn decrypt_node_name(node: &Value, master_key: &[u8]) -> Option<String> {
    let s = |k: &str| node.get(k).and_then(|v| json::to_dotnet_string(v).ok().flatten());
    let attributes = s("a").filter(|a| !a.trim().is_empty())?;
    let key_data = s("k").filter(|k| !k.trim().is_empty())?;
    for entry in key_data.split('/') {
        let encoded = entry.split_once(':').map(|(_, k)| k).unwrap_or(entry);
        let Ok(encrypted_key) = from_base64_url(encoded) else { continue };
        if encrypted_key.len() != 16 {
            continue;
        }
        let Ok(folder_key) = transform_ecb(&encrypted_key, master_key, false) else { continue };
        let Ok(data) = from_base64_url(&attributes) else { continue };
        if data.is_empty() || !data.len().is_multiple_of(16) {
            continue;
        }
        let mut plain = data.clone();
        let cipher = aes(&folder_key);
        let mut previous = [0u8; 16];
        for block in plain.chunks_mut(16) {
            let encrypted: [u8; 16] = block.try_into().expect("16 bytes");
            {
                use aes::cipher::BlockDecrypt;
                cipher.decrypt_block(GenericArray::from_mut_slice(block));
            }
            for (b, p) in block.iter_mut().zip(previous) {
                *b ^= p;
            }
            previous = encrypted;
        }
        let text = String::from_utf8_lossy(&plain).trim_end_matches('\0').to_owned();
        let Some(rest) = text.strip_prefix("MEGA") else { continue };
        let Ok(Value::Object(o)) = json::parse(rest) else { continue };
        return o.get("n").and_then(|n| json::to_dotnet_string(n).ok().flatten());
    }
    None
}

/// A login's result (`MegaSessionInfo`): the session ID and the master key
/// (base64url), to save as `MegaSessionID` and `MegaMasterKey`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MegaSession {
    pub session_id: String,
    pub master_key: String,
}

/// Why a MEGA request failed: an API error code (`MegaApiException`, -26
/// asks for a two-factor code) or another failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MegaError {
    Api(i64),
    Other(String),
    Cancelled,
}

impl std::fmt::Display for MegaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MegaError::Api(c) => f.write_str(&api_error_message(*c)),
            MegaError::Other(m) => f.write_str(m),
            MegaError::Cancelled => f.write_str("The operation was canceled."),
        }
    }
}

impl From<String> for MegaError {
    fn from(m: String) -> Self {
        MegaError::Other(m)
    }
}

impl From<UploadError> for MegaError {
    fn from(e: UploadError) -> Self {
        match e {
            UploadError::Cancelled => MegaError::Cancelled,
            UploadError::Exception(m) => MegaError::Other(m),
        }
    }
}

type MegaResult<T> = Result<T, MegaError>;

/// `Mega`.
pub struct Mega {
    pub base: Uploader,
    pub email: String,
    pub password: String,
    pub folder_id: Option<String>,
    session_id: Option<String>,
    master_key: Option<Vec<u8>>,
    sequence_number: i64,
}

uploader_base!(Mega, FileUploader);
impl WritesBackConfig for Mega {}

fn random_sequence() -> i64 {
    let mut b = [0u8; 4];
    getrandom::fill(&mut b).expect("OS random numbers");
    (u32::from_le_bytes(b) & 0x7fff_ffff) as i64
}

/// `GenerateHashcashToken`: MEGA's proof of work.
pub fn hashcash_token(challenge: &str) -> Result<String, String> {
    let parts: Vec<&str> = challenge.split(':').collect();
    let easiness = match (parts.len() >= 4, parts.first(), parts.get(1).and_then(|e| e.parse::<u8>().ok())) {
        (true, Some(&"1"), Some(e)) => e,
        _ => return fail("MEGA returned an invalid proof-of-work challenge."),
    };
    let token_text = parts[3];
    let token = from_base64_url(token_text).map_err(|_| "MEGA returned an invalid proof-of-work token.".to_owned())?;
    if token.len() != 48 {
        return fail("MEGA returned an invalid proof-of-work token.");
    }
    let mut buffer = vec![0u8; 4];
    for _ in 0..262_144 {
        buffer.extend_from_slice(&token);
    }
    let threshold: u32 = ((((easiness & 63) as u32) << 1) + 1) << (((easiness >> 6) as u32) * 7 + 3);
    let mut nonce: u32 = 0;
    loop {
        buffer[..4].copy_from_slice(&nonce.to_be_bytes());
        let hash = Sha256::digest(&buffer);
        if u32::from_be_bytes(hash[..4].try_into().expect("4 bytes")) <= threshold {
            return Ok(format!("1:{token_text}:{}", to_base64_url(&buffer[..4])));
        }
        nonce = nonce.wrapping_add(1);
    }
}

/// `PasswordToBytes`: the characters' low bytes, padded to 4.
fn password_to_bytes(password: &str) -> Vec<u8> {
    let chars: Vec<u16> = password.encode_utf16().collect();
    let mut b = vec![0u8; (chars.len() + 3) & !3];
    for (i, c) in chars.iter().enumerate() {
        b[i] = *c as u8;
    }
    b
}

/// `CreateAuthenticationInfo`: (password key, user hash).
pub fn authentication_info(email: &str, password: &str, version: i64, salt: Option<&str>) -> Result<(Vec<u8>, String), String> {
    let pw = password_to_bytes(password);
    if let (2, Some(salt)) = (version, salt.filter(|s| !s.trim().is_empty())) {
        let mut derived = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<sha2::Sha512>(&pw, &from_base64_url(salt)?, 100_000, &mut derived);
        return Ok((derived[..16].to_vec(), to_base64_url(&derived[16..])));
    }
    if version == 1 {
        let mut key: [u8; 16] = [0x93, 0xC4, 0x67, 0xE3, 0x7D, 0xB0, 0xC7, 0xA4, 0xD1, 0xBE, 0x3F, 0x81, 0x01, 0x52, 0xCB, 0x56];
        let ciphers: Vec<aes::Aes128> = pw
            .chunks(16)
            .map(|c| {
                let mut k = [0u8; 16];
                k[..c.len()].copy_from_slice(c);
                aes(&k)
            })
            .collect();
        for _ in 0..65536 {
            for c in &ciphers {
                ecb_block(c, &mut key);
            }
        }
        let mut hash = [0u8; 16];
        for (i, b) in email.to_lowercase().bytes().enumerate() {
            hash[i % 16] ^= b;
        }
        let c = aes(&key);
        for _ in 0..16384 {
            ecb_block(&c, &mut hash);
        }
        let mut result = [0u8; 8];
        result[..4].copy_from_slice(&hash[..4]);
        result[4..].copy_from_slice(&hash[8..12]);
        return Ok((key.to_vec(), to_base64_url(&result)));
    }
    fail(format!("MEGA account version {version} is not supported."))
}

/// `ReadMpi`.
fn read_mpi(data: &[u8], offset: &mut usize) -> Result<BigUint, String> {
    if *offset + 2 > data.len() {
        return fail("Invalid MEGA MPI value.");
    }
    let bits = data[*offset] as usize * 256 + data[*offset + 1] as usize;
    let len = bits.div_ceil(8);
    *offset += 2;
    if len == 0 || *offset + len > data.len() {
        return fail("Invalid MEGA MPI value.");
    }
    let v = BigUint::from_bytes_be(&data[*offset..*offset + len]);
    *offset += len;
    Ok(v)
}

/// `DecryptSessionID`: the RSA-encrypted session with the private key the
/// master key unlocks.
fn decrypt_session_id(encrypted_session_id: &str, encrypted_private_key: &[u8], master_key: &[u8]) -> Result<String, String> {
    let mut padded = encrypted_private_key.to_vec();
    padded.resize(padded.len().div_ceil(16) * 16, 0);
    let private = transform_ecb(&padded, master_key, false)?;
    let mut o = 0;
    let p = read_mpi(&private, &mut o)?;
    let q = read_mpi(&private, &mut o)?;
    let d = read_mpi(&private, &mut o)?;
    let session = from_base64_url(encrypted_session_id)?;
    let mut so = 0;
    let c = read_mpi(&session, &mut so)?;
    let bytes = c.modpow(&d, &(p * q)).to_bytes_be();
    if bytes.len() < 43 {
        return fail("MEGA returned an invalid encrypted session.");
    }
    Ok(to_base64_url(&bytes[..43]))
}

impl Mega {
    /// `new Mega(email, password)`: for [`Mega::login`].
    pub fn new(email: &str, password: &str) -> Self {
        Self {
            base: Uploader::new(),
            email: email.trim().to_owned(),
            password: password.to_owned(),
            folder_id: None,
            session_id: None,
            master_key: None,
            sequence_number: random_sequence(),
        }
    }

    /// `new Mega(sessionID, masterKey)`: for uploads.
    pub fn with_session(session_id: &str, master_key: Vec<u8>) -> Self {
        Self { session_id: Some(session_id.to_owned()), master_key: Some(master_key), ..Self::new("", "") }
    }

    /// `SendApiRequestAsync`: one command; the result (or the error code).
    fn api(&mut self, request: Value, include_session: bool) -> MegaResult<Value> {
        self.sequence_number += 1;
        let mut url = format!("{API_URL}?id={}", self.sequence_number);
        if include_session && let Some(sid) = self.session_id.as_deref().filter(|s| !s.trim().is_empty()) {
            url.push_str(&format!("&sid={}", super::cloud_storage::uri_escape_data_string(sid)));
        }
        let command = json::get_str(&request, "a").unwrap_or_else(|| "unknown".into());
        let body = json::to_string(&Value::Array(vec![request]));
        let mut hashcash: Option<String> = None;
        for attempt in 0..2 {
            let mut headers = Headers::new();
            headers.set("Accept", "application/json");
            if let Some(h) = &hashcash {
                headers.set("X-Hashcash", h.clone());
            }
            let data = http::shared_stream(std::io::Cursor::new(body.clone().into_bytes()));
            let Some(info) = self.base.get_response_stream(HttpMethod::POST, &url, Some(&data), Some(http::CONTENT_TYPE_JSON), &[], Some(&headers), true)? else {
                return Err(MegaError::Other("MEGA API request failed.".into()));
            };
            if info.status_code == 402 {
                let challenge = info.header("X-Hashcash").map(str::to_owned).filter(|c| !c.trim().is_empty());
                match (attempt, challenge) {
                    (0, Some(c)) => {
                        hashcash = Some(hashcash_token(&c)?);
                        continue;
                    }
                    _ => return Err(MegaError::Other("MEGA proof-of-work verification failed.".into())),
                }
            }
            if !info.is_success() {
                return Err(MegaError::Other(format!("MEGA API request failed with HTTP status {}.", info.status_code)));
            }
            let text = info.response_text.clone().unwrap_or_default();
            if text.trim().is_empty() {
                return Err(MegaError::Other("MEGA returned an empty API response.".into()));
            }
            let parsed = json::parse(&text).map_err(|_| {
                let kind = if text.trim_start().starts_with('<') { "HTML".to_owned() } else { format!("a {}-character payload", text.encode_utf16().count()) };
                MegaError::Other(format!("MEGA returned {kind} instead of JSON for API command '{command}'."))
            })?;
            let mut token = match parsed {
                Value::Array(a) => a.into_iter().next().ok_or_else(|| MegaError::Other("MEGA returned an empty API response.".into()))?,
                other => other,
            };
            if let Some(result) = token.as_object().and_then(|o| o.get("result")).cloned() {
                token = result;
            }
            if let Some(code) = token.as_i64().filter(|c| *c < 0) {
                return Err(MegaError::Api(code));
            }
            return Ok(token);
        }
        Err(MegaError::Other("MEGA proof-of-work verification failed.".into()))
    }

    /// `LoginAsync(twoFactorCode)`: the session to save. `MegaError::Api(-26)`
    /// means the account wants a two-factor code: ask and log in again.
    pub fn login(&mut self, two_factor_code: Option<&str>) -> MegaResult<MegaSession> {
        self.session_id = None;
        self.master_key = None;
        let pre = self.api(json::object([("a", "us0".into()), ("user", Value::String(self.email.clone()))]), false)?;
        let version = pre.get("v").and_then(Value::as_i64).unwrap_or(0);
        let salt = pre.get("s").and_then(Value::as_str).map(str::to_owned);
        let (password_key, user_hash) = authentication_info(&self.email, &self.password, version, salt.as_deref())?;
        let mut request = json::object([("a", "us".into()), ("user", Value::String(self.email.clone())), ("uh", Value::String(user_hash))]);
        if let Some(code) = two_factor_code.filter(|c| !c.trim().is_empty()) {
            request.as_object_mut().expect("object").insert("mfa".into(), Value::String(code.trim().to_owned()));
        }
        let login = self.api(request, false)?;
        let s = |k: &str| login.get(k).and_then(Value::as_str).map(str::to_owned);
        let master_key = transform_ecb(&from_base64_url(&s("k").unwrap_or_default())?, &password_key, false)?;
        let session_id = match s("csid").filter(|c| !c.trim().is_empty()) {
            Some(csid) => decrypt_session_id(&csid, &from_base64_url(&s("privk").unwrap_or_default())?, &master_key)?,
            None => {
                let tsid = s("tsid").unwrap_or_default();
                let valid = from_base64_url(&tsid).ok().filter(|b| b.len() == 32).is_some_and(|b| {
                    transform_ecb(&b[16..], &master_key, false).is_ok_and(|challenge| challenge[..] == b[..16])
                });
                if !valid {
                    return Err(MegaError::Other("MEGA returned an invalid login session.".into()));
                }
                tsid
            }
        };
        self.session_id = Some(session_id.clone());
        self.master_key = Some(master_key.clone());
        Ok(MegaSession { session_id, master_key: to_base64_url(&master_key) })
    }

    fn root_node_id(&mut self) -> MegaResult<String> {
        let response = self.api(json::object([("a", "f".into()), ("c", 1.into())]), true)?;
        response
            .get("f")
            .and_then(Value::as_array)
            .and_then(|nodes| nodes.iter().find(|n| n.get("t").and_then(Value::as_i64) == Some(2)))
            .and_then(|n| n.get("h"))
            .and_then(Value::as_str)
            .filter(|h| !h.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| MegaError::Other("MEGA did not return the Cloud Drive root folder.".into()))
    }

    /// `GetFoldersAsync(parent)`: the folders right under `parent` (the
    /// Cloud Drive root when its ID is empty), names decrypted, sorted by
    /// name. The folder picker of Destination settings shows these.
    pub fn get_folders(&mut self, parent: Option<&MegaFolderInfo>) -> MegaResult<Vec<MegaFolderInfo>> {
        if self.session_id.as_deref().is_none_or(|s| s.trim().is_empty()) || self.master_key.as_ref().is_none_or(|k| k.len() != 16) {
            return Err(MegaError::Other("MEGA login is required. Log in from Destination settings.".into()));
        }
        let response = self.api(json::object([("a", "f".into()), ("c", 1.into())]), true)?;
        let nodes: Vec<Value> = response.get("f").and_then(Value::as_array).cloned().unwrap_or_default();
        let node_str = |n: &Value, k: &str| n.get(k).and_then(|v| json::to_dotnet_string(v).ok().flatten());
        let node_type = |n: &Value| n.get("t").and_then(|t| t.as_i64().or_else(|| t.as_str().and_then(|s| s.trim().parse().ok())));
        let mut parent_id = parent.and_then(|p| p.id.clone()).filter(|id| !id.trim().is_empty());
        if parent_id.is_none() {
            parent_id = nodes.iter().find(|n| node_type(n) == Some(2)).and_then(|n| node_str(n, "h"));
        }
        let Some(parent_id) = parent_id.filter(|id| !id.trim().is_empty()) else {
            return Err(MegaError::Other("MEGA did not return the Cloud Drive root folder.".into()));
        };
        let master_key = self.master_key.clone().unwrap_or_default();
        let mut folders: Vec<MegaFolderInfo> = nodes
            .iter()
            .filter(|n| node_type(n) == Some(1) && node_str(n, "p").as_deref() == Some(parent_id.as_str()))
            .map(|n| MegaFolderInfo { id: node_str(n, "h"), name: decrypt_node_name(n, &master_key) })
            .filter(|f| f.id.as_deref().is_some_and(|id| !id.trim().is_empty()))
            .collect();
        folders.sort_by(|a, b| match (&a.name, &b.name) {
            (Some(x), Some(y)) => sharex_helpers::culture::current_culture_ignore_case_cmp(x, y),
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        Ok(folders)
    }

    fn upload_url(&mut self, file_size: u64) -> MegaResult<String> {
        let response = self.api(json::object([("a", "u".into()), ("ssl", 2.into()), ("v", 3.into()), ("s", Value::from(file_size))]), true)?;
        let url = response.get("p").and_then(Value::as_str).unwrap_or("");
        if url.trim().is_empty() {
            return Err(MegaError::Other("MEGA did not return an upload URL.".into()));
        }
        Ok(url.trim_end_matches('/').to_owned())
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let mut result = UploadResult::default();
        let report = self.base.allow_report_progress;
        self.base.allow_report_progress = false;
        let r = self.upload_inner(data, file_name, report, &mut result);
        self.base.allow_report_progress = report;
        match r {
            Ok(()) => {}
            Err(MegaError::Cancelled) => return Err(UploadError::Cancelled),
            Err(e) => {
                if self.base.errors.is_empty() {
                    self.base.errors.add(e.to_string());
                }
            }
        }
        Ok(Some(result))
    }

    fn upload_inner(&mut self, data: &SharedStream, file_name: &str, report: bool, result: &mut UploadResult) -> MegaResult<()> {
        let file_size = http::stream_length(data).map_err(|e| e.to_string())?;
        let master_key = match (&self.session_id, &self.master_key) {
            (Some(s), Some(k)) if !s.trim().is_empty() && k.len() == 16 => k.clone(),
            _ => return Err(MegaError::Other("MEGA login is required. Log in from Destination settings.".into())),
        };
        let target = match self.folder_id.clone().filter(|f| !f.trim().is_empty()) {
            Some(f) => f,
            None => self.root_node_id()?,
        };
        let upload_url = self.upload_url(file_size)?;
        let mut completion: Option<String> = None;
        let mut position = 0u64;
        let mut progress = http::ProgressManager::new(file_size, 0);
        let callback = self.base.progress_callback().filter(|_| report);
        let mut cipher = FileCipher::new();
        for size in chunk_sizes(file_size) {
            if self.base.stop_requested() {
                return Err(MegaError::Cancelled);
            }
            let mut chunk = vec![0u8; size as usize];
            if size > 0 {
                use std::io::{Read, Seek, SeekFrom};
                let mut s = data.lock().expect("upload stream");
                s.seek(SeekFrom::Start(position)).map_err(|e| e.to_string())?;
                s.read_exact(&mut chunk).map_err(|e| e.to_string())?;
                drop(s);
                cipher.encrypt_chunk(&mut chunk);
            }
            let body = http::shared_stream(std::io::Cursor::new(chunk));
            let response = self
                .base
                .get_response_stream(HttpMethod::POST, &format!("{upload_url}/{position}"), Some(&body), Some(http::CONTENT_TYPE_OCTET_STREAM), &[], None, false)?
                .ok_or_else(|| MegaError::Other("MEGA rejected the upload request.".into()))?;
            if let Some(code) = response.response_text.as_deref().and_then(|t| t.trim().parse::<i64>().ok()).filter(|c| *c < 0) {
                return Err(MegaError::Api(code));
            }
            let token = response.response_data.as_deref().unwrap_or(&[]);
            match token.len() {
                UPLOAD_TOKEN_LENGTH => completion = Some(to_base64_url(token)),
                0 => {}
                n => return Err(MegaError::Other(format!("MEGA returned an invalid upload token ({n} bytes)."))),
            }
            position += size;
            if file_size > 0
                && progress.update_progress(size)
                && let Some(cb) = &callback
            {
                cb(&progress);
            }
        }
        let completion = completion.filter(|c| !c.trim().is_empty()).ok_or_else(|| MegaError::Other("MEGA did not return an upload completion handle.".into()))?;
        let full_key = cipher.full_file_key();
        let attributes = encrypt_attributes(file_name, &cipher.file_key);
        let encrypted_key = transform_ecb(&full_key, &master_key, true)?;
        let node = json::object([
            ("h", Value::String(completion)),
            ("t", 0.into()),
            ("a", Value::String(to_base64_url(&attributes))),
            ("k", Value::String(to_base64_url(&encrypted_key))),
        ]);
        let created = self.api(json::object([("a", "p".into()), ("t", Value::String(target)), ("n", Value::Array(vec![node]))]), true)?;
        let node_id = json::select_token(&created, "f[0].h")
            .ok()
            .flatten()
            .and_then(|v| json::to_dotnet_string(v).ok().flatten())
            .filter(|h| !h.trim().is_empty())
            .ok_or_else(|| MegaError::Other("MEGA created the file but did not return its node handle.".into()))?;
        let public = self.api(json::object([("a", "l".into()), ("n", Value::String(node_id))]), true)?;
        let handle = json::to_dotnet_string(&public).ok().flatten().filter(|h| !h.trim().is_empty());
        let Some(handle) = handle else { return Err(MegaError::Other("MEGA did not return a public file handle.".into())) };
        result.response = Some(json::to_string(&created));
        result.url = Some(format!("{PUBLIC_FILE_URL}{handle}#{}", to_base64_url(&full_key)));
        result.success = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mega_helpers() {
        assert_eq!(chunk_sizes(0), vec![0]);
        assert_eq!(chunk_sizes(300 * 1024), vec![128 * 1024, 172 * 1024]);
        assert_eq!(from_base64_url(&to_base64_url(&[1, 2, 255])).unwrap(), vec![1, 2, 255]);
        assert_eq!(password_to_bytes("ab"), vec![b'a', b'b', 0, 0]);
        let (key, hash) = authentication_info("A@b.c", "pw", 1, None).unwrap();
        assert_eq!(key.len(), 16);
        assert_eq!(from_base64_url(&hash).unwrap().len(), 8);
        let token = hashcash_token(&format!("1:255:x:{}", to_base64_url(&[7u8; 48]))).unwrap();
        assert!(token.starts_with("1:"), "{token}");
    }
}
