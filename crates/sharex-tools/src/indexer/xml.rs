//! `IndexerXml`: `XmlWriter` with `Indent = true`, UTF-8 without a BOM.

use super::{FolderInfo, IndexerSettings, scan, size_string};
use crate::dotnet::NEW_LINE;

/// `XmlWriter`'s escaping of attribute values.
fn attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\n' => o.push_str("&#xA;"),
            '\r' => o.push_str("&#xD;"),
            '\t' => o.push_str("&#x9;"),
            c => o.push(c),
        }
    }
    o
}

/// `XmlWriter`'s escaping of element text.
fn text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            c => o.push(c),
        }
    }
    o
}

/// A tiny indenting writer with `XmlWriter`'s output rules: a start tag stays
/// open for attributes; an element without content closes as ` />`.
struct Writer {
    out: String,
    stack: Vec<(String, bool)>,
    open: bool,
}

impl Writer {
    fn indent(&mut self, depth: usize) {
        self.out.push_str(NEW_LINE);
        for _ in 0..depth {
            self.out.push_str("  ");
        }
    }

    fn close_start(&mut self) {
        if self.open {
            self.out.push('>');
            self.open = false;
        }
    }

    fn start(&mut self, name: &str) {
        self.close_start();
        if let Some(parent) = self.stack.last_mut() {
            parent.1 = true;
        }
        let depth = self.stack.len();
        self.indent(depth);
        self.out.push('<');
        self.out.push_str(name);
        self.stack.push((name.to_string(), false));
        self.open = true;
    }

    fn attribute(&mut self, name: &str, value: &str) {
        self.out.push_str(&format!(" {name}=\"{}\"", attr(value)));
    }

    fn element_string(&mut self, name: &str, value: &str) {
        self.start(name);
        self.close_start();
        self.out.push_str(&text(value));
        self.out.push_str(&format!("</{name}>"));
        self.stack.pop();
    }

    fn end(&mut self) {
        let (name, has_children) = self.stack.pop().expect("element");
        if self.open {
            self.out.push_str(" />");
            self.open = false;
        } else {
            if has_children {
                let depth = self.stack.len();
                self.indent(depth);
            }
            self.out.push_str(&format!("</{name}>"));
        }
    }
}

fn index_folder(w: &mut Writer, dir: &FolderInfo, s: &IndexerSettings) {
    w.start("Folder");
    if s.use_attribute {
        w.attribute("Name", dir.folder_name());
        if s.show_size_info && !dir.is_empty() {
            w.attribute("Size", &size_string(dir.size, s.binary_units));
        }
    } else {
        w.element_string("Name", dir.folder_name());
        if s.show_size_info && !dir.is_empty() {
            w.element_string("Size", &size_string(dir.size, s.binary_units));
        }
    }
    if !dir.files.is_empty() {
        w.start("Files");
        for f in &dir.files {
            w.start("File");
            if s.use_attribute {
                w.attribute("Name", &f.name);
                if s.show_size_info {
                    w.attribute("Size", &size_string(f.length, s.binary_units));
                }
            } else {
                w.element_string("Name", &f.name);
                if s.show_size_info {
                    w.element_string("Size", &size_string(f.length, s.binary_units));
                }
            }
            w.end();
        }
        w.end();
    }
    if !dir.folders.is_empty() {
        w.start("Folders");
        for sub in &dir.folders {
            index_folder(w, sub, s);
        }
        w.end();
    }
    w.end();
}

pub(super) fn index(folder_path: &str, s: &IndexerSettings) -> String {
    let info = scan(folder_path, s);
    let mut w = Writer { out: "<?xml version=\"1.0\" encoding=\"utf-8\"?>".into(), stack: Vec::new(), open: false };
    index_folder(&mut w, &info, s);
    w.out
}
