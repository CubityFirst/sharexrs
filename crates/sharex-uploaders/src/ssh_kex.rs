//! SSH key exchange as SSH.NET does it (its `ConnectionInfo` list, in its
//! order): `curve25519-sha256`, `ecdh-sha2-nistp256/384/521`,
//! `diffie-hellman-group-exchange-sha256` (2048/4096/8192 bits),
//! `diffie-hellman-group16-sha512`, `-group18-sha512`, `-group14-sha256`,
//! `diffie-hellman-group-exchange-sha1`, `-group14-sha1`, `-group1-sha1`;
//! both ends (the client for SFTP uploads, the server for the mock). SSH.NET
//! also offers `mlkem768x25519-sha256` and `sntrup761x25519-sha512` first;
//! a server that has those has `curve25519-sha256` too.

use num_bigint::BigUint;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};

use crate::ssh::{Reader, SshError, SshResult, Writer};

/// The key exchange methods this end offers, in SSH.NET's order.
pub const KEX_ALGORITHMS: &[&str] = &[
    "curve25519-sha256",
    "curve25519-sha256@libssh.org",
    "ecdh-sha2-nistp256",
    "ecdh-sha2-nistp384",
    "ecdh-sha2-nistp521",
    "diffie-hellman-group-exchange-sha256",
    "diffie-hellman-group16-sha512",
    "diffie-hellman-group18-sha512",
    "diffie-hellman-group14-sha256",
    "diffie-hellman-group-exchange-sha1",
    "diffie-hellman-group14-sha1",
    "diffie-hellman-group1-sha1",
];

pub const MSG_KEXDH_INIT: u8 = 30;
pub const MSG_KEXDH_REPLY: u8 = 31;
pub const MSG_KEX_DH_GEX_GROUP: u8 = 31;
pub const MSG_KEX_DH_GEX_INIT: u8 = 32;
pub const MSG_KEX_DH_GEX_REPLY: u8 = 33;
pub const MSG_KEX_DH_GEX_REQUEST: u8 = 34;

/// SSH.NET's group exchange request: minimum, preferred, maximum bits.
pub const GEX_SIZES: (u32, u32, u32) = (2048, 4096, 8192);

/// The exchange hash (and key derivation) of a method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KexHash {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl KexHash {
    pub fn of(kex: &str) -> Self {
        match kex {
            "ecdh-sha2-nistp384" => KexHash::Sha384,
            "ecdh-sha2-nistp521" | "diffie-hellman-group16-sha512" | "diffie-hellman-group18-sha512" => KexHash::Sha512,
            "diffie-hellman-group-exchange-sha1" | "diffie-hellman-group14-sha1" | "diffie-hellman-group1-sha1" => KexHash::Sha1,
            _ => KexHash::Sha256,
        }
    }

    pub fn digest(self, parts: &[&[u8]]) -> Vec<u8> {
        fn run<D: Digest>(parts: &[&[u8]]) -> Vec<u8> {
            let mut d = D::new();
            for p in parts {
                d.update(p);
            }
            d.finalize().to_vec()
        }
        match self {
            KexHash::Sha1 => run::<Sha1>(parts),
            KexHash::Sha256 => run::<Sha256>(parts),
            KexHash::Sha384 => run::<Sha384>(parts),
            KexHash::Sha512 => run::<Sha512>(parts),
        }
    }

    fn bits(self) -> u64 {
        match self {
            KexHash::Sha1 => 160,
            KexHash::Sha256 => 256,
            KexHash::Sha384 => 384,
            KexHash::Sha512 => 512,
        }
    }
}

/// The 1024-bit MODP group's prime (RFC 2409, RFC 3526).
const MODP_1024: &str = concat!(
    "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74020BBEA63B139B22514A08798E3404DD",
    "EF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7ED",
    "EE386BFB5A899FA5AE9F24117C4B1FE649286651ECE65381FFFFFFFFFFFFFFFF",
);

