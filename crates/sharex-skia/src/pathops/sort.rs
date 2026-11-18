//! `src/base/SkTSort.h`: `SkTQSort` (introsort: insertion sort up to 32
//! elements, heap sort past the depth limit), so ties come out in Skia's
//! order.

fn heap_sort_sift_up<T: Copy, C: Fn(&T, &T) -> bool>(array: &mut [T], mut root: usize, bottom: usize, less: &C) {
    let x = array[root - 1];
    let start = root;
    let mut j = root << 1;
    while j <= bottom {
        if j < bottom && less(&array[j - 1], &array[j]) {
            j += 1;
        }
        array[root - 1] = array[j - 1];
        root = j;
        j = root << 1;
    }
    j = root >> 1;
    while j >= start {
        if less(&array[j - 1], &x) {
            array[root - 1] = array[j - 1];
            root = j;
            j = root >> 1;
        } else {
            break;
        }
    }
    array[root - 1] = x;
}

fn heap_sort_sift_down<T: Copy, C: Fn(&T, &T) -> bool>(array: &mut [T], mut root: usize, bottom: usize, less: &C) {
    let x = array[root - 1];
    let mut child = root << 1;
    while child <= bottom {
        if child < bottom && less(&array[child - 1], &array[child]) {
            child += 1;
        }
        if less(&x, &array[child - 1]) {
            array[root - 1] = array[child - 1];
            root = child;
            child = root << 1;
        } else {
            break;
        }
    }
    array[root - 1] = x;
}

fn heap_sort<T: Copy, C: Fn(&T, &T) -> bool>(array: &mut [T], less: &C) {
    let count = array.len();
    let mut i = count >> 1;
    while i > 0 {
        heap_sort_sift_down(array, i, count, less);
        i -= 1;
    }
    let mut i = count - 1;
    while i > 0 {
        array.swap(0, i);
        heap_sort_sift_up(array, 1, i, less);
        i -= 1;
    }
}

fn insertion_sort<T: Copy, C: Fn(&T, &T) -> bool>(a: &mut [T], less: &C) {
    let count = a.len();
    if count == 0 {
        return;
    }
    for next in 1..count {
        if !less(&a[next], &a[next - 1]) {
            continue;
        }
        let insert = a[next];
        let mut hole = next;
        loop {
            a[hole] = a[hole - 1];
            hole -= 1;
            if !(0 < hole && less(&insert, &a[hole - 1])) {
                break;
            }
        }
        a[hole] = insert;
    }
}

fn partition<T: Copy, C: Fn(&T, &T) -> bool>(a: &mut [T], pivot: usize, less: &C) -> usize {
    let right = a.len() - 1;
    let pivot_value = a[pivot];
    a.swap(pivot, right);
    let mut new_pivot = 0;
    let mut left = 0;
    while left < right {
        if less(&a[left], &pivot_value) {
            a.swap(left, new_pivot);
            new_pivot += 1;
        }
        left += 1;
    }
    a.swap(new_pivot, right);
    new_pivot
}

fn intro_sort<T: Copy, C: Fn(&T, &T) -> bool>(mut depth: i32, mut a: &mut [T], less: &C) {
    loop {
        let count = a.len();
        if count <= 32 {
            insertion_sort(a, less);
            return;
        }
        if depth == 0 {
            heap_sort(a, less);
            return;
        }
        depth -= 1;
        let middle = (count - 1) >> 1;
        let pivot = partition(a, middle, less);
        let (lo, hi) = a.split_at_mut(pivot);
        intro_sort(depth, lo, less);
        a = &mut hi[1..];
    }
}

/// `SkNextLog2`.
fn next_log2(value: u32) -> i32 {
    32 - (value.wrapping_sub(1)).leading_zeros() as i32
}

/// `SkTQSort(begin, end, lessThan)`.
pub fn tq_sort<T: Copy, C: Fn(&T, &T) -> bool>(a: &mut [T], less: C) {
    let n = a.len();
    if n <= 1 {
        return;
    }
    let depth = 2 * next_log2((n - 1) as u32);
    intro_sort(depth, a, &less);
}

pub fn tq_sort_f64(a: &mut [f64]) {
    tq_sort(a, |x, y| x < y);
}

pub fn tq_sort_f32(a: &mut [f32]) {
    tq_sort(a, |x, y| x < y);
}
