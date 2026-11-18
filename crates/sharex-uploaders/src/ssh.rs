//! A small SSH-2 implementation (RFC 4253, 4252, 4254) for SFTP uploads,
//! standing in for SSH.NET: `curve25519-sha256` key exchange, `ssh-ed25519`,
//! `ecdsa-sha2-nistp256` and `rsa-sha2-256/512` host keys (signature
//! checked; any key is accepted, as ShareX does), `aes{128,192,256}-ctr`
//! with `hmac-sha2-{256,512}`, password and public key authentication, one
//! session channel with a subsystem. The server role is here too, for the
//! mock SFTP server.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use aes::cipher::{KeyIvInit, StreamCipher};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256, Sha512};

pub const MSG_DISCONNECT: u8 = 1;
pub const MSG_IGNORE: u8 = 2;
pub const MSG_UNIMPLEMENTED: u8 = 3;
pub const MSG_DEBUG: u8 = 4;
pub const MSG_SERVICE_REQUEST: u8 = 5;
pub const MSG_SERVICE_ACCEPT: u8 = 6;
pub const MSG_EXT_INFO: u8 = 7;
pub const MSG_KEXINIT: u8 = 20;
pub const MSG_NEWKEYS: u8 = 21;
pub const MSG_KEX_ECDH_INIT: u8 = 30;
pub const MSG_KEX_ECDH_REPLY: u8 = 31;
pub const MSG_USERAUTH_REQUEST: u8 = 50;
pub const MSG_USERAUTH_FAILURE: u8 = 51;
pub const MSG_USERAUTH_SUCCESS: u8 = 52;
pub const MSG_USERAUTH_BANNER: u8 = 53;
pub const MSG_USERAUTH_PK_OK: u8 = 60;
pub const MSG_GLOBAL_REQUEST: u8 = 80;
pub const MSG_REQUEST_FAILURE: u8 = 82;
pub const MSG_CHANNEL_OPEN: u8 = 90;
pub const MSG_CHANNEL_OPEN_CONFIRMATION: u8 = 91;
pub const MSG_CHANNEL_OPEN_FAILURE: u8 = 92;
pub const MSG_CHANNEL_WINDOW_ADJUST: u8 = 93;
pub const MSG_CHANNEL_DATA: u8 = 94;
pub const MSG_CHANNEL_EXTENDED_DATA: u8 = 95;
pub const MSG_CHANNEL_EOF: u8 = 96;
pub const MSG_CHANNEL_CLOSE: u8 = 97;
pub const MSG_CHANNEL_REQUEST: u8 = 98;
pub const MSG_CHANNEL_SUCCESS: u8 = 99;
pub const MSG_CHANNEL_FAILURE: u8 = 100;

const KEX_ALGORITHMS: &[&str] = &["curve25519-sha256", "curve25519-sha256@libssh.org"];
const HOST_KEY_ALGORITHMS: &[&str] = &["ssh-ed25519", "ecdsa-sha2-nistp256", "rsa-sha2-512", "rsa-sha2-256"];
const CIPHERS: &[&str] = &["aes128-ctr", "aes192-ctr", "aes256-ctr"];
const MACS: &[&str] = &["hmac-sha2-256", "hmac-sha2-512"];

/// An SSH failure: its message, as the exception ShareX would show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshError(pub String);

impl std::fmt::Display for SshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<std::io::Error> for SshError {
    fn from(e: std::io::Error) -> Self {
        SshError(e.to_string())
    }
}

pub type SshResult<T> = Result<T, SshError>;

fn err<T>(m: impl Into<String>) -> SshResult<T> {
    Err(SshError(m.into()))
}

// ---------------------------------------------------------------------------
// Wire encoding.

