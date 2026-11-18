//! `new PrivateKeyFile(path[, passphrase])` as SSH.NET 2026 reads a key
//! file: OpenSSH keys, PEM (`RSA PRIVATE KEY`, `EC PRIVATE KEY`, encrypted
//! with `DEK-Info` AES-CBC or 3DES), PKCS#8 (plain and encrypted), PuTTY
//! (`.ppk` versions 2 and 3, Argon2) and SSH.COM (`SSH2 ENCRYPTED PRIVATE
//! KEY`, RSA); RSA, ECDSA (P-256, P-384, P-521) and Ed25519 keys, with
//! SSH.NET's error texts. Its quirks come along: an Ed25519 PKCS#8 key is
//! read from its wrapped bytes (so it signs as another key).

use std::sync::OnceLock;

use base64::Engine;
use fancy_regex::Regex;

use crate::ssh::{Reader, UserKey};

const PRIVATE_KEY_PATTERN: &str = r#"(?m)^-+ *BEGIN (?<keyName>\w+( \w+)*) *-+\r?\n((Proc-Type: 4,ENCRYPTED\r?\nDEK-Info: (?<cipherName>[A-Z0-9-]+),(?<salt>[a-fA-F0-9]+)\r?\n\r?\n)|(Comment: "?[^\r\n]*"?\r?\n))?(?<data>([a-zA-Z0-9/+=]{1,80}\r?\n)+)(\r?\n)?-+ *END \k<keyName> *-+"#;
const PUTTY_PATTERN: &str = r"(?m)^(?<keyName>PuTTY-User-Key-File)-(?<version>\d+): (?<algorithmName>[\w-]+)\r?\nEncryption: (?<encryptionType>[\w-]+)\r?\nComment: (?<comment>.*?)\r?\nPublic-Lines: \d+\r?\n(?<publicKey>(([a-zA-Z0-9/+=]{1,64})\r?\n)+)(Key-Derivation: (?<argon2Type>\w+)\r?\nArgon2-Memory: (?<argon2Memory>\d+)\r?\nArgon2-Passes: (?<argon2Passes>\d+)\r?\nArgon2-Parallelism: (?<argon2Parallelism>\d+)\r?\nArgon2-Salt: (?<argon2Salt>[a-fA-F0-9]+)\r?\n)?Private-Lines: \d+\r?\n(?<data>(([a-zA-Z0-9/+=]{1,64})\r?\n)+)+Private-MAC: (?<mac>[a-fA-F0-9]+)";

const ENCRYPTED_NO_PASSPHRASE: &str = "Private key is encrypted but passphrase is empty.";
const BAD_PADDING: &str = "Padding is invalid and cannot be removed.";

fn regex(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("key file pattern"))
}

/// `Convert.FromBase64String` (white space ignored).
fn from_base64(s: &str) -> Result<Vec<u8>, String> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(clean)
        .map_err(|_| "The input is not a valid Base-64 string as it contains a non-base 64 character, more than two padding characters, or an illegal character among the padding characters.".to_owned())
}

fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    hex::decode(s).map_err(|_| "The input is not a valid hex string as it contains a non-hex character.".to_owned())
}

/// Reads a key file's text.
pub fn load(text: &str, passphrase: Option<&str>) -> Result<UserKey, String> {
    static PEM: OnceLock<Regex> = OnceLock::new();
    static PUTTY: OnceLock<Regex> = OnceLock::new();
    let putty = text.starts_with("PuTTY-User-Key-File");
    let re = if putty { regex(&PUTTY, PUTTY_PATTERN) } else { regex(&PEM, PRIVATE_KEY_PATTERN) };
    let caps = re.captures(text).ok().flatten().ok_or_else(|| "Invalid private key file.".to_owned())?;
    let group = |name: &str| caps.name(name).map(|m| m.as_str().to_owned()).unwrap_or_default();
    let key_name = group("keyName");
    let data = from_base64(&group("data"))?;
    let passphrase = passphrase.filter(|p| !p.is_empty());
    match key_name.as_str() {
        "RSA PRIVATE KEY" | "EC PRIVATE KEY" => pkcs1(&group("cipherName"), &group("salt"), &key_name, &data, passphrase),
        "PRIVATE KEY" => pkcs8(&data),
        "ENCRYPTED PRIVATE KEY" => {
            let info = pkcs8::EncryptedPrivateKeyInfo::try_from(data.as_slice()).map_err(|e| e.to_string())?;
            // BouncyCastle's text for a wrong passphrase.
            let plain = info.decrypt(passphrase.unwrap_or("")).map_err(|_| "pad block corrupted".to_owned())?;
            pkcs8(plain.as_bytes())
        }
        "OPENSSH PRIVATE KEY" => openssh(&data, passphrase),
        "SSH2 ENCRYPTED PRIVATE KEY" => sshcom(&data, passphrase),
        "PuTTY-User-Key-File" => putty_key(&caps, passphrase),
        other => Err(format!("Key '{other}' is not supported.")),
    }
}

