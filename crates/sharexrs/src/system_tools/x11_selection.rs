//! The X11 `CLIPBOARD` selection as Avalonia.X11 reads it
//! (`ClipboardReadSessionFactory`, `SelectionReadSession.SendFormatRequest`
//! / `SendDataRequest`): a window of our own asks the owner to convert the
//! selection into a property, waiting at most `SelectionHelper.Timeout`
//! (5 s) for each answer; `INCR` transfers are read chunk by chunk.
//!
//! Built on Linux, and in tests everywhere so it's type-checked on every
//! platform (x11rb is pure Rust); only Linux opens a connection.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::time::{Duration, Instant};

use sharex_tools::system::clipboard_formats::Selection;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, Property, Window, WindowClass};
use x11rb::rust_connection::RustConnection;

/// `SelectionHelper.Timeout`.
const TIMEOUT: Duration = Duration::from_secs(5);

pub struct X11Selection {
    conn: RustConnection,
    window: Window,
    clipboard: Atom,
    property: Atom,
    incr: Atom,
}

impl X11Selection {
    /// A connection to `$DISPLAY` and a window to receive the answers;
    /// none without an X server or a selection owner.
    pub fn open() -> Option<Self> {
        let (conn, screen) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let window = conn.generate_id().ok()?;
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .ok()?;
        let mut sel = Self { conn, window, clipboard: 0, property: 0, incr: 0 };
        sel.clipboard = sel.atom("CLIPBOARD")?;
        sel.property = sel.atom("SHAREXRS_SELECTION")?;
        sel.incr = sel.atom("INCR")?;
        // `TryGetDataAsync`: no owner, no data.
        let owner = sel.conn.get_selection_owner(sel.clipboard).ok()?.reply().ok()?.owner;
        (owner != x11rb::NONE).then_some(sel)
    }

    fn atom(&self, name: &str) -> Option<Atom> {
        Some(self.conn.intern_atom(false, name.as_bytes()).ok()?.reply().ok()?.atom)
    }

    fn atom_name(&self, atom: Atom) -> Option<String> {
        let r = self.conn.get_atom_name(atom).ok()?.reply().ok()?;
        Some(String::from_utf8_lossy(&r.name).into_owned())
    }

    /// The next event matching `want` before `deadline`.
    fn wait(&self, deadline: Instant, want: impl Fn(&Event) -> bool) -> Option<Event> {
        loop {
            match self.conn.poll_for_event() {
                Ok(Some(e)) if want(&e) => return Some(e),
                Ok(Some(_)) => continue,
                Ok(None) => {}
                Err(_) => return None,
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// `XConvertSelection` into our property: the answer's type and bytes.
    fn request(&self, target: Atom) -> Option<(Atom, Vec<u8>)> {
        self.conn.convert_selection(self.window, self.clipboard, target, self.property, x11rb::CURRENT_TIME).ok()?;
        self.conn.flush().ok()?;
        let deadline = Instant::now() + TIMEOUT;
        let window = self.window;
        let Event::SelectionNotify(n) = self.wait(deadline, |e| matches!(e, Event::SelectionNotify(n) if n.requestor == window))? else { return None };
        if n.property == x11rb::NONE {
            return None;
        }
        let r = self.conn.get_property(true, self.window, self.property, AtomEnum::ANY, 0, u32::MAX).ok()?.reply().ok()?;
        if r.type_ != self.incr {
            return Some((r.type_, r.value));
        }
        // `INCR`: chunks until an empty one.
        let mut data = Vec::new();
        let mut ty = x11rb::NONE;
        loop {
            let deadline = Instant::now() + TIMEOUT;
            let property = self.property;
            self.wait(deadline, |e| matches!(e, Event::PropertyNotify(p) if p.window == window && p.atom == property && p.state == Property::NEW_VALUE))?;
            let r = self.conn.get_property(true, self.window, self.property, AtomEnum::ANY, 0, u32::MAX).ok()?.reply().ok()?;
            self.conn.flush().ok()?;
            if r.value.is_empty() {
                return Some((ty, data));
            }
            ty = r.type_;
            data.extend_from_slice(&r.value);
        }
    }
}

impl Selection for X11Selection {
    fn targets(&self) -> Vec<String> {
        let Some(targets) = self.atom("TARGETS") else { return Vec::new() };
        let Some((_, bytes)) = self.request(targets) else { return Vec::new() };
        bytes
            .chunks_exact(4)
            .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .filter(|&a| a != x11rb::NONE)
            .filter_map(|a| self.atom_name(a))
            .collect()
    }

    fn convert(&self, target: &str) -> Option<(String, Vec<u8>)> {
        let (ty, bytes) = self.request(self.atom(target)?)?;
        Some((self.atom_name(ty).unwrap_or_default(), bytes))
    }

    fn exists(&self, path: &str) -> bool {
        let p = std::path::Path::new(path);
        p.is_dir() || p.is_file()
    }
}

impl Drop for X11Selection {
    fn drop(&mut self) {
        let _ = self.conn.destroy_window(self.window);
        let _ = self.conn.flush();
    }
}
