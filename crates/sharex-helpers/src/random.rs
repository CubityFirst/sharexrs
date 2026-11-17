//! `ShareX.HelpersLib/Random/RandomCrypto.cs` and the character sets of
//! `Helpers.cs`.

pub const NUMBERS: &str = "0123456789";
pub const ALPHABET_CAPITAL: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz";
/// `Helpers.Alphanumeric`: numbers, capitals, then lower case.
pub const ALPHANUMERIC: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
/// `Helpers.AlphanumericInverse`: numbers, lower case, then capitals.
pub const ALPHANUMERIC_INVERSE: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const HEXADECIMAL: &str = "0123456789ABCDEF";
/// `Helpers.Base56`: Base58 without `1` and `o`.
pub const BASE56: &str = "23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz";

fn next_u32() -> u32 {
    let mut b = [0u8; 4];
    // The OS's cryptographic generator, as `RandomNumberGenerator`; it only
    // fails if the OS can't provide randomness at all.
    getrandom::fill(&mut b).expect("the OS random number generator");
    u32::from_le_bytes(b)
}

/// `RandomCrypto.Next(minValue, maxValue)`: a uniform integer in
/// `min..=max` (both inclusive), without modulo bias.
pub fn next(min: i64, max: i64) -> i64 {
    let max = max + 1;
    if min >= max {
        return min;
    }
    let diff = (max - min) as u64;
    let range = 1u64 << 32;
    let remainder = range % diff;
    loop {
        let r = next_u32() as u64;
        if r < range - remainder {
            return min + (r % diff) as i64;
        }
    }
}

/// `Helpers.GetRandomChar`: any of `chars`.
pub fn char_from(chars: &str) -> char {
    let chars: Vec<char> = chars.chars().collect();
    chars[next(0, chars.len() as i64 - 1) as usize]
}

/// `RandomCrypto.Pick`.
pub fn pick<'a, T>(items: &'a [T]) -> Option<&'a T> {
    if items.is_empty() {
        return None;
    }
    items.get(next(0, items.len() as i64 - 1) as usize)
}

/// `Helpers.GetRandomString`.
pub fn string(chars: &str, length: usize) -> String {
    (0..length).map(|_| char_from(chars)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_stays_in_its_inclusive_range() {
        for _ in 0..1000 {
            let n = next(3, 5);
            assert!((3..=5).contains(&n));
        }
        assert_eq!(next(7, 7), 7);
    }
}