// ---------------------------------------------------------------------------
// Keys from their parts.

fn rsa_key(key: rsa::RsaPrivateKey) -> Result<UserKey, String> {
    let keypair = ssh_key::private::RsaKeypair::try_from(key).map_err(|e| e.to_string())?;
    ssh_key::PrivateKey::new(ssh_key::private::KeypairData::Rsa(keypair), "").map_err(|e| e.to_string())
}

fn rsa_from_parts(n: &[u8], e: &[u8], d: &[u8], p: &[u8], q: &[u8]) -> Result<UserKey, String> {
    let b = rsa::BigUint::from_bytes_be;
    let mut k = rsa::RsaPrivateKey::from_components(b(n), b(e), b(d), vec![b(p), b(q)]).map_err(|e| e.to_string())?;
    k.precompute().map_err(|e| e.to_string())?;
    rsa_key(k)
}

/// `new ED25519Key(privateKeyData)`: the first 32 bytes are the seed.
fn ed25519_key(data: &[u8]) -> Result<UserKey, String> {
    let seed: [u8; 32] = data.get(..32).and_then(|s| s.try_into().ok()).ok_or_else(|| "Offset and length were out of bounds for the array or count is greater than the number of elements from index to the end of the source collection.".to_owned())?;
    let keypair = ssh_key::private::Ed25519Keypair::from_seed(&seed);
    ssh_key::PrivateKey::new(ssh_key::private::KeypairData::Ed25519(keypair), "").map_err(|e| e.to_string())
}

/// `new EcdsaKey(curve, publickey, privatekey)`: the curve by name
/// (`nistp256`) or OID.
fn ecdsa_key(curve: &str, public: &[u8], private: &[u8]) -> Result<UserKey, String> {
    use ssh_key::private::EcdsaKeypair;
    let private: Vec<u8> = private.iter().skip_while(|b| **b == 0).copied().collect();
    let keypair = match curve.to_ascii_lowercase().as_str() {
        "nistp256" | "1.2.840.10045.3.1.7" => EcdsaKeypair::NistP256 {
            public: p256::EncodedPoint::from_bytes(public).map_err(|e| e.to_string())?,
            private: p256::SecretKey::from_slice(&private).map_err(|e| e.to_string())?.into(),
        },
        "nistp384" | "1.3.132.0.34" => EcdsaKeypair::NistP384 {
            public: p384::EncodedPoint::from_bytes(public).map_err(|e| e.to_string())?,
            private: p384::SecretKey::from_slice(&private).map_err(|e| e.to_string())?.into(),
        },
        "nistp521" | "1.3.132.0.35" => EcdsaKeypair::NistP521 {
            public: p521::EncodedPoint::from_bytes(public).map_err(|e| e.to_string())?,
            private: p521::SecretKey::from_slice(&private).map_err(|e| e.to_string())?.into(),
        },
        other => return Err(format!("Unsupported curve oid: {other}")),
    };
    ssh_key::PrivateKey::new(ssh_key::private::KeypairData::Ecdsa(keypair), "").map_err(|e| e.to_string())
}