/// The 2048-bit MODP group's prime (RFC 2409, RFC 3526).
const MODP_2048: &str = concat!(
    "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74020BBEA63B139B22514A08798E3404DD",
    "EF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7ED",
    "EE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3DC2007CB8A163BF0598DA48361C55D39A69163FA8FD24CF5F",
    "83655D23DCA3AD961C62F356208552BB9ED529077096966D670C354E4ABC9804F1746C08CA18217C32905E462E36CE3B",
    "E39E772C180E86039B2783A2EC07A28FB5C55DF06F4C52C9DE2BCBF6955817183995497CEA956AE515D2261898FA0510",
    "15728E5A8AACAA68FFFFFFFFFFFFFFFF",
);

/// The 4096-bit MODP group's prime (RFC 2409, RFC 3526).
const MODP_4096: &str = concat!(
    "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74020BBEA63B139B22514A08798E3404DD",
    "EF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7ED",
    "EE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3DC2007CB8A163BF0598DA48361C55D39A69163FA8FD24CF5F",
    "83655D23DCA3AD961C62F356208552BB9ED529077096966D670C354E4ABC9804F1746C08CA18217C32905E462E36CE3B",
    "E39E772C180E86039B2783A2EC07A28FB5C55DF06F4C52C9DE2BCBF6955817183995497CEA956AE515D2261898FA0510",
    "15728E5A8AAAC42DAD33170D04507A33A85521ABDF1CBA64ECFB850458DBEF0A8AEA71575D060C7DB3970F85A6E1E4C7",
    "ABF5AE8CDB0933D71E8C94E04A25619DCEE3D2261AD2EE6BF12FFA06D98A0864D87602733EC86A64521F2B18177B200C",
    "BBE117577A615D6C770988C0BAD946E208E24FA074E5AB3143DB5BFCE0FD108E4B82D120A92108011A723C12A787E6D7",
    "88719A10BDBA5B2699C327186AF4E23C1A946834B6150BDA2583E9CA2AD44CE8DBBBC2DB04DE8EF92E8EFC141FBECAA6",
    "287C59474E6BC05D99B2964FA090C3A2233BA186515BE7ED1F612970CEE2D7AFB81BDD762170481CD0069127D5B05AA9",
    "93B4EA988D8FDDC186FFB7DC90A6C08F4DF435C934063199FFFFFFFFFFFFFFFF",
);

/// The 8192-bit MODP group's prime (RFC 2409, RFC 3526).
const MODP_8192: &str = concat!(
    "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74020BBEA63B139B22514A08798E3404DD",
    "EF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7ED",
    "EE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3DC2007CB8A163BF0598DA48361C55D39A69163FA8FD24CF5F",
    "83655D23DCA3AD961C62F356208552BB9ED529077096966D670C354E4ABC9804F1746C08CA18217C32905E462E36CE3B",
    "E39E772C180E86039B2783A2EC07A28FB5C55DF06F4C52C9DE2BCBF6955817183995497CEA956AE515D2261898FA0510",
    "15728E5A8AAAC42DAD33170D04507A33A85521ABDF1CBA64ECFB850458DBEF0A8AEA71575D060C7DB3970F85A6E1E4C7",
    "ABF5AE8CDB0933D71E8C94E04A25619DCEE3D2261AD2EE6BF12FFA06D98A0864D87602733EC86A64521F2B18177B200C",
    "BBE117577A615D6C770988C0BAD946E208E24FA074E5AB3143DB5BFCE0FD108E4B82D120A92108011A723C12A787E6D7",
    "88719A10BDBA5B2699C327186AF4E23C1A946834B6150BDA2583E9CA2AD44CE8DBBBC2DB04DE8EF92E8EFC141FBECAA6",
    "287C59474E6BC05D99B2964FA090C3A2233BA186515BE7ED1F612970CEE2D7AFB81BDD762170481CD0069127D5B05AA9",
    "93B4EA988D8FDDC186FFB7DC90A6C08F4DF435C93402849236C3FAB4D27C7026C1D4DCB2602646DEC9751E763DBA37BD",
    "F8FF9406AD9E530EE5DB382F413001AEB06A53ED9027D831179727B0865A8918DA3EDBEBCF9B14ED44CE6CBACED4BB1B",
    "DB7F1447E6CC254B332051512BD7AF426FB8F401378CD2BF5983CA01C64B92ECF032EA15D1721D03F482D7CE6E74FEF6",
    "D55E702F46980C82B5A84031900B1C9E59E7C97FBEC7E8F323A97A7E36CC88BE0F1D45B7FF585AC54BD407B22B4154AA",
    "CC8F6D7EBF48E1D814CC5ED20F8037E0A79715EEF29BE32806A1D58BB7C5DA76F550AA3D8A1FBFF0EB19CCB1A313D55C",
    "DA56C9EC2EF29632387FE8D76E3C0468043E8F663F4860EE12BF2D5B0B7474D6E694F91E6DBE115974A3926F12FEE5E4",
    "38777CB6A932DF8CD8BEC4D073B931BA3BC832B68D9DD300741FA7BF8AFC47ED2576F6936BA424663AAB639C5AE4F568",
    "3423B4742BF1C978238F16CBE39D652DE3FDB8BEFC848AD922222E04A4037C0713EB57A81A23F0C73473FC646CEA306B",
    "4BCBC8862F8385DDFA9D4B7FA2C087E879683303ED5BDD3A062B3CF5B3A278A66D2A13F83F44F82DDF310EE074AB6A36",
    "4597E899A0255DC164F31CC50846851DF9AB48195DED7EA1B1D510BD7EE74D73FAF36BC31ECFA268359046F4EB879F92",
    "4009438B481C6CD7889A002ED5EE382BC9190DA6FC026E479558E4475677E9AA9E3050E2765694DFC81F56E880B96E71",
    "60C980DD98EDD3DFFFFFFFFFFFFFFFFF",
);