/// Builds a message.
#[derive(Default, Clone)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn new(msg: u8) -> Self {
        Self(vec![msg])
    }
    pub fn byte(mut self, b: u8) -> Self {
        self.0.push(b);
        self
    }
    pub fn bool(self, b: bool) -> Self {
        self.byte(b as u8)
    }
    pub fn u32(mut self, v: u32) -> Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn u64(mut self, v: u64) -> Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn bytes(mut self, b: &[u8]) -> Self {
        self.0.extend_from_slice(&(b.len() as u32).to_be_bytes());
        self.0.extend_from_slice(b);
        self
    }
    pub fn string(self, s: &str) -> Self {
        self.bytes(s.as_bytes())
    }
    pub fn raw(mut self, b: &[u8]) -> Self {
        self.0.extend_from_slice(b);
        self
    }
    /// An `mpint` of an unsigned big-endian number.
    pub fn mpint(self, n: &[u8]) -> Self {
        let n: &[u8] = &n[n.iter().take_while(|b| **b == 0).count()..];
        if n.first().is_some_and(|b| b & 0x80 != 0) {
            let mut v = vec![0u8];
            v.extend_from_slice(n);
            self.bytes(&v)
        } else {
            self.bytes(n)
        }
    }
}

/// Reads a message.
pub struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn take(&mut self, n: usize) -> SshResult<&'a [u8]> {
        if self.pos + n > self.data.len() {
            return err("Malformed SSH message.");
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn byte(&mut self) -> SshResult<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> SshResult<bool> {
        Ok(self.byte()? != 0)
    }
    pub fn u32(&mut self) -> SshResult<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().expect("4 bytes")))
    }
    pub fn u64(&mut self) -> SshResult<u64> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().expect("8 bytes")))
    }
    pub fn bytes(&mut self) -> SshResult<&'a [u8]> {
        let n = self.u32()? as usize;
        self.take(n)
    }
    pub fn string(&mut self) -> SshResult<String> {
        Ok(String::from_utf8_lossy(self.bytes()?).into_owned())
    }
    pub fn name_list(&mut self) -> SshResult<Vec<String>> {
        let s = self.string()?;
        Ok(if s.is_empty() { Vec::new() } else { s.split(',').map(str::to_owned).collect() })
    }
    pub fn rest(&mut self) -> &'a [u8] {
        let s = &self.data[self.pos..];
        self.pos = self.data.len();
        s
    }
}

// ---------------------------------------------------------------------------
// Ciphers and MACs.

enum Ctr {
    Aes128(ctr::Ctr128BE<aes::Aes128>),
    Aes192(ctr::Ctr128BE<aes::Aes192>),
    Aes256(ctr::Ctr128BE<aes::Aes256>),
}

impl Ctr {
    fn new(name: &str, key: &[u8], iv: &[u8]) -> Self {
        match name {
            "aes256-ctr" => Ctr::Aes256(ctr::Ctr128BE::new_from_slices(&key[..32], &iv[..16]).expect("sizes")),
            "aes192-ctr" => Ctr::Aes192(ctr::Ctr128BE::new_from_slices(&key[..24], &iv[..16]).expect("sizes")),
            _ => Ctr::Aes128(ctr::Ctr128BE::new_from_slices(&key[..16], &iv[..16]).expect("sizes")),
        }
    }
    fn apply(&mut self, data: &mut [u8]) {
        match self {
            Ctr::Aes128(c) => c.apply_keystream(data),
            Ctr::Aes192(c) => c.apply_keystream(data),
            Ctr::Aes256(c) => c.apply_keystream(data),
        }
    }
}

fn key_len(cipher: &str) -> usize {
    match cipher {
        "aes256-ctr" => 32,
        "aes192-ctr" => 24,
        _ => 16,
    }
}

fn mac_len(mac: &str) -> usize {
    if mac == "hmac-sha2-512" { 64 } else { 32 }
}

fn compute_mac(mac: &str, key: &[u8], seq: u32, packet: &[u8]) -> Vec<u8> {
    if mac == "hmac-sha2-512" {
        let mut m = Hmac::<Sha512>::new_from_slice(key).expect("any key");
        m.update(&seq.to_be_bytes());
        m.update(packet);
        m.finalize().into_bytes().to_vec()
    } else {
        let mut m = Hmac::<Sha256>::new_from_slice(key).expect("any key");
        m.update(&seq.to_be_bytes());
        m.update(packet);
        m.finalize().into_bytes().to_vec()
    }
}

struct Direction {
    cipher: Option<Ctr>,
    mac: Option<(String, Vec<u8>)>,
    seq: u32,
}