/// `new EcdsaKey(der)`: an `ECPrivateKey` (SEC1) with its curve and public
/// point.
fn ecdsa_from_sec1(der: &[u8], curve: Option<String>) -> Result<UserKey, String> {
    let k = sec1::EcPrivateKey::try_from(der).map_err(|e| e.to_string())?;
    let curve = match curve {
        Some(c) => c,
        None => k.parameters.and_then(|p| p.named_curve()).map(|o| o.to_string()).ok_or_else(|| "The ASN.1 value is invalid.".to_owned())?,
    };
    let public = k.public_key.ok_or_else(|| "The ASN.1 value is invalid.".to_owned())?;
    ecdsa_key(&curve, public, k.private_key)
}

// ---------------------------------------------------------------------------
// The formats.

/// `PKCS1`: a PEM key, decrypted with the `DEK-Info` cipher and a key made
/// from the passphrase and salt (OpenSSL's `EVP_BytesToKey` with MD5).
fn pkcs1(cipher: &str, salt: &str, key_name: &str, data: &[u8], passphrase: Option<&str>) -> Result<UserKey, String> {
    let plain = if !cipher.is_empty() && !salt.is_empty() {
        let pass = passphrase.ok_or_else(|| ENCRYPTED_NO_PASSPHRASE.to_owned())?;
        let salt = from_hex(salt)?;
        let key_bits = match cipher {
            "DES-EDE3-CBC" => 192,
            "AES-128-CBC" => 128,
            "AES-192-CBC" => 192,
            "AES-256-CBC" => 256,
            _ => return Err(format!("Private key cipher \"{cipher}\" is not supported.")),
        };
        let mut material = Vec::new();
        let base: Vec<u8> = pass.bytes().chain(salt.iter().take(8).copied()).collect();
        let mut h = <md5::Md5 as md5::Digest>::digest(&base).to_vec();
        material.extend_from_slice(&h);
        while material.len() < key_bits / 8 {
            let mut next = h.clone();
            next.extend_from_slice(&base);
            h = <md5::Md5 as md5::Digest>::digest(&next).to_vec();
            material.extend_from_slice(&h);
        }
        cbc_decrypt_pkcs7(cipher, &material[..key_bits / 8], &salt, data)?
    } else {
        data.to_vec()
    };
    if key_name == "EC PRIVATE KEY" {
        return ecdsa_from_sec1(&plain, None);
    }
    use rsa::pkcs1::DecodeRsaPrivateKey;
    rsa_key(rsa::RsaPrivateKey::from_pkcs1_der(&plain).map_err(|e| e.to_string())?)
}

/// CBC decryption with PKCS#7 padding (.NET's text for bad padding).
fn cbc_decrypt_pkcs7(cipher: &str, key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding};
    let mut buf = data.to_vec();
    let block = if cipher == "DES-EDE3-CBC" { 8 } else { 16 };
    if buf.is_empty() || !buf.len().is_multiple_of(block) {
        return Err("The input data is not a complete block.".into());
    }
    let r = match cipher {
        "DES-EDE3-CBC" => cbc::Decryptor::<des::TdesEde3>::new_from_slices(key, &iv[..8]).map(|c| c.decrypt_padded_mut::<NoPadding>(&mut buf).map(|_| ())),
        "AES-192-CBC" => cbc::Decryptor::<aes::Aes192>::new_from_slices(key, &iv[..16]).map(|c| c.decrypt_padded_mut::<NoPadding>(&mut buf).map(|_| ())),
        "AES-256-CBC" => cbc::Decryptor::<aes::Aes256>::new_from_slices(key, &iv[..16]).map(|c| c.decrypt_padded_mut::<NoPadding>(&mut buf).map(|_| ())),
        _ => cbc::Decryptor::<aes::Aes128>::new_from_slices(key, &iv[..16]).map(|c| c.decrypt_padded_mut::<NoPadding>(&mut buf).map(|_| ())),
    };
    r.map_err(|e| e.to_string())?.map_err(|_| BAD_PADDING.to_owned())?;
    let n = *buf.last().expect("not empty") as usize;
    if n == 0 || n > block || !buf[buf.len() - n..].iter().all(|b| *b as usize == n) {
        return Err(BAD_PADDING.into());
    }
    buf.truncate(buf.len() - n);
    Ok(buf)
}