fn prime(hex: &str) -> BigUint {
    BigUint::parse_bytes(hex.as_bytes(), 16).expect("MODP prime")
}

/// The fixed group of a `diffie-hellman-groupN` method.
fn fixed_group(kex: &str) -> Option<BigUint> {
    Some(prime(match kex {
        "diffie-hellman-group1-sha1" => MODP_1024,
        "diffie-hellman-group14-sha1" | "diffie-hellman-group14-sha256" => MODP_2048,
        "diffie-hellman-group16-sha512" => MODP_4096,
        "diffie-hellman-group18-sha512" => MODP_8192,
        _ => return None,
    }))
}

/// The group a server gives for a group exchange request: the smallest of
/// 2048, 4096 and 8192 bits at least the preferred size (within bounds).
fn exchange_group(min: u32, n: u32, max: u32) -> BigUint {
    let sizes = [(2048u32, MODP_2048), (4096, MODP_4096), (8192, MODP_8192)];
    let pick = sizes.iter().find(|(s, _)| *s >= n && *s >= min && *s <= max).or_else(|| sizes.iter().rev().find(|(s, _)| *s <= max)).unwrap_or(&sizes[0]);
    prime(pick.1)
}

fn random_exponent(bits: u64) -> BigUint {
    let mut b = vec![0u8; bits.div_ceil(8) as usize];
    getrandom::fill(&mut b).expect("OS random numbers");
    b[0] |= 0x80;
    BigUint::from_bytes_be(&b)
}

fn mpint(n: &BigUint) -> Vec<u8> {
    n.to_bytes_be()
}

fn bad(what: &str) -> SshError {
    SshError(format!("Bad {what} in the key exchange."))
}

/// What a key exchange gives: the shared secret (unsigned big-endian), the
/// exchange hash, the server's host key blob and its signature.
pub struct KexResult {
    pub k: Vec<u8>,
    pub h: Vec<u8>,
    pub host_key: Vec<u8>,
    pub signature: Vec<u8>,
}

/// The versions and KEXINIT payloads both ends hash.
pub struct KexContext<'a> {
    pub client_version: &'a str,
    pub server_version: &'a str,
    pub i_c: &'a [u8],
    pub i_s: &'a [u8],
}

impl KexContext<'_> {
    fn prefix(&self, k_s: &[u8]) -> Writer {
        Writer::default().string(self.client_version).string(self.server_version).bytes(self.i_c).bytes(self.i_s).bytes(k_s)
    }
}

/// An elliptic curve's ephemeral key pair and agreement.
enum Ecdh {
    P256(p256::ecdh::EphemeralSecret),
    P384(p384::ecdh::EphemeralSecret),
    P521(p521::ecdh::EphemeralSecret),
}

