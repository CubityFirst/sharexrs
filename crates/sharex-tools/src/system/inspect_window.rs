//! Inspect window: `InspectWindowViewModel`'s details (the property list,
//! title, subtitle, copy text, the opacity slider's conversions),
//! `InspectWindowService`'s window list rules, and `WindowStyles.ToString()`
//! as .NET formats a `[Flags]` enum. Reading a window's properties is
//! [`super::windows`]'s job (Windows only).

use crate::text::{format_dotnet, Arg};

/// `Enum.GetNames/GetValues(typeof(WindowStyles))`: .NET's own sorted
/// table (by value; duplicates in the order .NET's sort left them), which
/// `ToString` formats from.
const WINDOW_STYLES: &[(&str, u32)] = &[
    ("WS_OVERLAPPED", 0),
    ("WS_EX_RIGHTSCROLLBAR", 0),
    ("WS_EX_LEFT", 0),
    ("WS_TILED", 0),
    ("WS_EX_LTRREADING", 0),
    ("WS_EX_DLGMODALFRAME", 1),
    ("WS_EX_NOPARENTNOTIFY", 4),
    ("WS_EX_TOPMOST", 8),
    ("WS_EX_ACCEPTFILES", 16),
    ("WS_EX_TRANSPARENT", 32),
    ("WS_EX_MDICHILD", 64),
    ("WS_EX_TOOLWINDOW", 128),
    ("WS_EX_WINDOWEDGE", 256),
    ("WS_EX_PALETTEWINDOW", 392),
    ("WS_EX_CLIENTEDGE", 512),
    ("WS_EX_OVERLAPPEDWINDOW", 768),
    ("WS_EX_CONTEXTHELP", 1024),
    ("WS_EX_RIGHT", 4096),
    ("WS_EX_RTLREADING", 8192),
    ("WS_EX_LEFTSCROLLBAR", 16384),
    ("WS_EX_CONTROLPARENT", 65536),
    ("WS_MAXIMIZEBOX", 65536),
    ("WS_TABSTOP", 65536),
    ("WS_GROUP", 131072),
    ("WS_EX_STATICEDGE", 131072),
    ("WS_MINIMIZEBOX", 131072),
    ("WS_THICKFRAME", 262144),
    ("WS_EX_APPWINDOW", 262144),
    ("WS_SIZEBOX", 262144),
    ("WS_SYSMENU", 524288),
    ("WS_EX_LAYERED", 524288),
    ("WS_EX_NOINHERITLAYOUT", 1048576),
    ("WS_HSCROLL", 1048576),
    ("WS_VSCROLL", 2097152),
    ("WS_DLGFRAME", 4194304),
    ("WS_EX_LAYOUTRTL", 4194304),
    ("WS_BORDER", 8388608),
    ("WS_CAPTION", 12582912),
    ("WS_OVERLAPPEDWINDOW", 13565952),
    ("WS_TILEDWINDOW", 13565952),
    ("WS_MAXIMIZE", 16777216),
    ("WS_CLIPCHILDREN", 33554432),
    ("WS_EX_COMPOSITED", 33554432),
    ("WS_CLIPSIBLINGS", 67108864),
    ("WS_EX_NOACTIVATE", 134217728),
    ("WS_DISABLED", 134217728),
    ("WS_VISIBLE", 268435456),
    ("WS_ICONIC", 536870912),
    ("WS_MINIMIZE", 536870912),
    ("WS_CHILD", 1073741824),
    ("WS_CHILDWINDOW", 1073741824),
    ("WS_POPUP", 2147483648),
    ("WS_POPUPWINDOW", 2156396544),
];

/// `WindowStyles` values the tools use.
pub mod styles {
    pub const WS_CAPTION: u32 = 0x00C0_0000;
    pub const WS_MAXIMIZEBOX: u32 = 0x0001_0000;
    pub const WS_SYSMENU: u32 = 0x0008_0000;
    pub const WS_THICKFRAME: u32 = 0x0004_0000;
    pub const WS_EX_CLIENTEDGE: u32 = 0x0000_0200;
    pub const WS_EX_DLGMODALFRAME: u32 = 0x0000_0001;
    pub const WS_EX_STATICEDGE: u32 = 0x0002_0000;
    pub const WS_EX_TOPMOST: u32 = 0x0000_0008;
    pub const WS_EX_LAYERED: u32 = 0x0008_0000;
}

