//! Per-pixel work on big images split over the CPU's threads (std's scoped
//! threads). Each item is computed by the same function wherever the split
//! falls, so the result is the same as on one thread; most of the gain on a
//! screen-sized buffer is its first touch (the OS zeroing new pages) done
//! in parallel.

use std::mem::MaybeUninit;

/// Below this many items, the work stays on the calling thread.
const MIN_PARALLEL: usize = 1 << 20;

/// How many threads for `len` items: one per 256K items, at most 8.
fn threads(len: usize) -> usize {
    if len < MIN_PARALLEL {
        return 1;
    }
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    cores.min(8).min(len / (MIN_PARALLEL / 4)).max(1)
}

/// Runs `f(start, chunk)` over `data` cut into chunks of whole groups of
/// `group` items (a pixel's bytes, an image row), `start` being the
/// chunk's first index in `data`.
pub fn for_each_chunk<T: Send>(data: &mut [T], group: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    let group = group.max(1);
    let n = threads(data.len());
    if n <= 1 {
        f(0, data);
        return;
    }
    let groups = data.len().div_ceil(group);
    let per = groups.div_ceil(n) * group;
    std::thread::scope(|s| {
        let f = &f;
        let mut chunks = data.chunks_mut(per).enumerate();
        // The calling thread takes the first chunk.
        let first = chunks.next();
        for (i, chunk) in chunks {
            s.spawn(move || f(i * per, chunk));
        }
        if let Some((_, chunk)) = first {
            f(0, chunk);
        }
    });
}

/// A `len`-item vector whose items `init(start, chunk)` writes, chunk by
/// chunk as [`for_each_chunk`] cuts it (every item of each chunk must be
/// written).
pub fn collect<T: Send>(len: usize, group: usize, init: impl Fn(usize, &mut [MaybeUninit<T>]) + Sync) -> Vec<T> {
    let mut v: Vec<T> = Vec::with_capacity(len);
    let spare = &mut v.spare_capacity_mut()[..len];
    for_each_chunk(spare, group, init);
    // SAFETY: `init` wrote all `len` items (its contract), the capacity is
    // at least `len`.
    unsafe { v.set_len(len) };
    v
}

/// Writes `f(i)` for each slot of `chunk`, `i` counting from `start`.
pub fn fill<T>(start: usize, chunk: &mut [MaybeUninit<T>], f: impl Fn(usize) -> T) {
    for (i, slot) in chunk.iter_mut().enumerate() {
        slot.write(f(start + i));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_split_gives_what_one_thread_does() {
        let len = MIN_PARALLEL * 3 + 7;
        let v: Vec<u32> = collect(len, 5, |start, chunk| fill(start, chunk, |i| (i as u32).wrapping_mul(2654435761)));
        assert_eq!(v.len(), len);
        assert!(v.iter().enumerate().all(|(i, &x)| x == (i as u32).wrapping_mul(2654435761)));
        let mut w = vec![0u8; len];
        for_each_chunk(&mut w, 4, |start, chunk| {
            assert_eq!(start % 4, 0);
            for (i, b) in chunk.iter_mut().enumerate() {
                *b = ((start + i) % 251) as u8;
            }
        });
        assert!(w.iter().enumerate().all(|(i, &b)| b == (i % 251) as u8));
    }
}