impl Direction {
    fn new() -> Self {
        Self { cipher: None, mac: None, seq: 0 }
    }
}

// ---------------------------------------------------------------------------
// Transport.

/// The role this end plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Client,
    Server,
}

/// The server's host key, for the server role.
pub type HostKey = ssh_key::PrivateKey;

/// An SSH transport over TCP after key exchange.
pub struct Transport {
    stream: TcpStream,
    send: Direction,
    recv: Direction,
    pub session_id: Vec<u8>,
    /// The server's host key blob (client role).
    pub server_host_key: Vec<u8>,
}

fn random_bytes(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).expect("OS random numbers");
    b
}

fn kexinit_payload() -> Vec<u8> {
    Writer::new(MSG_KEXINIT)
        .raw(&random_bytes(16))
        .string(&KEX_ALGORITHMS.join(","))
        .string(&HOST_KEY_ALGORITHMS.join(","))
        .string(&CIPHERS.join(","))
        .string(&CIPHERS.join(","))
        .string(&MACS.join(","))
        .string(&MACS.join(","))
        .string("none")
        .string("none")
        .string("")
        .string("")
        .bool(false)
        .u32(0)
        .0
}

fn server_kexinit_payload(host_key_algorithm: &str) -> Vec<u8> {
    let mut w = Writer::new(MSG_KEXINIT).raw(&random_bytes(16)).string(&KEX_ALGORITHMS.join(",")).string(host_key_algorithm);
    w = w.string(&CIPHERS.join(",")).string(&CIPHERS.join(",")).string(&MACS.join(",")).string(&MACS.join(","));
    w.string("none").string("none").string("").string("").bool(false).u32(0).0
}

/// The first client algorithm the server also has.
fn negotiate(client: &[String], server: &[String], what: &str) -> SshResult<String> {
    client.iter().find(|c| server.contains(c)).cloned().ok_or_else(|| SshError(format!("No matching {what} algorithm found.")))
}

struct KexInit {
    kex: Vec<String>,
    host_key: Vec<String>,
    enc_cs: Vec<String>,
    enc_sc: Vec<String>,
    mac_cs: Vec<String>,
    mac_sc: Vec<String>,
}

fn parse_kexinit(payload: &[u8]) -> SshResult<KexInit> {
    let mut r = Reader::new(payload);
    r.byte()?;
    r.take(16)?;
    Ok(KexInit {
        kex: r.name_list()?,
        host_key: r.name_list()?,
        enc_cs: r.name_list()?,
        enc_sc: r.name_list()?,
        mac_cs: r.name_list()?,
        mac_sc: r.name_list()?,
    })
}

/// `HASH(K || H || X || session_id)`, extended to `len` bytes.
fn derive(k: &[u8], h: &[u8], letter: u8, session_id: &[u8], len: usize) -> Vec<u8> {
    let kk = Writer::default().mpint(k).0;
    let mut out = Sha256::new().chain_update(&kk).chain_update(h).chain_update([letter]).chain_update(session_id).finalize().to_vec();
    while out.len() < len {
        let more = Sha256::new().chain_update(&kk).chain_update(h).chain_update(&out).finalize();
        out.extend_from_slice(&more);
    }
    out.truncate(len);
    out
}

impl Transport {
    /// Writes one packet.
    pub fn send(&mut self, payload: &[u8]) -> SshResult<()> {
        let block = if self.send.cipher.is_some() { 16 } else { 8 };
        let mut padding = block - (5 + payload.len()) % block;
        if padding < 4 {
            padding += block;
        }
        let mut packet = Vec::with_capacity(5 + payload.len() + padding);
        packet.extend_from_slice(&((1 + payload.len() + padding) as u32).to_be_bytes());
        packet.push(padding as u8);
        packet.extend_from_slice(payload);
        packet.extend_from_slice(&random_bytes(padding));
        let mac = self.send.mac.as_ref().map(|(name, key)| compute_mac(name, key, self.send.seq, &packet));
        if let Some(c) = self.send.cipher.as_mut() {
            c.apply(&mut packet);
        }
        self.stream.write_all(&packet)?;
        if let Some(m) = mac {
            self.stream.write_all(&m)?;
        }
        self.stream.flush()?;
        self.send.seq = self.send.seq.wrapping_add(1);
        Ok(())
    }