/// `((WindowStyles)value).ToString()`: the last name of an exact value,
/// else the names whose bits make up the value (taken greedily from the
/// largest, listed from the smallest, zero names skipped), else the number.
pub fn window_styles_to_string(value: u32) -> String {
    if value == 0 {
        return WINDOW_STYLES[0].0.to_string();
    }
    if let Some((name, _)) = WINDOW_STYLES.iter().rev().find(|(_, v)| *v == value) {
        return name.to_string();
    }
    let mut rest = value;
    let mut found = Vec::new();
    for (i, (_, v)) in WINDOW_STYLES.iter().enumerate().rev() {
        if *v == 0 {
            break;
        }
        if rest & v == *v {
            rest -= v;
            found.push(i);
        }
    }
    if rest != 0 {
        return value.to_string();
    }
    found.iter().rev().map(|&i| WINDOW_STYLES[i].0).collect::<Vec<_>>().join(", ")
}

/// The details pane's style text: one name per line
/// (`Style.ToString().Replace(", ", NewLine)`).
pub fn window_styles_lines(value: u32) -> String {
    window_styles_to_string(value).replace(", ", super::NEW_LINE)
}

/// A window rectangle (`System.Drawing.Rectangle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    /// `Rectangle.IsEmpty`: all four zero.
    pub fn is_empty(&self) -> bool {
        self.x == 0 && self.y == 0 && self.width == 0 && self.height == 0
    }
}

/// `FormatRectangle`: empty for an empty rectangle.
pub fn format_rectangle(r: Rect) -> String {
    if r.is_empty() {
        return String::new();
    }
    format_dotnet(
        sharex_strings::tools::InspectWindowViewModel_Rectangle_format(),
        &[Arg::I64(r.x as i64), Arg::I64(r.y as i64), Arg::I64(r.width as i64), Arg::I64(r.height as i64)],
    )
}

/// What `UpdateWindowInfo` reads from a window (each property empty, or
/// its fallback, when reading it failed: `TryGet`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WindowSnapshot {
    pub handle: isize,
    pub title: String,
    pub class_name: String,
    pub process_name: String,
    pub process_file_name: String,
    /// `ProcessId.ToString()`, or empty.
    pub process_id: String,
    pub rectangle: Rect,
    pub client_rectangle: Rect,
    /// `None` when `GetWindowLong` couldn't be read (the details show "").
    pub style: Option<u32>,
    pub ex_style: Option<u32>,
    pub top_most: bool,
    /// `Opacity` (255 unless layered).
    pub opacity: u8,
}

/// `InspectWindowProperty`.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub multiline: bool,
}

/// The selected window's header and property list.
#[derive(Debug, Clone, PartialEq)]
pub struct Details {
    pub title: String,
    pub subtitle: String,
    pub kind: String,
    pub properties: Vec<Property>,
    /// The top-most switch and opacity slider (top-level windows only).
    pub top_most: Option<bool>,
    pub opacity_percent: Option<f64>,
}

/// `UpdateWindowInfo`.
pub fn details(w: &WindowSnapshot, top_level: bool) -> Details {
    use sharex_strings::tools as s;
    let title = if w.title.trim().is_empty() { s::InspectWindowViewModel_Untitled_window().to_string() } else { w.title.clone() };
    let subtitle = if w.process_name.trim().is_empty() {
        w.class_name.clone()
    } else if w.class_name.trim().is_empty() {
        w.process_name.clone()
    } else {
        format!("{}  |  {}", w.process_name, w.class_name)
    };
    let p = |name: &str, value: String| Property { name: name.to_string(), value, multiline: false };
    let style = |v: Option<u32>| v.map(window_styles_lines).unwrap_or_default();
    let properties = vec![
        p(s::InspectWindowViewModel_Window_handle(), format!("0x{:08X}", w.handle as i64)),
        p(s::InspectWindowViewModel_Window_title(), w.title.clone()),
        p(s::InspectWindowViewModel_Class_name(), w.class_name.clone()),
        p(s::InspectWindowViewModel_Process_name(), w.process_name.clone()),
        p(s::InspectWindowViewModel_Process_file_name(), w.process_file_name.clone()),
        p(s::InspectWindowViewModel_Process_identifier(), w.process_id.clone()),
        p(s::InspectWindowViewModel_Window_rectangle(), format_rectangle(w.rectangle)),
        p(s::InspectWindowViewModel_Client_rectangle(), format_rectangle(w.client_rectangle)),
        Property { name: s::InspectWindowViewModel_Window_styles().into(), value: style(w.style), multiline: true },
        Property { name: s::InspectWindowViewModel_Extended_window_styles().into(), value: style(w.ex_style), multiline: true },
    ];
    Details {
        title,
        subtitle,
        kind: if top_level { s::InspectWindowViewModel_Window() } else { s::InspectWindowViewModel_Control() }.to_string(),
        properties,
        top_most: top_level.then_some(w.top_most),
        opacity_percent: top_level.then(|| opacity_to_percent(w.opacity)),
    }
}

/// `ClipboardText`: each property's name and value on two lines, a blank
/// line between properties.
pub fn clipboard_text(properties: &[Property]) -> String {
    let nl = super::NEW_LINE;
    properties.iter().map(|p| format!("{}{nl}{}", p.name, p.value)).collect::<Vec<_>>().join(&format!("{nl}{nl}"))
}

