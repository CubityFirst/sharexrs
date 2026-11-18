//! `ArraySortHelper<T>.IntrospectiveSort`: the unstable sort behind .NET's
//! `Array.Sort` and `List<T>.Sort` with a comparer, ported so equal keys end
//! up where .NET leaves them (moved here from `sharex-tools`, which re-exports
//! it, so crates without the image tools can use it).

use std::cmp::Ordering;

/// `ArraySortHelper<T>.IntrospectiveSort(keys, comparer)` (what `Array.Sort`
/// and `List<T>.Sort` run with a comparer).
pub fn intro_sort<T: Clone>(keys: &mut [T], cmp: &mut dyn FnMut(&T, &T) -> Ordering) {
    if keys.len() > 1 {
        let depth = 2 * ((keys.len() as u32).ilog2() as i32 + 1);
        intro_sort_inner(keys, depth, cmp);
    }
}

fn swap_if_greater<T>(keys: &mut [T], cmp: &mut dyn FnMut(&T, &T) -> Ordering, i: usize, j: usize) {
    if cmp(&keys[i], &keys[j]) == Ordering::Greater {
        keys.swap(i, j);
    }
}

fn intro_sort_inner<T: Clone>(keys: &mut [T], mut depth: i32, cmp: &mut dyn FnMut(&T, &T) -> Ordering) {
    let mut size = keys.len();
    while size > 1 {
        if size <= 16 {
            if size == 2 {
                swap_if_greater(keys, cmp, 0, 1);
                return;
            }
            if size == 3 {
                swap_if_greater(keys, cmp, 0, 1);
                swap_if_greater(keys, cmp, 0, 2);
                swap_if_greater(keys, cmp, 1, 2);
                return;
            }
            insertion_sort(&mut keys[..size], cmp);
            return;
        }
        if depth == 0 {
            heap_sort(&mut keys[..size], cmp);
            return;
        }
        depth -= 1;
        let p = pick_pivot_and_partition(&mut keys[..size], cmp);
        intro_sort_inner(&mut keys[p + 1..size], depth, cmp);
        size = p;
    }
}

fn pick_pivot_and_partition<T: Clone>(keys: &mut [T], cmp: &mut dyn FnMut(&T, &T) -> Ordering) -> usize {
    let hi = keys.len() - 1;
    let middle = hi >> 1;
    swap_if_greater(keys, cmp, 0, middle);
    swap_if_greater(keys, cmp, 0, hi);
    swap_if_greater(keys, cmp, middle, hi);
    let pivot = keys[middle].clone();
    keys.swap(middle, hi - 1);
    let (mut left, mut right) = (0usize, hi - 1);
    while left < right {
        loop {
            left += 1;
            if cmp(&keys[left], &pivot) != Ordering::Less {
                break;
            }
        }
        loop {
            right -= 1;
            if cmp(&pivot, &keys[right]) != Ordering::Less {
                break;
            }
        }
        if left >= right {
            break;
        }
        keys.swap(left, right);
    }
    if left != hi - 1 {
        keys.swap(left, hi - 1);
    }
    left
}

fn heap_sort<T: Clone>(keys: &mut [T], cmp: &mut dyn FnMut(&T, &T) -> Ordering) {
    let n = keys.len();
    let mut i = n >> 1;
    while i >= 1 {
        down_heap(keys, i, n, cmp);
        i -= 1;
    }
    let mut i = n;
    while i > 1 {
        keys.swap(0, i - 1);
        down_heap(keys, 1, i - 1, cmp);
        i -= 1;
    }
}

fn down_heap<T: Clone>(keys: &mut [T], mut i: usize, n: usize, cmp: &mut dyn FnMut(&T, &T) -> Ordering) {
    let d = keys[i - 1].clone();
    while i <= n >> 1 {
        let mut child = 2 * i;
        if child < n && cmp(&keys[child - 1], &keys[child]) == Ordering::Less {
            child += 1;
        }
        if cmp(&d, &keys[child - 1]) != Ordering::Less {
            break;
        }
        keys[i - 1] = keys[child - 1].clone();
        i = child;
    }
    keys[i - 1] = d;
}

fn insertion_sort<T: Clone>(keys: &mut [T], cmp: &mut dyn FnMut(&T, &T) -> Ordering) {
    for i in 0..keys.len() - 1 {
        let t = keys[i + 1].clone();
        let mut j = i as isize;
        while j >= 0 && cmp(&t, &keys[j as usize]) == Ordering::Less {
            keys[j as usize + 1] = keys[j as usize].clone();
            j -= 1;
        }
        keys[(j + 1) as usize] = t;
    }
}