/// `PKCS8`: RSA, EC (version 1, with its public point) and Ed25519.
fn pkcs8(der: &[u8]) -> Result<UserKey, String> {
    let info = pkcs8::PrivateKeyInfo::try_from(der).map_err(|e| e.to_string())?;
    let oid = info.algorithm.oid.to_string();
    match oid.as_str() {
        "1.2.840.113549.1.1.1" => {
            use rsa::pkcs1::DecodeRsaPrivateKey;
            rsa_key(rsa::RsaPrivateKey::from_pkcs1_der(info.private_key).map_err(|e| e.to_string())?)
        }
        "1.2.840.10045.2.1" => {
            let curve = info.algorithm.parameters_oid().map_err(|e| e.to_string())?.to_string();
            ecdsa_from_sec1(info.private_key, Some(curve))
        }
        // `new ED25519Key(octets)`: the octets still hold their OCTET STRING
        // header, as SSH.NET reads them.
        "1.3.101.112" => ed25519_key(info.private_key),
        other => Err(format!("Private key algorithm \"{other}\" is not supported.")),
    }
}

/// `OpenSSH`: `openssh-key-v1`, encrypted with a bcrypt-derived key or
/// not, one key (Ed25519, ECDSA, RSA), its check numbers and padding.
fn openssh(data: &[u8], passphrase: Option<&str>) -> Result<UserKey, String> {
    let e = |e: crate::ssh::SshError| e.0;
    const MAGIC: &[u8] = b"openssh-key-v1\0";
    if data.get(..MAGIC.len()) != Some(MAGIC) {
        return Err("This openssh key does not contain the 'openssh-key-v1' format magic header".into());
    }
    let mut r = Reader::new(&data[MAGIC.len()..]);
    let cipher = r.string().map_err(e)?;
    let kdf = r.string().map_err(e)?;
    let (mut salt, mut rounds) = (Vec::new(), 0u32);
    if r.u32().map_err(e)? as i32 > 0 {
        salt = r.bytes().map_err(e)?.to_vec();
        rounds = r.u32().map_err(e)?;
    }
    if r.u32().map_err(e)? != 1 {
        return Err("At this time only one public key in the openssh key is supported.".into());
    }
    r.bytes().map_err(e)?;
    let len = r.u32().map_err(e)? as usize;
    let rest = r.rest();
    let private = if cipher != "none" {
        let pass = passphrase.ok_or_else(|| ENCRYPTED_NO_PASSPHRASE.to_owned())?;
        if kdf != "bcrypt" {
            return Err(format!("kdf {kdf} is not supported for openssh key file"));
        }
        let unsupported = || format!("Cipher '{cipher}' is not supported for an OpenSSH key.");
        // SSH.NET's key and IV sizes for each cipher it reads.
        let (key_len, iv_len, tag_len) = match cipher.as_str() {
            "aes128-cbc" | "aes128-ctr" => (16, 16, 0),
            "aes192-cbc" | "aes192-ctr" => (24, 16, 0),
            "aes256-cbc" | "aes256-ctr" => (32, 16, 0),
            "3des-cbc" => (24, 8, 0),
            "aes128-gcm@openssh.com" => (16, 16, 16),
            "aes256-gcm@openssh.com" => (32, 16, 16),
            "chacha20-poly1305@openssh.com" => (32, 12, 16),
            _ => return Err(unsupported()),
        };
        let mut derived = vec![0u8; key_len + iv_len];
        bcrypt_pbkdf::bcrypt_pbkdf(pass.as_bytes(), &salt, rounds, &mut derived).map_err(|e| e.to_string())?;
        let mut buf = rest.get(..len).ok_or_else(|| "Unable to read beyond the end of the stream.".to_owned())?.to_vec();
        let tag: Option<[u8; 16]> = if tag_len > 0 { rest.get(len..len + 16).and_then(|t| t.try_into().ok()) } else { None };
        let c = ssh_key::Cipher::new(&cipher).map_err(|_| unsupported())?;
        c.decrypt(&derived[..key_len], &derived[key_len..], &mut buf, tag).map_err(|_| "The computed authentication tag did not match the input authentication tag.".to_owned())?;
        buf
    } else {
        rest.get(..len).ok_or_else(|| "Unable to read beyond the end of the stream.".to_owned())?.to_vec()
    };
    if !private.len().is_multiple_of(8) {
        return Err("The private key section must be a multiple of the block size (8)".into());
    }
    let mut s = Reader::new(&private);
    let (a, b) = (s.u32().map_err(e)? as i32, s.u32().map_err(e)? as i32);
    if a != b {
        return Err(format!("The random check bytes of the OpenSSH key do not match ({a} <-> {b})."));
    }
    let kind = s.string().map_err(e)?;
    let key = match kind.as_str() {
        "ssh-ed25519" => {
            s.bytes().map_err(e)?;
            ed25519_key(s.bytes().map_err(e)?)?
        }
        "ecdsa-sha2-nistp256" | "ecdsa-sha2-nistp384" | "ecdsa-sha2-nistp521" => {
            let curve = s.string().map_err(e)?;
            let public = s.bytes().map_err(e)?.to_vec();
            ecdsa_key(&curve, &public, s.bytes().map_err(e)?)?
        }
        "ssh-rsa" => {
            let n = s.bytes().map_err(e)?.to_vec();
            let ex = s.bytes().map_err(e)?.to_vec();
            let d = s.bytes().map_err(e)?.to_vec();
            s.bytes().map_err(e)?;
            let p = s.bytes().map_err(e)?.to_vec();
            let q = s.bytes().map_err(e)?.to_vec();
            rsa_from_parts(&n, &ex, &d, &p, &q)?
        }
        other => return Err(format!("OpenSSH key type '{other}' is not supported.")),
    };
    s.string().map_err(e)?;
    for (i, b) in s.rest().iter().enumerate() {
        if *b as usize != i + 1 {
            return Err(format!("Padding of openssh key format contained wrong byte at position: {i}"));
        }
    }
    Ok(key)
}