    /// Reads one packet's payload.
    pub fn recv_raw(&mut self) -> SshResult<Vec<u8>> {
        let block = if self.recv.cipher.is_some() { 16 } else { 8 };
        let mut first = vec![0u8; block];
        self.stream.read_exact(&mut first)?;
        if let Some(c) = self.recv.cipher.as_mut() {
            c.apply(&mut first);
        }
        let len = u32::from_be_bytes(first[..4].try_into().expect("4 bytes")) as usize;
        if !(5..=256 * 1024).contains(&len) || !(len + 4).is_multiple_of(block) {
            return err("Bad SSH packet length.");
        }
        let mut rest = vec![0u8; len + 4 - block];
        self.stream.read_exact(&mut rest)?;
        if let Some(c) = self.recv.cipher.as_mut() {
            c.apply(&mut rest);
        }
        let mut packet = first;
        packet.extend_from_slice(&rest);
        if let Some((name, key)) = &self.recv.mac {
            let mut mac = vec![0u8; mac_len(name)];
            self.stream.read_exact(&mut mac)?;
            if compute_mac(name, key, self.recv.seq, &packet) != mac {
                return err("Bad SSH packet MAC.");
            }
        }
        self.recv.seq = self.recv.seq.wrapping_add(1);
        let padding = packet[4] as usize;
        if padding + 1 > len {
            return err("Bad SSH packet padding.");
        }
        Ok(packet[5..4 + len - padding].to_vec())
    }

    /// Reads the next packet that isn't IGNORE, DEBUG or UNIMPLEMENTED; a
    /// DISCONNECT becomes an error.
    pub fn recv(&mut self) -> SshResult<Vec<u8>> {
        loop {
            let p = self.recv_raw()?;
            match p.first().copied() {
                Some(MSG_IGNORE | MSG_DEBUG | MSG_UNIMPLEMENTED | MSG_EXT_INFO) => continue,
                Some(MSG_DISCONNECT) => {
                    let mut r = Reader::new(&p[1..]);
                    let _ = r.u32();
                    return err(r.string().unwrap_or_else(|_| "The server disconnected.".into()));
                }
                Some(MSG_GLOBAL_REQUEST) => {
                    let mut r = Reader::new(&p[1..]);
                    let _ = r.string();
                    if r.bool().unwrap_or(false) {
                        self.send(&[MSG_REQUEST_FAILURE])?;
                    }
                    continue;
                }
                Some(_) => return Ok(p),
                None => return err("Empty SSH packet."),
            }
        }
    }

    fn version_exchange(stream: &mut TcpStream, ours: &str) -> SshResult<String> {
        stream.write_all(format!("{ours}\r\n").as_bytes())?;
        loop {
            let mut line = Vec::new();
            let mut b = [0u8; 1];
            loop {
                if stream.read(&mut b)? == 0 {
                    return err("The server closed the connection.");
                }
                if b[0] == b'\n' {
                    break;
                }
                line.push(b[0]);
                if line.len() > 8192 {
                    return err("Bad SSH identification.");
                }
            }
            let line = String::from_utf8_lossy(&line).trim_end_matches('\r').to_owned();
            if line.starts_with("SSH-") {
                if !line.starts_with("SSH-2.0-") && !line.starts_with("SSH-1.99-") {
                    return err(format!("Server version '{line}' is not supported."));
                }
                return Ok(line);
            }
        }
    }