impl Ecdh {
    fn new(kex: &str) -> Self {
        let mut rng = rand_core::OsRng;
        match kex {
            "ecdh-sha2-nistp384" => Ecdh::P384(p384::ecdh::EphemeralSecret::random(&mut rng)),
            "ecdh-sha2-nistp521" => Ecdh::P521(p521::ecdh::EphemeralSecret::random(&mut rng)),
            _ => Ecdh::P256(p256::ecdh::EphemeralSecret::random(&mut rng)),
        }
    }

    /// The public point, uncompressed (SEC1).
    fn public(&self) -> Vec<u8> {
        use elliptic_curve::sec1::ToEncodedPoint;
        match self {
            Ecdh::P256(s) => s.public_key().to_encoded_point(false).as_bytes().to_vec(),
            Ecdh::P384(s) => s.public_key().to_encoded_point(false).as_bytes().to_vec(),
            Ecdh::P521(s) => s.public_key().to_encoded_point(false).as_bytes().to_vec(),
        }
    }

    /// The shared secret's x coordinate.
    fn agree(&self, other: &[u8]) -> SshResult<Vec<u8>> {
        match self {
            Ecdh::P256(s) => {
                let p = p256::PublicKey::from_sec1_bytes(other).map_err(|_| bad("public point"))?;
                Ok(s.diffie_hellman(&p).raw_secret_bytes().to_vec())
            }
            Ecdh::P384(s) => {
                let p = p384::PublicKey::from_sec1_bytes(other).map_err(|_| bad("public point"))?;
                Ok(s.diffie_hellman(&p).raw_secret_bytes().to_vec())
            }
            Ecdh::P521(s) => {
                let p = p521::PublicKey::from_sec1_bytes(other).map_err(|_| bad("public point"))?;
                Ok(s.diffie_hellman(&p).raw_secret_bytes().to_vec())
            }
        }
    }
}

/// Sends and receives packets for a key exchange.
pub trait KexIo {
    fn send(&mut self, payload: &[u8]) -> SshResult<()>;
    fn recv(&mut self) -> SshResult<Vec<u8>>;
}

fn expect(p: Vec<u8>, msg: u8, name: &str) -> SshResult<Vec<u8>> {
    if p.first() != Some(&msg) {
        return Err(SshError(format!("Expected {name}.")));
    }
    Ok(p)
}

