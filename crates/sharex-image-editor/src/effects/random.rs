//! `System.Random` as the editor's effects use it.
//!
//! - [`shared`] is `Random.Shared`: per thread, a Xoshiro256** generator
//!   (`Random.XoshiroImpl` with .NET's `Next`/`NextSingle`/`NextDouble`,
//!   Lemire's bounded integers), seeded from the OS. ShareX never seeds it;
//!   for validation, [`seed_shared`] sets the thread's state to
//!   splitmix64 of a seed, as `tools/editor-harness --effects` does to
//!   ShareX's (`Random.ThreadSafeRandom.t_random`), so random effects can be
//!   compared pixel for pixel.
//! - [`Compat`] is `new Random(seed)` (`Net5CompatSeedImpl`, Knuth's
//!   subtractive generator).

use std::cell::RefCell;

/// `Random.XoshiroImpl` (Xoshiro256**).
#[derive(Clone, Debug)]
pub struct Xoshiro {
    s: [u64; 4],
}

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Xoshiro {
    /// The state the harness sets: splitmix64 of `seed`, four times.
    pub fn from_seed(seed: u64) -> Self {
        let mut x = seed;
        let s = [
            splitmix(&mut x),
            splitmix(&mut x),
            splitmix(&mut x),
            splitmix(&mut x),
        ];
        Xoshiro { s }
    }

    /// Seeded from the process's random hasher keys (the OS), as .NET seeds
    /// `Random.Shared` from `Interop.GetRandomBytes`.
    pub fn from_os() -> Self {
        use std::hash::{BuildHasher, Hasher};
        let mut s = [0u64; 4];
        for (i, v) in s.iter_mut().enumerate() {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_usize(i);
            *v = h.finish();
        }
        if s == [0; 4] {
            s[0] = 1;
        }
        Xoshiro { s }
    }

    pub fn next_u64(&mut self) -> u64 {
        let [mut s0, mut s1, mut s2, mut s3] = self.s;
        let result = s1.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s1 << 17;
        s2 ^= s0;
        s3 ^= s1;
        s1 ^= s2;
        s0 ^= s3;
        s2 ^= t;
        s3 = s3.rotate_left(45);
        self.s = [s0, s1, s2, s3];
        result
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Lemire's nearly divisionless bounded integer (`Random.NextUInt32`).
    fn bounded(&mut self, max: u32) -> u32 {
        let mut product = max as u64 * self.next_u32() as u64;
        let mut low = product as u32;
        if low < max {
            let remainder = 0u32.wrapping_sub(max) % max;
            while low < remainder {
                product = max as u64 * self.next_u32() as u64;
                low = product as u32;
            }
        }
        (product >> 32) as u32
    }

    /// `Next()`: `0..int.MaxValue`.
    pub fn next(&mut self) -> i32 {
        loop {
            let r = self.next_u64() >> 33;
            if r != i32::MAX as u64 {
                return r as i32;
            }
        }
    }

    /// `Next(maxValue)`: `0..max`.
    pub fn next_max(&mut self, max: i32) -> i32 {
        self.bounded(max as u32) as i32
    }

    /// `Next(minValue, maxValue)`: `min..max`.
    pub fn next_range(&mut self, min: i32, max: i32) -> i32 {
        (self.bounded(max.wrapping_sub(min) as u32) as i32).wrapping_add(min)
    }

    /// `NextSingle()`.
    pub fn next_single(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * (1.0f32 / (1u32 << 24) as f32)
    }

    /// `NextDouble()`.
    pub fn next_double(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

thread_local! {
    static SHARED: RefCell<Option<Xoshiro>> = const { RefCell::new(None) };
}

/// Runs `f` with this thread's `Random.Shared`.
pub fn shared<R>(f: impl FnOnce(&mut Xoshiro) -> R) -> R {
    SHARED.with(|s| {
        let mut s = s.borrow_mut();
        f(s.get_or_insert_with(Xoshiro::from_os))
    })
}

/// Sets this thread's `Random.Shared` to [`Xoshiro::from_seed`] (validation).
pub fn seed_shared(seed: u64) {
    SHARED.with(|s| *s.borrow_mut() = Some(Xoshiro::from_seed(seed)));
}

/// `new Random(seed)`: `Net5CompatSeedImpl`.
pub struct Compat {
    seeds: [i32; 56],
    inext: usize,
    inextp: usize,
}

impl Compat {
    pub fn new(seed: i32) -> Self {
        let mut seeds = [0i32; 56];
        let subtraction = if seed == i32::MIN {
            i32::MAX
        } else {
            seed.abs()
        };
        let mut mj = 161_803_398 - subtraction;
        seeds[55] = mj;
        let mut mk = 1i32;
        let mut ii = 0usize;
        for _ in 1..55 {
            ii += 21;
            if ii >= 55 {
                ii -= 55;
            }
            seeds[ii] = mk;
            mk = mj.wrapping_sub(mk);
            if mk < 0 {
                mk = mk.wrapping_add(i32::MAX);
            }
            mj = seeds[ii];
        }
        for _ in 1..5 {
            for i in 1..56 {
                let mut n = i + 30;
                if n >= 55 {
                    n -= 55;
                }
                seeds[i] = seeds[i].wrapping_sub(seeds[1 + n]);
                if seeds[i] < 0 {
                    seeds[i] = seeds[i].wrapping_add(i32::MAX);
                }
            }
        }
        Compat {
            seeds,
            inext: 0,
            inextp: 21,
        }
    }

    fn internal_sample(&mut self) -> i32 {
        let mut ln = self.inext + 1;
        if ln >= 56 {
            ln = 1;
        }
        let mut lp = self.inextp + 1;
        if lp >= 56 {
            lp = 1;
        }
        let mut r = self.seeds[ln].wrapping_sub(self.seeds[lp]);
        if r == i32::MAX {
            r -= 1;
        }
        if r < 0 {
            r = r.wrapping_add(i32::MAX);
        }
        self.seeds[ln] = r;
        self.inext = ln;
        self.inextp = lp;
        r
    }

    fn sample(&mut self) -> f64 {
        self.internal_sample() as f64 * (1.0 / i32::MAX as f64)
    }

    fn large_sample(&mut self) -> f64 {
        let mut result = self.internal_sample();
        if self.internal_sample() % 2 == 0 {
            result = -result;
        }
        let mut d = result as f64;
        d += (i32::MAX - 1) as f64;
        d / (2.0 * i32::MAX as f64 - 1.0)
    }

    /// `Next()`.
    pub fn next(&mut self) -> i32 {
        self.internal_sample()
    }

    /// `Next(maxValue)`.
    pub fn next_max(&mut self, max: i32) -> i32 {
        (self.sample() * max as f64) as i32
    }

    /// `Next(minValue, maxValue)`.
    pub fn next_range(&mut self, min: i32, max: i32) -> i32 {
        let range = max as i64 - min as i64;
        if range <= i32::MAX as i64 {
            (self.sample() * range as f64) as i32 + min
        } else {
            (self.large_sample() * range as f64) as i64 as i32 + min
        }
    }

    /// `NextDouble()`.
    pub fn next_double(&mut self) -> f64 {
        self.sample()
    }

    /// `NextSingle()`: `(float)Sample()`.
    pub fn next_single(&mut self) -> f32 {
        loop {
            let f = self.sample() as f32;
            if f < 1.0 {
                return f;
            }
        }
    }
}
