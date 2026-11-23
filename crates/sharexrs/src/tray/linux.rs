//! Linux: a StatusNotifierItem over D-Bus (ksni, as snapr), which is what
//! Avalonia's `TrayIcon` uses there: a click is `Clicked` (a left click),
//! and the menu is the native one ShareX builds (`BuildNativeMenu`).
//! Not testable on Windows; see `docs/PORTING.md`.

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{CheckmarkItem, MenuItem, StandardItem, SubMenu};

use super::{Button, Callback, NativeEntry, TrayEvent};

struct Sni {
    callback: Callback,
    tooltip: String,
    /// The icon, ARGB32 big-endian as the protocol wants.
    icon: ksni::Icon,
    menu: Vec<NativeEntry>,
}

fn items(entries: &[NativeEntry]) -> Vec<MenuItem<Sni>> {
    entries
        .iter()
        .map(|e| {
            if e.separator {
                return MenuItem::Separator;
            }
            if let Some(children) = &e.children {
                return SubMenu { label: e.header.clone(), enabled: e.enabled, submenu: items(children), ..Default::default() }
                    .into();
            }
            let command = e.command;
            let activate = Box::new(move |t: &mut Sni| {
                if let Some(i) = command {
                    (t.callback)(TrayEvent::Command(i));
                }
            });
            if e.checked {
                CheckmarkItem { label: e.header.clone(), enabled: e.enabled, checked: true, activate, ..Default::default() }
                    .into()
            } else {
                StandardItem { label: e.header.clone(), enabled: e.enabled, activate, ..Default::default() }.into()
            }
        })
        .collect()
}

impl ksni::Tray for Sni {
    fn id(&self) -> String {
        "sharexrs".into()
    }

    fn title(&self) -> String {
        "ShareXrs".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip { title: self.tooltip.clone(), ..Default::default() }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        (self.callback)(TrayEvent::Up(Button::Left));
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        items(&self.menu)
    }
}

fn icon(image: &image::RgbaImage) -> ksni::Icon {
    ksni::Icon {
        width: image.width() as i32,
        height: image.height() as i32,
        data: image.pixels().flat_map(|p| [p[3], p[0], p[1], p[2]]).collect(),
    }
}

pub struct Tray {
    handle: Option<Handle<Sni>>,
    callback: Callback,
    tooltip: String,
    icon: ksni::Icon,
    menu: Vec<NativeEntry>,
}

impl Tray {
    pub fn new(tooltip: &str, visible: bool, callback: Callback) -> Result<Self, String> {
        let mut tray = Self {
            handle: None,
            callback,
            tooltip: tooltip.into(),
            icon: icon(&super::app_icon(Self::icon_size())),
            menu: Vec::new(),
        };
        tray.set_visible(visible);
        Ok(tray)
    }

    pub fn icon_size() -> u32 {
        32
    }

    /// Whether the item is registered, so on screen: not without a
    /// StatusNotifierWatcher (stock GNOME, a sway bar without a tray).
    pub fn is_shown(&self) -> bool {
        self.handle.is_some()
    }

    pub fn set_icon(&mut self, image: &image::RgbaImage) {
        self.icon = icon(image);
        let i = self.icon.clone();
        if let Some(h) = &self.handle {
            h.update(|t| t.icon = i);
        }
    }

    pub fn set_tooltip(&mut self, text: &str) {
        self.tooltip = text.into();
        let t2 = self.tooltip.clone();
        if let Some(h) = &self.handle {
            h.update(|t| t.tooltip = t2);
        }
    }

    /// Shown while visible: the item is registered, else taken down.
    pub fn set_visible(&mut self, visible: bool) {
        if visible && self.handle.is_none() {
            let sni = Sni {
                callback: self.callback.clone(),
                tooltip: self.tooltip.clone(),
                icon: self.icon.clone(),
                menu: self.menu.clone(),
            };
            match sni.spawn() {
                Ok(h) => self.handle = Some(h),
                Err(e) => eprintln!("tray unavailable: {e}"),
            }
        } else if !visible && let Some(h) = self.handle.take() {
            h.shutdown().wait();
        }
    }

    pub fn set_menu(&mut self, entries: Vec<NativeEntry>) {
        self.menu = entries.clone();
        if let Some(h) = &self.handle {
            h.update(|t| t.menu = entries);
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.shutdown().wait();
        }
    }
}