/// `PuTTY`: versions 2 and 3, `aes256-cbc` or not, the MAC checked.
fn putty_key(caps: &fancy_regex::Captures, passphrase: Option<&str>) -> Result<UserKey, String> {
    use hmac::{Hmac, Mac};
    let group = |name: &str| caps.name(name).map(|m| m.as_str().to_owned()).unwrap_or_default();
    let (version, algorithm, encryption, comment) = (group("version"), group("algorithmName"), group("encryptionType"), group("comment"));
    let public = from_base64(&group("publicKey"))?;
    let data = from_base64(&group("data"))?;
    enum MacKey {
        Sha1(Vec<u8>),
        Sha256(Vec<u8>),
    }
    let (private, mac_key) = match encryption.as_str() {
        "none" => {
            let mac_key = match version.as_str() {
                "3" => MacKey::Sha256(Vec::new()),
                "2" => MacKey::Sha1(<sha1::Sha1 as sha1::Digest>::digest(b"putty-private-key-file-mac-key").to_vec()),
                v => return Err(format!("PuTTY key file version {v} is not supported")),
            };
            (data, mac_key)
        }
        "aes256-cbc" => {
            let pass = passphrase.ok_or_else(|| ENCRYPTED_NO_PASSPHRASE.to_owned())?;
            let (key, iv, mac_key) = match version.as_str() {
                "3" => {
                    let kdf = group("argon2Type");
                    let algorithm = match kdf.as_str() {
                        "Argon2i" => argon2::Algorithm::Argon2i,
                        "Argon2d" => argon2::Algorithm::Argon2d,
                        "Argon2id" => argon2::Algorithm::Argon2id,
                        other => return Err(format!("KDF {other} is not supported for PuTTY key file")),
                    };
                    let number = |n: &str| group(n).parse::<u32>().map_err(|_| format!("The input string '{}' was not in a correct format.", group(n)));
                    let params = argon2::Params::new(number("argon2Memory")?, number("argon2Passes")?, number("argon2Parallelism")?, Some(80))
                        .map_err(|e| e.to_string())?;
                    let mut out = [0u8; 80];
                    argon2::Argon2::new(algorithm, argon2::Version::V0x13, params)
                        .hash_password_into(pass.as_bytes(), &from_hex(&group("argon2Salt"))?, &mut out)
                        .map_err(|_| "Failed to generate key via Argon2".to_owned())?;
                    (out[..32].to_vec(), out[32..48].to_vec(), MacKey::Sha256(out[48..80].to_vec()))
                }
                "2" => {
                    let mut key = Vec::new();
                    for i in 0..2u32 {
                        let mut d = <sha1::Sha1 as sha1::Digest>::new();
                        sha1::Digest::update(&mut d, i.to_be_bytes());
                        sha1::Digest::update(&mut d, pass.as_bytes());
                        key.extend_from_slice(&sha1::Digest::finalize(d));
                    }
                    let mac = <sha1::Sha1 as sha1::Digest>::digest(format!("putty-private-key-file-mac-key{pass}").as_bytes()).to_vec();
                    (key[..32].to_vec(), vec![0u8; 16], MacKey::Sha1(mac))
                }
                v => return Err(format!("PuTTY key file version {v} is not supported")),
            };
            use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding};
            let mut buf = data.clone();
            if !buf.len().is_multiple_of(16) {
                return Err("The input data is not a complete block.".into());
            }
            cbc::Decryptor::<aes::Aes256>::new_from_slices(&key, &iv).map_err(|e| e.to_string())?.decrypt_padded_mut::<NoPadding>(&mut buf).map_err(|e| e.to_string())?;
            (buf, mac_key)
        }
        other => return Err(format!("Encryption {other} is not supported for PuTTY key file")),
    };
    let mac_data = crate::ssh::Writer::default().string(&algorithm).string(&encryption).string(&comment).bytes(&public).bytes(&private).0;
    let mac = match mac_key {
        MacKey::Sha1(k) => {
            let mut m = <Hmac<sha1::Sha1> as Mac>::new_from_slice(&k).expect("any key");
            m.update(&mac_data);
            m.finalize().into_bytes().to_vec()
        }
        MacKey::Sha256(k) => {
            let mut m = <Hmac<sha2::Sha256> as Mac>::new_from_slice(&k).expect("any key");
            m.update(&mac_data);
            m.finalize().into_bytes().to_vec()
        }
    };
    if mac != from_hex(&group("mac"))? {
        return Err("MAC verification failed for PuTTY key file".into());
    }
    let mut p = Reader::new(&public);
    let kind = p.string().map_err(|e| e.0)?;
    let mut s = Reader::new(&private);
    let e = |e: crate::ssh::SshError| e.0;
    match kind.as_str() {
        "ssh-ed25519" => ed25519_key(s.bytes().map_err(e)?),
        "ecdsa-sha2-nistp256" | "ecdsa-sha2-nistp384" | "ecdsa-sha2-nistp521" => {
            let curve = p.string().map_err(e)?;
            let point = p.bytes().map_err(e)?.to_vec();
            ecdsa_key(&curve, &point, s.bytes().map_err(e)?)
        }
        "ssh-rsa" => {
            let ex = p.bytes().map_err(e)?.to_vec();
            let n = p.bytes().map_err(e)?.to_vec();
            let d = s.bytes().map_err(e)?.to_vec();
            let pp = s.bytes().map_err(e)?.to_vec();
            let q = s.bytes().map_err(e)?.to_vec();
            rsa_from_parts(&n, &ex, &d, &pp, &q)
        }
        other => Err(format!("Key type {other} is not supported for PuTTY key file")),
    }
}