/// The client's side of `kex`.
pub fn client(io: &mut dyn KexIo, kex: &str, ctx: &KexContext) -> SshResult<KexResult> {
    let hash = KexHash::of(kex);
    if kex.starts_with("curve25519") {
        let secret = x25519_dalek::EphemeralSecret::random_from_rng(rand_core::OsRng);
        let q_c = x25519_dalek::PublicKey::from(&secret);
        io.send(&Writer::new(MSG_KEXDH_INIT).bytes(q_c.as_bytes()).0)?;
        let reply = expect(io.recv()?, MSG_KEXDH_REPLY, "KEX_ECDH_REPLY")?;
        let mut r = Reader::new(&reply[1..]);
        let k_s = r.bytes()?.to_vec();
        let q_s: [u8; 32] = r.bytes()?.try_into().map_err(|_| SshError("Bad server key exchange value.".into()))?;
        let signature = r.bytes()?.to_vec();
        let k = secret.diffie_hellman(&x25519_dalek::PublicKey::from(q_s)).to_bytes().to_vec();
        let h = hash.digest(&[&ctx.prefix(&k_s).bytes(q_c.as_bytes()).bytes(&q_s).mpint(&k).0]);
        return Ok(KexResult { k, h, host_key: k_s, signature });
    }
    if kex.starts_with("ecdh-sha2-") {
        let secret = Ecdh::new(kex);
        let q_c = secret.public();
        io.send(&Writer::new(MSG_KEXDH_INIT).bytes(&q_c).0)?;
        let reply = expect(io.recv()?, MSG_KEXDH_REPLY, "KEX_ECDH_REPLY")?;
        let mut r = Reader::new(&reply[1..]);
        let k_s = r.bytes()?.to_vec();
        let q_s = r.bytes()?.to_vec();
        let signature = r.bytes()?.to_vec();
        let k = secret.agree(&q_s)?;
        let h = hash.digest(&[&ctx.prefix(&k_s).bytes(&q_c).bytes(&q_s).mpint(&k).0]);
        return Ok(KexResult { k, h, host_key: k_s, signature });
    }
    let x = random_exponent(2 * hash.bits());
    let two = BigUint::from(2u32);
    if let Some(p) = fixed_group(kex) {
        let e = two.modpow(&x, &p);
        io.send(&Writer::new(MSG_KEXDH_INIT).mpint(&mpint(&e)).0)?;
        let reply = expect(io.recv()?, MSG_KEXDH_REPLY, "KEXDH_REPLY")?;
        let mut r = Reader::new(&reply[1..]);
        let k_s = r.bytes()?.to_vec();
        let f = BigUint::from_bytes_be(r.bytes()?);
        let signature = r.bytes()?.to_vec();
        let k = mpint(&f.modpow(&x, &p));
        let h = hash.digest(&[&ctx.prefix(&k_s).mpint(&mpint(&e)).mpint(&mpint(&f)).mpint(&k).0]);
        return Ok(KexResult { k, h, host_key: k_s, signature });
    }
    // Group exchange.
    let (min, n, max) = GEX_SIZES;
    io.send(&Writer::new(MSG_KEX_DH_GEX_REQUEST).u32(min).u32(n).u32(max).0)?;
    let group = expect(io.recv()?, MSG_KEX_DH_GEX_GROUP, "KEX_DH_GEX_GROUP")?;
    let mut r = Reader::new(&group[1..]);
    let p = BigUint::from_bytes_be(r.bytes()?);
    let g = BigUint::from_bytes_be(r.bytes()?);
    let e = g.modpow(&x, &p);
    io.send(&Writer::new(MSG_KEX_DH_GEX_INIT).mpint(&mpint(&e)).0)?;
    let reply = expect(io.recv()?, MSG_KEX_DH_GEX_REPLY, "KEX_DH_GEX_REPLY")?;
    let mut r = Reader::new(&reply[1..]);
    let k_s = r.bytes()?.to_vec();
    let f = BigUint::from_bytes_be(r.bytes()?);
    let signature = r.bytes()?.to_vec();
    let k = mpint(&f.modpow(&x, &p));
    let h = hash.digest(&[&ctx
        .prefix(&k_s)
        .u32(min)
        .u32(n)
        .u32(max)
        .mpint(&mpint(&p))
        .mpint(&mpint(&g))
        .mpint(&mpint(&e))
        .mpint(&mpint(&f))
        .mpint(&k)
        .0]);
    Ok(KexResult { k, h, host_key: k_s, signature })
}