    /// Connects as a client: version exchange and key exchange.
    pub fn connect(mut stream: TcpStream, client_version: &str) -> SshResult<Self> {
        let server_version = Self::version_exchange(&mut stream, client_version)?;
        let mut t = Transport { stream, send: Direction::new(), recv: Direction::new(), session_id: Vec::new(), server_host_key: Vec::new() };
        let i_c = kexinit_payload();
        t.send(&i_c)?;
        let i_s = t.recv()?;
        if i_s.first() != Some(&MSG_KEXINIT) {
            return err("Expected KEXINIT.");
        }
        let server = parse_kexinit(&i_s)?;
        let client = parse_kexinit(&i_c)?;
        negotiate(&client.kex, &server.kex, "key exchange")?;
        let host_key_alg = negotiate(&client.host_key, &server.host_key, "host key")?;
        let enc_cs = negotiate(&client.enc_cs, &server.enc_cs, "encryption")?;
        let enc_sc = negotiate(&client.enc_sc, &server.enc_sc, "encryption")?;
        let mac_cs = negotiate(&client.mac_cs, &server.mac_cs, "MAC")?;
        let mac_sc = negotiate(&client.mac_sc, &server.mac_sc, "MAC")?;
        let secret = x25519_dalek::EphemeralSecret::random_from_rng(rand_core::OsRng);
        let q_c = x25519_dalek::PublicKey::from(&secret);
        t.send(&Writer::new(MSG_KEX_ECDH_INIT).bytes(q_c.as_bytes()).0)?;
        let reply = t.recv()?;
        if reply.first() != Some(&MSG_KEX_ECDH_REPLY) {
            return err("Expected KEX_ECDH_REPLY.");
        }
        let mut r = Reader::new(&reply[1..]);
        let k_s = r.bytes()?.to_vec();
        let q_s: [u8; 32] = r.bytes()?.try_into().map_err(|_| SshError("Bad server key exchange value.".into()))?;
        let signature = r.bytes()?.to_vec();
        let k = secret.diffie_hellman(&x25519_dalek::PublicKey::from(q_s)).to_bytes();
        let h = Sha256::digest(
            Writer::default()
                .string(client_version)
                .string(&server_version)
                .bytes(&i_c)
                .bytes(&i_s)
                .bytes(&k_s)
                .bytes(q_c.as_bytes())
                .bytes(&q_s)
                .mpint(&k)
                .0,
        )
        .to_vec();
        verify_host_signature(&k_s, &host_key_alg, &h, &signature)?;
        t.server_host_key = k_s;
        t.session_id = h.clone();
        t.send(&[MSG_NEWKEYS])?;
        if t.recv()?.first() != Some(&MSG_NEWKEYS) {
            return err("Expected NEWKEYS.");
        }
        t.install_keys(&k, &h, Role::Client, &enc_cs, &enc_sc, &mac_cs, &mac_sc);
        Ok(t)
    }

    /// Accepts a client as the server, signing with `host_key` (ed25519).
    pub fn accept(mut stream: TcpStream, server_version: &str, host_key: &HostKey) -> SshResult<Self> {
        let client_version = Self::version_exchange(&mut stream, server_version)?;
        let mut t = Transport { stream, send: Direction::new(), recv: Direction::new(), session_id: Vec::new(), server_host_key: Vec::new() };
        let i_s = server_kexinit_payload("ssh-ed25519");
        t.send(&i_s)?;
        let i_c = t.recv()?;
        if i_c.first() != Some(&MSG_KEXINIT) {
            return err("Expected KEXINIT.");
        }
        let client = parse_kexinit(&i_c)?;
        let server = parse_kexinit(&i_s)?;
        negotiate(&client.kex, &server.kex, "key exchange")?;
        let enc_cs = negotiate(&client.enc_cs, &server.enc_cs, "encryption")?;
        let enc_sc = negotiate(&client.enc_sc, &server.enc_sc, "encryption")?;
        let mac_cs = negotiate(&client.mac_cs, &server.mac_cs, "MAC")?;
        let mac_sc = negotiate(&client.mac_sc, &server.mac_sc, "MAC")?;
        let init = t.recv()?;
        if init.first() != Some(&MSG_KEX_ECDH_INIT) {
            return err("Expected KEX_ECDH_INIT.");
        }
        let q_c: [u8; 32] = Reader::new(&init[1..]).bytes()?.try_into().map_err(|_| SshError("Bad client key exchange value.".into()))?;
        let secret = x25519_dalek::EphemeralSecret::random_from_rng(rand_core::OsRng);
        let q_s = x25519_dalek::PublicKey::from(&secret);
        let k = secret.diffie_hellman(&x25519_dalek::PublicKey::from(q_c)).to_bytes();
        let k_s = host_key.public_key().to_bytes().map_err(|e| SshError(e.to_string()))?;
        let h = Sha256::digest(
            Writer::default()
                .string(&client_version)
                .string(server_version)
                .bytes(&i_c)
                .bytes(&i_s)
                .bytes(&k_s)
                .bytes(&q_c)
                .bytes(q_s.as_bytes())
                .mpint(&k)
                .0,
        )
        .to_vec();
        let signature = sign(host_key, &h)?;
        t.send(&Writer::new(MSG_KEX_ECDH_REPLY).bytes(&k_s).bytes(q_s.as_bytes()).bytes(&signature).0)?;
        t.send(&[MSG_NEWKEYS])?;
        if t.recv()?.first() != Some(&MSG_NEWKEYS) {
            return err("Expected NEWKEYS.");
        }
        t.session_id = h.clone();
        t.install_keys(&k, &h, Role::Server, &enc_cs, &enc_sc, &mac_cs, &mac_sc);
        Ok(t)
    }

