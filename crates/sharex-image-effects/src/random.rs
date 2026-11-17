//! `RandomFast` (HelpersLib/Random/RandomFast.cs): inclusive ranges over `System.Random`.
//!
//! ShareX seeds nothing, so any generator is faithful in normal use. For validation,
//! [`RandomFast::seeded`] reproduces `new Random(seed)` exactly (.NET's
//! `Net5CompatSeedImpl`, Knuth's subtractive generator), which the effects harness
//! swaps into ShareX's `RandomFast`, so the random effects (Slice, Torn edge,
//! Particles, default gradients) can be compared pixel for pixel.

pub struct RandomFast {
    imp: Imp,
}

enum Imp {
    Os,
    Compat {
        seeds: Box<[i32; 56]>,
        inext: usize,
        inextp: usize,
    },
}

impl Default for RandomFast {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomFast {
    /// Unseeded, like ShareX's.
    pub fn new() -> Self {
        RandomFast { imp: Imp::Os }
    }

    /// `new Random(seed)`.
    pub fn seeded(seed: i32) -> Self {
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
        RandomFast {
            imp: Imp::Compat {
                seeds: Box::new(seeds),
                inext: 0,
                inextp: 21,
            },
        }
    }

    fn sample(&mut self) -> f64 {
        match &mut self.imp {
            Imp::Os => {
                let mut b = [0u8; 8];
                getrandom::fill(&mut b).expect("the OS random number generator");
                (u64::from_le_bytes(b) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
            }
            Imp::Compat {
                seeds,
                inext,
                inextp,
            } => {
                let mut ln = *inext + 1;
                if ln >= 56 {
                    ln = 1;
                }
                let mut lp = *inextp + 1;
                if lp >= 56 {
                    lp = 1;
                }
                let mut r = seeds[ln].wrapping_sub(seeds[lp]);
                if r == i32::MAX {
                    r -= 1;
                }
                if r < 0 {
                    r = r.wrapping_add(i32::MAX);
                }
                seeds[ln] = r;
                *inext = ln;
                *inextp = lp;
                r as f64 * (1.0 / i32::MAX as f64)
            }
        }
    }

    /// `Random.Next(min, max)`: `min..max` exclusive.
    fn next_exclusive(&mut self, min: i32, max: i32) -> i32 {
        let range = max as i64 - min as i64;
        (self.sample() * range as f64) as i64 as i32 + min
    }

    /// `RandomFast.Next(min, max)`: both inclusive.
    pub fn next(&mut self, min: i32, max: i32) -> i32 {
        assert!(min <= max, "RandomFast.Next: min > max");
        if min == max {
            return min;
        }
        self.next_exclusive(min, max + 1)
    }

    /// `RandomFast.Next(max)`: `0..=max`.
    pub fn next_max(&mut self, max: i32) -> i32 {
        (self.sample() * (max + 1) as f64) as i32
    }

    /// `RandomFast.Pick`.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let i = self.next_max(items.len() as i32 - 1);
        &items[i as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_dotnet_seeded_random() {
        // .NET: new Random(42).Next(0, 100) x5, new Random(-7).Next(-50, 51) x5.
        let mut r = RandomFast::seeded(42);
        let v: Vec<i32> = (0..5).map(|_| r.next(0, 99)).collect();
        assert_eq!(v, [66, 14, 12, 52, 16]);
        let mut r = RandomFast::seeded(-7);
        let v: Vec<i32> = (0..5).map(|_| r.next(-50, 50)).collect();
        assert_eq!(v, [-12, 37, 16, -45, -13]);
    }
}