/// The server's side of `kex` (the mock server): `k_s` is its host key
/// blob, `sign` signs the exchange hash.
pub fn server(io: &mut dyn KexIo, kex: &str, ctx: &KexContext, k_s: &[u8], sign: &dyn Fn(&[u8]) -> SshResult<Vec<u8>>) -> SshResult<(Vec<u8>, Vec<u8>)> {
    let hash = KexHash::of(kex);
    if kex.starts_with("curve25519") {
        let init = expect(io.recv()?, MSG_KEXDH_INIT, "KEX_ECDH_INIT")?;
        let q_c: [u8; 32] = Reader::new(&init[1..]).bytes()?.try_into().map_err(|_| SshError("Bad client key exchange value.".into()))?;
        let secret = x25519_dalek::EphemeralSecret::random_from_rng(rand_core::OsRng);
        let q_s = x25519_dalek::PublicKey::from(&secret);
        let k = secret.diffie_hellman(&x25519_dalek::PublicKey::from(q_c)).to_bytes().to_vec();
        let h = hash.digest(&[&ctx.prefix(k_s).bytes(&q_c).bytes(q_s.as_bytes()).mpint(&k).0]);
        io.send(&Writer::new(MSG_KEXDH_REPLY).bytes(k_s).bytes(q_s.as_bytes()).bytes(&sign(&h)?).0)?;
        return Ok((k, h));
    }
    if kex.starts_with("ecdh-sha2-") {
        let init = expect(io.recv()?, MSG_KEXDH_INIT, "KEX_ECDH_INIT")?;
        let q_c = Reader::new(&init[1..]).bytes()?.to_vec();
        let secret = Ecdh::new(kex);
        let q_s = secret.public();
        let k = secret.agree(&q_c)?;
        let h = hash.digest(&[&ctx.prefix(k_s).bytes(&q_c).bytes(&q_s).mpint(&k).0]);
        io.send(&Writer::new(MSG_KEXDH_REPLY).bytes(k_s).bytes(&q_s).bytes(&sign(&h)?).0)?;
        return Ok((k, h));
    }
    let y = random_exponent(2 * hash.bits());
    let two = BigUint::from(2u32);
    if let Some(p) = fixed_group(kex) {
        let init = expect(io.recv()?, MSG_KEXDH_INIT, "KEXDH_INIT")?;
        let e = BigUint::from_bytes_be(Reader::new(&init[1..]).bytes()?);
        let f = two.modpow(&y, &p);
        let k = mpint(&e.modpow(&y, &p));
        let h = hash.digest(&[&ctx.prefix(k_s).mpint(&mpint(&e)).mpint(&mpint(&f)).mpint(&k).0]);
        io.send(&Writer::new(MSG_KEXDH_REPLY).bytes(k_s).mpint(&mpint(&f)).bytes(&sign(&h)?).0)?;
        return Ok((k, h));
    }
    let request = expect(io.recv()?, MSG_KEX_DH_GEX_REQUEST, "KEX_DH_GEX_REQUEST")?;
    let mut r = Reader::new(&request[1..]);
    let (min, n, max) = (r.u32()?, r.u32()?, r.u32()?);
    let p = exchange_group(min, n, max);
    io.send(&Writer::new(MSG_KEX_DH_GEX_GROUP).mpint(&mpint(&p)).mpint(&mpint(&two)).0)?;
    let init = expect(io.recv()?, MSG_KEX_DH_GEX_INIT, "KEX_DH_GEX_INIT")?;
    let e = BigUint::from_bytes_be(Reader::new(&init[1..]).bytes()?);
    let f = two.modpow(&y, &p);
    let k = mpint(&e.modpow(&y, &p));
    let h = hash.digest(&[&ctx
        .prefix(k_s)
        .u32(min)
        .u32(n)
        .u32(max)
        .mpint(&mpint(&p))
        .mpint(&mpint(&two))
        .mpint(&mpint(&e))
        .mpint(&mpint(&f))
        .mpint(&k)
        .0]);
    io.send(&Writer::new(MSG_KEX_DH_GEX_REPLY).bytes(k_s).mpint(&mpint(&f)).bytes(&sign(&h)?).0)?;
    Ok((k, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modp_primes_are_rfc_3526s() {
        // The RFC's prime is 2^n - 2^(n-64) - 1 + 2^64 * (floor(2^(n-130) pi) + k);
        // the ends are the same for every group.
        for (hex, bits) in [(MODP_1024, 1024), (MODP_2048, 2048), (MODP_4096, 4096), (MODP_8192, 8192)] {
            let p = prime(hex);
            assert_eq!(p.bits(), bits);
            assert!(hex.starts_with("FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD1"));
            assert!(hex.ends_with("FFFFFFFFFFFFFFFF"));
        }
        assert!(MODP_2048.ends_with("15728E5A8AACAA68FFFFFFFFFFFFFFFF"));
        assert!(MODP_4096.ends_with("4DF435C934063199FFFFFFFFFFFFFFFF"));
        assert!(MODP_8192.ends_with("60C980DD98EDD3DFFFFFFFFFFFFFFFFF"));
    }

    #[test]
    fn hashes_by_method() {
        assert_eq!(KexHash::of("diffie-hellman-group14-sha1"), KexHash::Sha1);
        assert_eq!(KexHash::of("ecdh-sha2-nistp521"), KexHash::Sha512);
        assert_eq!(KexHash::of("curve25519-sha256"), KexHash::Sha256);
        assert_eq!(KexHash::Sha384.digest(&[b"abc"]).len(), 48);
    }
}
