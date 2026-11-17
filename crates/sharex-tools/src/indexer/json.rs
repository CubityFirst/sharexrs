//! `IndexerJson`: Newtonsoft's `JsonTextWriter` with `Formatting.Indented`.

use super::{FolderInfo, IndexerSettings, scan, size_string};
use crate::dotnet::NEW_LINE;

/// `JavaScriptUtils.WriteEscapedJavaScriptString` with the default escaping.
pub fn json_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\u{85}' || c == '\u{2028}' || c == '\u{2029}' => {
                o.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// An indenting JSON writer with `JsonTextWriter`'s layout.
struct Writer {
    out: String,
    /// Per open container: whether it has a value yet.
    stack: Vec<bool>,
    after_property: bool,
}

impl Writer {
    fn newline(&mut self) {
        self.out.push_str(NEW_LINE);
        for _ in 0..self.stack.len() {
            self.out.push_str("  ");
        }
    }

    /// Before a value or property in the current container.
    fn before_item(&mut self) {
        if self.after_property {
            self.after_property = false;
            return;
        }
        if let Some(has) = self.stack.last_mut() {
            if *has {
                self.out.push(',');
            }
            *has = true;
            self.newline();
        }
    }

    fn property(&mut self, name: &str) {
        self.before_item();
        self.out.push_str(&json_string(name));
        self.out.push_str(": ");
        self.after_property = true;
    }

    fn value(&mut self, v: &str) {
        self.before_item();
        self.out.push_str(&json_string(v));
    }

    fn start(&mut self, c: char) {
        self.before_item();
        self.out.push(c);
        self.stack.push(false);
    }

    fn end(&mut self, c: char) {
        let had = self.stack.pop().expect("container");
        if had {
            self.newline();
        }
        self.out.push(c);
    }
}

fn index_folder(w: &mut Writer, dir: &FolderInfo, s: &IndexerSettings) {
    if s.create_parseable_json {
        w.property("Name");
        w.value(dir.folder_name());
        if s.show_size_info {
            w.property("Size");
            w.value(&size_string(dir.size, s.binary_units));
        }
        if !dir.folders.is_empty() {
            w.property("Folders");
            w.start('[');
            for sub in &dir.folders {
                w.start('{');
                index_folder(w, sub, s);
                w.end('}');
            }
            w.end(']');
        }
        if !dir.files.is_empty() {
            w.property("Files");
            w.start('[');
            for f in &dir.files {
                w.start('{');
                w.property("Name");
                w.value(&f.name);
                if s.show_size_info {
                    w.property("Size");
                    w.value(&size_string(f.length, s.binary_units));
                }
                w.end('}');
            }
            w.end(']');
        }
    } else {
        w.property(dir.folder_name());
        w.start('[');
        for sub in &dir.folders {
            w.start('{');
            index_folder(w, sub, s);
            w.end('}');
        }
        for f in &dir.files {
            w.value(&f.name);
        }
        w.end(']');
    }
}

pub(super) fn index(folder_path: &str, s: &IndexerSettings) -> String {
    let info = scan(folder_path, s);
    let mut w = Writer { out: String::new(), stack: Vec::new(), after_property: false };
    w.start('{');
    index_folder(&mut w, &info, s);
    w.end('}');
    w.out
}