/// `SSHCOM`: RSA, plain or `3des-cbc` (a key made from the passphrase
/// with MD5).
fn sshcom(data: &[u8], passphrase: Option<&str>) -> Result<UserKey, String> {
    let e = |e: crate::ssh::SshError| e.0;
    let mut r = Reader::new(data);
    if r.u32().map_err(e)? != 0x3F6F_F9EB {
        return Err("Invalid SSH2 private key.".into());
    }
    r.u32().map_err(e)?;
    let key_type = r.string().map_err(e)?;
    let cipher = r.string().map_err(e)?;
    let len = r.u32().map_err(e)? as usize;
    let blob = r.rest().get(..len).ok_or_else(|| "Unable to read beyond the end of the stream.".to_owned())?.to_vec();
    let plain = match cipher.as_str() {
        "none" => blob,
        "3des-cbc" => {
            let pass = passphrase.ok_or_else(|| ENCRYPTED_NO_PASSPHRASE.to_owned())?;
            let mut key = Vec::new();
            let mut h = <md5::Md5 as md5::Digest>::digest(pass.as_bytes()).to_vec();
            key.extend_from_slice(&h);
            while key.len() < 24 {
                let mut next = pass.as_bytes().to_vec();
                next.extend_from_slice(&h);
                h = <md5::Md5 as md5::Digest>::digest(&next).to_vec();
                key.extend_from_slice(&h);
            }
            use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding};
            let mut buf = blob;
            cbc::Decryptor::<des::TdesEde3>::new_from_slices(&key[..24], &[0u8; 8])
                .map_err(|e| e.to_string())?
                .decrypt_padded_mut::<NoPadding>(&mut buf)
                .map_err(|_| "The input data is not a complete block.".to_owned())?;
            buf
        }
        other => return Err(format!("Cipher method '{other}' is not supported.")),
    };
    let mut s = Reader::new(&plain);
    if s.u32().map_err(e)? as usize > len.saturating_sub(4) {
        return Err("Invalid passphrase.".into());
    }
    if !key_type.contains("rsa") {
        return Err(format!("Key type '{key_type}' is not supported."));
    }
    let mut read = || -> Result<Vec<u8>, String> {
        let bits = s.u32().map_err(e)? as usize;
        let n = bits.div_ceil(8);
        let mut out = vec![0u8; n];
        for b in out.iter_mut() {
            *b = s.byte().map_err(e)?;
        }
        Ok(out)
    };
    let ex = read()?;
    let d = read()?;
    let n = read()?;
    let _iqmp = read()?;
    let q = read()?;
    let p = read()?;
    rsa_from_parts(&n, &ex, &d, &p, &q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
    }

    #[test]
    fn every_format_reads_to_its_public_key() {
        let pass = Some("made-up-passphrase");
        for (name, p) in [
            ("sftp_test_rsa_pem_aes", pass),
            ("sftp_test_rsa_pem_3des", pass),
            ("sftp_test_rsa_pkcs8_enc", pass),
            ("sftp_test_ec384_pem", pass),
            ("sftp_test_ec256_pkcs8", None),
            ("sftp_test_ec521", None),
            ("sftp_test_rsa_v2.ppk", pass),
            ("sftp_test_rsa_v3.ppk", pass),
            ("sftp_test_rsa_v3_plain.ppk", None),
            ("sftp_test_rsa_sshcom", pass),
            ("sftp_test_rsa", pass),
            ("sftp_test_ed25519", None),
        ] {
            let key = load(&fixture(name), p).unwrap_or_else(|e| panic!("{name}: {e}"));
            let expected = ssh_key::PublicKey::from_openssh(fixture(&format!("{name}.pub")).trim()).unwrap();
            assert_eq!(key.public_key().key_data(), expected.key_data(), "{name}");
        }
    }

    #[test]
    fn sshnet_errors() {
        assert_eq!(load(&fixture("sftp_test_rsa_pem_aes"), None).unwrap_err(), ENCRYPTED_NO_PASSPHRASE);
        assert_eq!(load(&fixture("sftp_test_rsa_v2.ppk"), Some("wrong")).unwrap_err(), "MAC verification failed for PuTTY key file");
        assert_eq!(load(&fixture("sftp_test_rsa_sshcom"), Some("wrong")).unwrap_err(), "Invalid passphrase.");
        assert_eq!(load(&fixture("sftp_test_rsa.pub"), None).unwrap_err(), "Invalid private key file.");
        assert_eq!(load(&fixture("sftp_test_ed25519_pkcs8_enc"), Some("wrong")).unwrap_err(), "pad block corrupted");
        assert!(load(&fixture("sftp_test_rsa"), Some("wrong")).unwrap_err().starts_with("The random check bytes of the OpenSSH key do not match ("));
        // The wrapped Ed25519 PKCS#8 key isn't the key its .pub names.
        let key = load(&fixture("sftp_test_ed25519_pkcs8_enc"), Some("made-up-passphrase")).unwrap();
        let expected = ssh_key::PublicKey::from_openssh(fixture("sftp_test_ed25519_pkcs8_enc.pub").trim()).unwrap();
        assert_ne!(key.public_key().key_data(), expected.key_data());
    }
}