    #[allow(clippy::too_many_arguments)]
    fn install_keys(&mut self, k: &[u8], h: &[u8], role: Role, enc_cs: &str, enc_sc: &str, mac_cs: &str, mac_sc: &str) {
        let sid = self.session_id.clone();
        let iv_cs = derive(k, h, b'A', &sid, 16);
        let iv_sc = derive(k, h, b'B', &sid, 16);
        let key_cs = derive(k, h, b'C', &sid, key_len(enc_cs));
        let key_sc = derive(k, h, b'D', &sid, key_len(enc_sc));
        let mac_key_cs = derive(k, h, b'E', &sid, mac_len(mac_cs));
        let mac_key_sc = derive(k, h, b'F', &sid, mac_len(mac_sc));
        let cs = (Ctr::new(enc_cs, &key_cs, &iv_cs), (mac_cs.to_owned(), mac_key_cs));
        let sc = (Ctr::new(enc_sc, &key_sc, &iv_sc), (mac_sc.to_owned(), mac_key_sc));
        let (out, inc) = if role == Role::Client { (cs, sc) } else { (sc, cs) };
        self.send.cipher = Some(out.0);
        self.send.mac = Some(out.1);
        self.recv.cipher = Some(inc.0);
        self.recv.mac = Some(inc.1);
    }

    /// Closes the connection with a DISCONNECT ("bye").
    pub fn disconnect(&mut self) {
        let _ = self.send(&Writer::new(MSG_DISCONNECT).u32(11).string("bye").string("").0);
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }

    pub fn set_timeout(&self, timeout: Option<Duration>) {
        let _ = self.stream.set_read_timeout(timeout);
        let _ = self.stream.set_write_timeout(timeout);
    }
}

/// Signs the exchange hash with the host key (SSH signature blob).
fn sign(key: &HostKey, data: &[u8]) -> SshResult<Vec<u8>> {
    use signature::Signer;
    let sig: ssh_key::Signature = key.key_data().try_sign(data).map_err(|e| SshError(e.to_string()))?;
    Ok(Writer::default().string(sig.algorithm().as_str()).bytes(sig.as_bytes()).0)
}

/// Checks the server's signature over the exchange hash with its host key.
fn verify_host_signature(key_blob: &[u8], algorithm: &str, h: &[u8], signature_blob: &[u8]) -> SshResult<()> {
    use signature::Verifier;
    let key = ssh_key::PublicKey::from_bytes(key_blob).map_err(|e| SshError(format!("Bad host key: {e}")))?;
    let sig = ssh_key::Signature::try_from(signature_blob).map_err(|e| SshError(format!("Bad host key signature: {e}")))?;
    if algorithm.starts_with("rsa-sha2") && sig.algorithm().as_str() != algorithm {
        return err("Host key signature algorithm mismatch.");
    }
    key.key_data().verify(h, &sig).map_err(|_| SshError("Key exchange negotiation failed: the host key signature is wrong.".into()))
}

/// A public-key login's key (`PrivateKeyFile`).
pub type UserKey = ssh_key::PrivateKey;

