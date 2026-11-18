//! The .NET library behaviour ShareX's tools depend on for their exact output:
//! `Array.Sort`/`List<T>.Sort`'s introsort (unstable, so equal keys end up
//! where .NET leaves them), `PriorityQueue<TElement, TPriority>`'s quaternary
//! heap (which of two equal priorities comes out first), `System.IO.Path`'s
//! string rules and `Environment.NewLine`.

use std::cmp::Ordering;

/// `Environment.NewLine`.
pub const NEW_LINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// `ArraySortHelper<T>.IntrospectiveSort(keys, comparer)` (what `Array.Sort`
/// and `List<T>.Sort` run with a comparer).
pub use sharex_helpers::dotnet_sort::intro_sort;

/// `System.Collections.Generic.PriorityQueue<TElement, TPriority>`: a
/// quaternary min-heap, ported so equal priorities dequeue in .NET's order.
pub struct PriorityQueue<E, P> {
    nodes: Vec<(E, P)>,
    cmp: fn(&P, &P) -> Ordering,
}

impl<E: Clone, P: Clone> PriorityQueue<E, P> {
    pub fn new(cmp: fn(&P, &P) -> Ordering) -> Self {
        PriorityQueue { nodes: Vec::new(), cmp }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn enqueue(&mut self, element: E, priority: P) {
        let index = self.nodes.len();
        self.nodes.push((element.clone(), priority.clone()));
        self.move_up((element, priority), index);
    }

    pub fn peek(&self) -> Option<&(E, P)> {
        self.nodes.first()
    }

    pub fn dequeue(&mut self) -> Option<(E, P)> {
        if self.nodes.is_empty() {
            return None;
        }
        let root = self.nodes[0].clone();
        let last = self.nodes.len() - 1;
        if last > 0 {
            let last_node = self.nodes[last].clone();
            self.nodes.truncate(last);
            self.move_down(last_node, 0);
        } else {
            self.nodes.truncate(last);
        }
        Some(root)
    }

    fn move_up(&mut self, node: (E, P), mut index: usize) {
        while index > 0 {
            let parent = (index - 1) >> 2;
            if (self.cmp)(&node.1, &self.nodes[parent].1) == Ordering::Less {
                self.nodes[index] = self.nodes[parent].clone();
                index = parent;
            } else {
                break;
            }
        }
        self.nodes[index] = node;
    }

    fn move_down(&mut self, node: (E, P), mut index: usize) {
        let size = self.nodes.len();
        loop {
            let mut i = (index << 2) + 1;
            if i >= size {
                break;
            }
            let mut min_index = i;
            let upper = (i + 4).min(size);
            i += 1;
            while i < upper {
                if (self.cmp)(&self.nodes[i].1, &self.nodes[min_index].1) == Ordering::Less {
                    min_index = i;
                }
                i += 1;
            }
            if (self.cmp)(&node.1, &self.nodes[min_index].1) != Ordering::Greater {
                break;
            }
            self.nodes[index] = self.nodes[min_index].clone();
            index = min_index;
        }
        self.nodes[index] = node;
    }
}

/// `double.CompareTo` (NaN sorts first).
pub fn cmp_f64(a: &f64, b: &f64) -> Ordering {
    a.partial_cmp(b).unwrap_or_else(|| match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        _ => Ordering::Greater,
    })
}

fn is_separator(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

fn is_volume_separator(c: char) -> bool {
    cfg!(windows) && c == ':'
}

/// `Path.GetFileName`.
pub fn file_name(path: &str) -> &str {
    let start = path.rfind(|c| is_separator(c) || is_volume_separator(c)).map(|i| i + 1).unwrap_or(0);
    &path[start..]
}

/// `Path.GetExtension` (with the dot; empty when there's none).
pub fn extension(path: &str) -> &str {
    let name = file_name(path);
    match name.rfind('.') {
        Some(i) if i != name.len() - 1 => &name[i..],
        _ => "",
    }
}

/// `Path.HasExtension`.
pub fn has_extension(path: &str) -> bool {
    !extension(path).is_empty()
}

/// `Path.GetFileNameWithoutExtension`.
pub fn file_name_without_extension(path: &str) -> &str {
    let name = file_name(path);
    match name.rfind('.') {
        Some(i) => &name[..i],
        None => name,
    }
}

/// `Path.GetDirectoryName` (None for a root or an empty path).
pub fn directory_name(path: &str) -> Option<String> {
    let i = path.rfind(|c| is_separator(c) || is_volume_separator(c))?;
    let mut dir = &path[..i];
    if dir.is_empty() || dir.ends_with(':') {
        // "C:\file" -> "C:\", "/file" -> "/"
        let root = &path[..=i];
        if root == path {
            return None;
        }
        dir = root;
    }
    Some(dir.to_string())
}

/// `Path.ChangeExtension(path, extension)`.
pub fn change_extension(path: &str, ext: &str) -> String {
    let name_start = path.len() - file_name(path).len();
    let mut base = path;
    if let Some(i) = path[name_start..].rfind('.') {
        base = &path[..name_start + i];
    }
    if ext.is_empty() {
        return format!("{base}.");
    }
    if ext.starts_with('.') { format!("{base}{ext}") } else { format!("{base}.{ext}") }
}

/// `Path.Combine(a, b)`: `b` when it's rooted, else joined with the platform's
/// separator unless `a` already ends with one.
pub fn combine(a: &str, b: &str) -> String {
    if a.is_empty() || is_rooted(b) {
        return b.to_string();
    }
    if b.is_empty() {
        return a.to_string();
    }
    let sep = if cfg!(windows) { '\\' } else { '/' };
    if a.ends_with(|c| is_separator(c) || is_volume_separator(c)) {
        format!("{a}{b}")
    } else {
        format!("{a}{sep}{b}")
    }
}

/// `Path.IsPathRooted`.
pub fn is_rooted(path: &str) -> bool {
    let mut chars = path.chars();
    match chars.next() {
        Some(c) if is_separator(c) => true,
        Some(_) if cfg!(windows) => chars.next() == Some(':'),
        _ => false,
    }
}

/// `string.Trim()` (the Unicode white space .NET trims).
pub fn trim(s: &str) -> &str {
    s.trim_matches(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_like_dotnet() {
        let mut v: Vec<i32> = (0..100).map(|i| (i * 37) % 23).collect();
        intro_sort(&mut v, &mut |a, b| a.cmp(b));
        assert!(v.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn priority_queue_orders() {
        let mut q = PriorityQueue::new(|a: &i64, b: &i64| a.cmp(b));
        for (i, p) in [5, 3, 9, 1, 3, 7].iter().enumerate() {
            q.enqueue(i, *p);
        }
        let order: Vec<i64> = std::iter::from_fn(|| q.dequeue().map(|x| x.1)).collect();
        assert_eq!(order, [1, 3, 3, 5, 7, 9]);
    }

    #[test]
    fn paths() {
        assert_eq!(file_name_without_extension("a/b.c.png"), "b.c");
        assert_eq!(change_extension("a/b_th", "jpg"), "a/b_th.jpg");
        assert_eq!(change_extension("a/b.x", "jpg"), "a/b.jpg");
        assert_eq!(extension("a.b/c"), "");
        assert_eq!(combine("a", "b"), format!("a{}b", if cfg!(windows) { '\\' } else { '/' }));
    }
}