/// `Math.Round(opacity / 255d * 100d)`.
pub fn opacity_to_percent(opacity: u8) -> f64 {
    (opacity as f64 / 255.0 * 100.0).round_ties_even()
}

/// `OnOpacityChanged`: the slider's percentage (10-100) as the layered
/// window's alpha.
pub fn percent_to_opacity(percent: f64) -> u8 {
    (percent.clamp(10.0, 100.0) / 100.0 * 255.0).round_ties_even() as u8
}

/// A candidate for the window lists (`GetVisibleWindows`).
#[derive(Debug, Clone, PartialEq)]
pub struct WindowEntry {
    pub handle: isize,
    pub title: String,
    pub class_name: String,
    pub process_name: String,
    pub visible: bool,
    pub cloaked: bool,
    pub rectangle: Rect,
}

impl WindowEntry {
    /// `InspectWindowListItem.DisplayName`.
    pub fn display_name(&self) -> String {
        if self.process_name.trim().is_empty() {
            self.title.clone()
        } else {
            format!("{}  —  {}", self.title, self.process_name)
        }
    }
}

/// `GetVisibleWindows`' filter and order: visible, not cloaked, titled,
/// with an area, not `Progman` or a `Button`, not `ignored`; sorted by title
/// with the current culture, ignoring case (stable, as LINQ's `OrderBy`).
pub fn visible_windows(entries: Vec<WindowEntry>, ignored: isize) -> Vec<WindowEntry> {
    let mut list: Vec<WindowEntry> = entries
        .into_iter()
        .filter(|w| {
            w.handle != ignored
                && w.visible
                && !w.cloaked
                && !w.title.trim().is_empty()
                && w.rectangle.width > 0
                && w.rectangle.height > 0
                && !["Progman", "Button"].iter().any(|c| sharex_helpers::culture::ordinal_ignore_case_eq(c, &w.class_name))
        })
        .collect();
    list.sort_by(|a, b| sharex_helpers::culture::current_culture_ignore_case_cmp(&a.title, &b.title));
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_format_as_dotnet() {
        assert_eq!(window_styles_to_string(0x16CF_0000), "WS_TILEDWINDOW, WS_EX_COMPOSITED, WS_CLIPSIBLINGS, WS_VISIBLE");
        assert_eq!(window_styles_to_string(0x0003_0000), "WS_TABSTOP, WS_MINIMIZEBOX");
        assert_eq!(window_styles_to_string(0x802), "2050");
        assert_eq!(window_styles_to_string(0), "WS_OVERLAPPED");
        assert_eq!(window_styles_lines(0x0004_0100), format!("WS_EX_WINDOWEDGE{}WS_SIZEBOX", crate::system::NEW_LINE));
    }

    #[test]
    fn details_and_texts() {
        let w = WindowSnapshot {
            handle: 0x1234,
            title: String::new(),
            class_name: "Notepad".into(),
            process_name: "notepad".into(),
            rectangle: Rect { x: 1, y: 2, width: 3, height: 4 },
            style: Some(0x1000_0000),
            opacity: 128,
            ..Default::default()
        };
        let d = details(&w, true);
        assert_eq!(d.subtitle, "notepad  |  Notepad");
        assert_eq!(d.properties[0].value, "0x00001234");
        assert_eq!(d.properties[6].value, "X: 1  Y: 2  |  Width: 3  Height: 4");
        assert_eq!(d.properties[7].value, "");
        assert_eq!(d.properties[8].value, "WS_VISIBLE");
        assert_eq!(d.properties[9].value, "");
        assert_eq!(d.opacity_percent, Some(50.0));
        assert_eq!(percent_to_opacity(5.0), 26);
        assert!(clipboard_text(&d.properties[..2]).contains(&format!("0x00001234{nl}{nl}", nl = crate::system::NEW_LINE)));
        let list = visible_windows(
            vec![
                WindowEntry { handle: 1, title: "b".into(), class_name: "X".into(), process_name: String::new(), visible: true, cloaked: false, rectangle: Rect { x: 0, y: 0, width: 1, height: 1 } },
                WindowEntry { handle: 2, title: "A".into(), class_name: "progman".into(), process_name: String::new(), visible: true, cloaked: false, rectangle: Rect { x: 0, y: 0, width: 1, height: 1 } },
                WindowEntry { handle: 3, title: "a".into(), class_name: "Y".into(), process_name: "p".into(), visible: true, cloaked: false, rectangle: Rect { x: 0, y: 0, width: 1, height: 1 } },
            ],
            0,
        );
        assert_eq!(list.iter().map(|w| w.handle).collect::<Vec<_>>(), [3, 1]);
        assert_eq!(list[0].display_name(), "a  —  p");
    }
}