/// The signature a public key login sends (`string alg, string blob`);
/// RSA keys sign with SHA-512 (`rsa-sha2-512`).
pub fn sign_user_auth(key: &UserKey, data: &[u8]) -> SshResult<(String, Vec<u8>)> {
    use signature::Signer;
    let sig: ssh_key::Signature = key.key_data().try_sign(data).map_err(|e| SshError(e.to_string()))?;
    let algorithm = sig.algorithm().as_str().to_owned();
    let blob = Writer::default().string(&algorithm).bytes(sig.as_bytes()).0;
    Ok((algorithm, blob))
}

/// Logs in (`ssh-userauth`): with the key if given, else the password.
/// `SSH.NET` starts with a `none` request to learn the methods.
pub fn authenticate(t: &mut Transport, user: &str, password: Option<&str>, key: Option<&UserKey>) -> SshResult<()> {
    t.send(&Writer::new(MSG_SERVICE_REQUEST).string("ssh-userauth").0)?;
    if t.recv()?.first() != Some(&MSG_SERVICE_ACCEPT) {
        return err("The server refused the authentication service.");
    }
    t.send(&Writer::new(MSG_USERAUTH_REQUEST).string(user).string("ssh-connection").string("none").0)?;
    let methods = match auth_reply(t)? {
        None => return Ok(()),
        Some(m) => m,
    };
    if let Some(key) = key.filter(|_| methods.iter().any(|m| m == "publickey")) {
        let blob = key.public_key().to_bytes().map_err(|e| SshError(e.to_string()))?;
        let algorithm = match key.algorithm() {
            ssh_key::Algorithm::Rsa { .. } => "rsa-sha2-512".to_owned(),
            a => a.as_str().to_owned(),
        };
        let to_sign = Writer::default()
            .bytes(&t.session_id)
            .byte(MSG_USERAUTH_REQUEST)
            .string(user)
            .string("ssh-connection")
            .string("publickey")
            .bool(true)
            .string(&algorithm)
            .bytes(&blob)
            .0;
        let (_, signature) = sign_user_auth(key, &to_sign)?;
        t.send(
            &Writer::new(MSG_USERAUTH_REQUEST)
                .string(user)
                .string("ssh-connection")
                .string("publickey")
                .bool(true)
                .string(&algorithm)
                .bytes(&blob)
                .bytes(&signature)
                .0,
        )?;
        return match auth_reply(t)? {
            None => Ok(()),
            Some(_) => err("Permission denied (publickey)."),
        };
    }
    if let Some(password) = password.filter(|_| methods.iter().any(|m| m == "password")) {
        t.send(&Writer::new(MSG_USERAUTH_REQUEST).string(user).string("ssh-connection").string("password").bool(false).string(password).0)?;
        return match auth_reply(t)? {
            None => Ok(()),
            Some(_) => err("Permission denied (password)."),
        };
    }
    err(format!("Permission denied ({}).", methods.join(",")))
}

/// `None` on success, the methods that can continue on failure.
fn auth_reply(t: &mut Transport) -> SshResult<Option<Vec<String>>> {
    loop {
        let p = t.recv()?;
        match p.first().copied() {
            Some(MSG_USERAUTH_SUCCESS) => return Ok(None),
            Some(MSG_USERAUTH_FAILURE) => return Ok(Some(Reader::new(&p[1..]).name_list()?)),
            Some(MSG_USERAUTH_BANNER) => continue,
            _ => return err("Unexpected message during authentication."),
        }
    }
}

/// A session channel (client side) running a subsystem.
pub struct Channel {
    pub transport: Transport,
    local_id: u32,
    remote_id: u32,
    remote_window: u64,
    remote_max_packet: u32,
    local_window: u64,
    inbox: Vec<u8>,
    closed: bool,
}

const LOCAL_WINDOW: u32 = 2 * 1024 * 1024;
const LOCAL_MAX_PACKET: u32 = 64 * 1024;

impl Channel {
    /// Opens a session channel and starts `subsystem` on it.
    pub fn open_subsystem(mut transport: Transport, subsystem: &str) -> SshResult<Self> {
        transport.send(&Writer::new(MSG_CHANNEL_OPEN).string("session").u32(0).u32(LOCAL_WINDOW).u32(LOCAL_MAX_PACKET).0)?;
        let p = transport.recv()?;
        if p.first() != Some(&MSG_CHANNEL_OPEN_CONFIRMATION) {
            return err("The server refused to open a session channel.");
        }
        let mut r = Reader::new(&p[1..]);
        r.u32()?;
        let remote_id = r.u32()?;
        let remote_window = r.u32()? as u64;
        let remote_max_packet = r.u32()?;
        let mut c = Channel {
            transport,
            local_id: 0,
            remote_id,
            remote_window,
            remote_max_packet,
            local_window: LOCAL_WINDOW as u64,
            inbox: Vec::new(),
            closed: false,
        };
        c.transport.send(&Writer::new(MSG_CHANNEL_REQUEST).u32(remote_id).string("subsystem").bool(true).string(subsystem).0)?;
        loop {
            let p = c.transport.recv()?;
            match p.first().copied() {
                Some(MSG_CHANNEL_SUCCESS) => return Ok(c),
                Some(MSG_CHANNEL_FAILURE) => return err(format!("The server refused the {subsystem} subsystem.")),
                _ => c.handle(&p)?,
            }
        }
    }

    fn handle(&mut self, p: &[u8]) -> SshResult<()> {
        let mut r = Reader::new(&p[1..]);
        match p.first().copied() {
            Some(MSG_CHANNEL_WINDOW_ADJUST) => {
                r.u32()?;
                self.remote_window += r.u32()? as u64;
            }
            Some(MSG_CHANNEL_DATA) => {
                r.u32()?;
                let data = r.bytes()?;
                self.local_window = self.local_window.saturating_sub(data.len() as u64);
                self.inbox.extend_from_slice(data);
                if self.local_window < (LOCAL_WINDOW / 2) as u64 {
                    let add = LOCAL_WINDOW as u64 - self.local_window;
                    self.transport.send(&Writer::new(MSG_CHANNEL_WINDOW_ADJUST).u32(self.remote_id).u32(add as u32).0)?;
                    self.local_window += add;
                }
            }
            Some(MSG_CHANNEL_EXTENDED_DATA) => {}
            Some(MSG_CHANNEL_EOF) | Some(MSG_CHANNEL_CLOSE) => self.closed = true,
            Some(MSG_CHANNEL_REQUEST) => {
                r.u32()?;
                r.string()?;
                if r.bool()? {
                    self.transport.send(&Writer::new(MSG_CHANNEL_FAILURE).u32(self.remote_id).0)?;
                }
            }
            _ => {}
        }
        let _ = self.local_id;
        Ok(())
    }

    /// The server's maximum packet size for this channel.
    pub fn remote_max_packet(&self) -> u32 {
        self.remote_max_packet
    }

    /// Sends data, waiting for window space.
    pub fn write(&mut self, mut data: &[u8]) -> SshResult<()> {
        while !data.is_empty() {
            while self.remote_window == 0 {
                let p = self.transport.recv()?;
                self.handle(&p)?;
                if self.closed {
                    return err("The channel was closed.");
                }
            }
            let n = data.len().min(self.remote_window as usize).min(self.remote_max_packet.saturating_sub(64).max(1) as usize);
            self.transport.send(&Writer::new(MSG_CHANNEL_DATA).u32(self.remote_id).bytes(&data[..n]).0)?;
            self.remote_window -= n as u64;
            data = &data[n..];
        }
        Ok(())
    }

    /// Reads exactly `n` bytes of channel data.
    pub fn read_exact(&mut self, n: usize) -> SshResult<Vec<u8>> {
        while self.inbox.len() < n {
            if self.closed {
                return err("The channel was closed.");
            }
            let p = self.transport.recv()?;
            self.handle(&p)?;
        }
        Ok(self.inbox.drain(..n).collect())
    }

    /// Closes the channel and the connection.
    pub fn close(mut self) {
        let _ = self.transport.send(&Writer::new(MSG_CHANNEL_CLOSE).u32(self.remote_id).0);
        self.transport.disconnect();
    }
}
