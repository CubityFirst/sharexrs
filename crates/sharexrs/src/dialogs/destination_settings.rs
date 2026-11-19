//! `ShareX.UploadersLib/Presentation/DestinationSettings`: the Destination
//! settings window. The navigation lists the destinations by category
//! (`DestinationSettingsViewModel.Categories`); each page is what
//! `DestinationSettingsPageBuilder` builds for it, bound to
//! `UploadersConfig` so every change applies at once.
//!
//! The pages' generic part (the cards of a destination's simple settings,
//! its nested settings objects and account lists) is generated from
//! ShareX's own builder by the uploaders harness
//! (`tools/uploaders-harness/dotnet/DestinationPages.cs`) into
//! `destination_pages.json`; the hand-built cards (accounts, albums,
//! folders, Amazon S3, img.fish) are written here as ShareX writes them.
//!
//! Settings are read and written through the config's JSON, as ShareX's
//! builder reads and writes members by reflection: a change sets one
//! member and the config is read back. `[JsonEncrypt]` members are shown
//! decrypted and written encrypted, as ShareX saves them.
//!
//! What needs the service (signing in, loading albums, folders and
//! devices) runs on a worker thread with a copy of the config, through
//! `sharex_uploaders::signin` and the destinations' listings; when it's
//! done, the members it changed are set and the card's status line says
//! how it went, as `DestinationSettingsAccounts` and the page builder's
//! cards do. Pages open in the browser through a [`UrlOpener`], which tests
//! replace.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, mpsc};

use egui::{CornerRadius, Id, Rect, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use serde::Deserialize;
use serde_json::{Value, json};
use sharex_strings::uploaders_lib as u;
use sharex_ui::controls::{self, Button, CheckBox, ComboBox, ComboItem, DecimalUpDown, TextBox, content_font};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};
use sharex_uploaders::UploadersConfig;
use sharex_uploaders::destinations::mega::MegaFolderInfo;
use sharex_uploaders::oauth::{OAuth2Info, OAuthInfo};
use sharex_uploaders::signin::{self, SignInService};

use super::{Cx, Dialog, Effect, Spec};
use crate::settings_form::{self, matches};
use crate::upload::UrlOpener;

/// `ColumnDefinitions="220,*"`.
const NAV_WIDTH: f32 = 220.0;
/// `Row`: `ColumnDefinitions="210,*" ColumnSpacing="8"`.
const LABEL_W: f32 = 210.0;
const LABEL_GAP: f32 = 8.0;
/// `Page`: `MaxWidth="780"`, `Margin="28,24,28,32"`.
const PAGE_MAX: f32 = 780.0;
const PAGE_MARGIN: Thickness = Thickness { left: 28.0, top: 24.0, right: 28.0, bottom: 32.0 };
/// `Border.section-card`: `Padding="12"`, 1px border, `Margin="0,0,0,8"`;
/// its `StackPanel` `Spacing="4"`.
const CARD_PAD: f32 = 12.0;
const CARD_GAP: f32 = 8.0;
const CARD_SPACING: f32 = 4.0;
/// `form-control`s and `Button.compact`: `MinHeight="32"` / `Height="32"`.
const CONTROL_H: f32 = 32.0;
/// `NumericUpDown`: Fluent's own height wins over `MinHeight="32"`.
const NUMBER_H: f32 = 40.0;
const CONTROL_PAD: Thickness = Thickness { left: 8.0, top: 4.0, right: 8.0, bottom: 4.0 };
const BUTTON_PAD: Thickness = Thickness { left: 10.0, top: 4.0, right: 10.0, bottom: 4.0 };
const TAG: &str = sharex_uploaders::dpapi::ENCRYPTED_TAG;

// ---------------------------------------------------------------------------
// The generated page spec.

#[derive(Deserialize)]
struct Category {
    id: String,
    icon: String,
    pages: Vec<PageDef>,
}

#[derive(Deserialize)]
struct PageDef {
    id: String,
    title: String,
    #[serde(default)]
    aliases: Vec<String>,
    cards: Vec<CardDef>,
}

#[derive(Deserialize)]
struct CardDef {
    special: Option<String>,
    title: Option<String>,
    #[serde(default)]
    rows: Vec<RowDef>,
    list: Option<String>,
    #[serde(default)]
    strings: bool,
    default_item: Option<Value>,
    hint: Option<String>,
}

#[derive(Deserialize)]
struct RowDef {
    label: String,
    path: Vec<String>,
    label_key: Option<String>,
    kind: String,
    #[serde(default)]
    secret: bool,
    #[serde(default)]
    encrypt: bool,
    #[serde(default)]
    values: Vec<EnumValueDef>,
    #[serde(rename = "type")]
    ty: Option<String>,
    tooltip: Option<String>,
}

#[derive(Deserialize)]
struct EnumValueDef {
    name: String,
    text: String,
    key: Option<String>,
}

fn categories() -> &'static [Category] {
    static SPEC: OnceLock<Vec<Category>> = OnceLock::new();
    SPEC.get_or_init(|| {
        let text = include_str!("destination_pages.json").trim_start_matches('\u{feff}');
        serde_json::from_str(text).expect("destination_pages.json")
    })
}

fn category_title(id: &str) -> &'static str {
    match id {
        "image-uploaders" => u::DestinationSettings_Category_Image_uploaders(),
        "text-uploaders" => u::DestinationSettings_Category_Text_uploaders(),
        "file-uploaders" => u::DestinationSettings_Category_File_uploaders(),
        _ => u::DestinationSettings_Category_URL_shorteners(),
    }
}

fn page_title(p: &PageDef) -> String {
    if p.id == "shared-folder" { u::DestinationSettings_Page_Shared_folder().to_owned() } else { p.title.clone() }
}

/// `DestinationPageDefinition.MatchesService`'s `Normalize`.
fn normalize(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// The page of the service named `service` (`MatchesService`).
fn find_service(service: &str) -> Option<(usize, usize)> {
    let name = normalize(service);
    for (c, cat) in categories().iter().enumerate() {
        for (i, p) in cat.pages.iter().enumerate() {
            if normalize(&p.title) == name || p.aliases.iter().any(|a| normalize(a) == name) {
                return Some((c, i));
            }
        }
    }
    None
}

/// `FormatLabel`: `AccessKeyID` -> "Access Key ID", or its translation.
fn format_label(name: &str) -> String {
    let chars: Vec<char> = name.replace('_', " ").chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            if prev.is_ascii_lowercase() || prev.is_ascii_digit() || (prev.is_ascii_uppercase() && next_lower) {
                out.push(' ');
            }
        }
        out.push(c);
    }
    let result = out.trim().to_owned();
    translated_label(&result)
}

/// The `DestinationSettings_Field_*` translation of a formatted label.
fn translated_label(label: &str) -> String {
    let mut key = String::from("DestinationSettings_Field_");
    let mut gap = false;
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            if gap {
                key.push('_');
            }
            gap = false;
            key.push(c);
        } else {
            gap = true;
        }
    }
    if gap {
        key.push('_');
    }
    u::lookup(&key).map(str::to_owned).unwrap_or_else(|| label.to_owned())
}

// ---------------------------------------------------------------------------
// Reading and writing the config's JSON.

#[derive(Clone, Debug, PartialEq)]
enum Seg {
    K(String),
    I(usize),
}

type Path = Vec<Seg>;

fn path(keys: &[&str]) -> Path {
    keys.iter().map(|k| Seg::K((*k).to_owned())).collect()
}

fn at<'a>(v: &'a Value, p: &[Seg]) -> &'a Value {
    static NULL: Value = Value::Null;
    let mut v = v;
    for s in p {
        v = match s {
            Seg::K(k) => v.get(k.as_str()),
            Seg::I(i) => v.get(*i),
        }
        .unwrap_or(&NULL);
    }
    v
}

fn set_at(v: &mut Value, p: &[Seg], new: Value) {
    let Some((last, parents)) = p.split_last() else {
        *v = new;
        return;
    };
    let mut v = v;
    for s in parents {
        v = match s {
            Seg::K(k) => {
                if !v.is_object() {
                    *v = json!({});
                }
                v.as_object_mut().expect("object").entry(k.clone()).or_insert(Value::Null)
            }
            Seg::I(i) => match v.get_mut(*i) {
                Some(x) => x,
                None => return,
            },
        };
    }
    match last {
        Seg::K(k) => {
            if !v.is_object() {
                *v = json!({});
            }
            v.as_object_mut().expect("object").insert(k.clone(), new);
        }
        Seg::I(i) => {
            if let Some(x) = v.get_mut(*i) {
                *x = new;
            }
        }
    }
}

fn str_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn not_blank(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty())
}

fn oauth2_connected(v: &Value) -> bool {
    let info: Option<OAuth2Info> = serde_json::from_value(v.clone()).ok();
    OAuth2Info::check(info.as_ref())
}

fn oauth1_connected(v: &Value) -> bool {
    let info: Option<OAuthInfo> = serde_json::from_value(v.clone()).ok();
    OAuthInfo::check(info.as_ref())
}

/// `BetweenOrDefault(0, count - 1)`, or -1 without items.
fn clamp_selection(value: i64, count: usize) -> i64 {
    if count == 0 {
        -1
    } else if (0..count as i64).contains(&value) {
        value
    } else {
        0
    }
}

/// `FTPAccount.ToString`: `Name (Host:Port)`.
fn ftp_account_text(a: &Value) -> String {
    format!("{} ({}:{})", str_of(&a["Name"]), str_of(&a["Host"]), str_of(&a["Port"]))
}

/// `LocalhostAccount.ToString`: `Name - LocalhostRoot:Port`.
fn localhost_account_text(a: &Value) -> String {
    format!("{} - {}:{}", str_of(&a["Name"]), str_of(&a["LocalhostRoot"]), str_of(&a["Port"]))
}

fn list_item_text(list: &str, item: &Value) -> String {
    match list {
        "FTPAccountList" => ftp_account_text(item),
        "LocalhostAccountList" => localhost_account_text(item),
        _ => str_of(item),
    }
}

/// `FTPAccount.PreviewHttpPath` (`GetUriPath("example.png")`).
fn ftp_preview(a: &Value) -> String {
    use sharex_helpers::{NameParser, NameParserType};
    use sharex_uploaders::url_helpers::{combine_url, url_encode};
    let host = str_of(&a["Host"]);
    if host.is_empty() {
        return String::new();
    }
    let mut file = "example.png".to_owned();
    if a["HttpHomePathNoExtension"].as_bool() == Some(true) {
        file = "example".into();
    }
    let file = url_encode(&file, false);
    let sub = NameParser::new(NameParserType::Url).parse(&str_of(&a["SubFolderPath"]).replace("%host", &host));
    let auto_add = a["HttpHomePathAutoAddSubFolderPath"].as_bool().unwrap_or(true);
    let scheme = match a["BrowserProtocol"].as_str().unwrap_or("http") {
        "https" => "https",
        "ftp" => "ftp",
        "ftps" => "ftps",
        "file" => "file",
        "sftp" => "sftp",
        _ => "http",
    };
    let home = str_of(&a["HttpHomePath"]).replace("%host", &host);
    let parser = sharex_uploaders::custom::CustomUploaderParser { use_name_parser: true, ..Default::default() };
    let home = parser.parse(&home).unwrap_or(home);
    if home.is_empty() {
        let mut url = host.strip_prefix("ftp.").unwrap_or(&host).to_owned();
        if auto_add {
            url = combine_url(&url, &sub);
        }
        url = combine_url(&url, &file);
        return format!("{scheme}://{url}");
    }
    let (authority, rest) = home.split_once('/').unwrap_or((&home, ""));
    let (dir, query) = match rest.rfind('?') {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (rest, ""),
    };
    let mut path = format!("/{dir}");
    let mut query = query.to_owned();
    if !query.is_empty() && query.ends_with('=') {
        query = if auto_add { sharex_uploaders::url_helpers::combine_urls(&[&query, &sub, &file]) } else { format!("{query}{file}") };
    } else {
        if auto_add {
            path = combine_url(&path, &sub);
        }
        path = combine_url(&path, &file);
    }
    let query = if query.is_empty() { String::new() } else { format!("?{query}") };
    format!("{scheme}://{authority}{path}{query}")
}

// ---------------------------------------------------------------------------
// A page's controls, built each frame from the config and laid out like
// the builder's StackPanels and Grids.

/// What an editor writes.
#[derive(Clone, Debug)]
enum Bind {
    Str { path: Path, encrypt: bool },
    Bool(Path),
    Enum { path: Path, names: Vec<String> },
    Int(Path),
    Dec(Path),
    StrList(Path),
    /// A text box of the window only (authorization codes, user names).
    Local(String),
    S3Endpoint,
    ImgurAlbum,
    PushbulletDevice,
    /// MEGA's e-mail and password: a change disconnects first.
    Mega(Path, bool),
    PhotobucketAlbumPath,
    /// A list editor's selection.
    ListSelect(String),
    PhotobucketAlbum,
    /// A remote folder card's list (page, the selected folder's member).
    FolderSelect(String, Path),
    /// MEGA's tree: a folder expanded or collapsed, selected (by ID).
    MegaToggle(String),
    MegaSelect(String),
}

enum Input {
    Text(String),
    Bool(bool),
    Index(usize),
    Num(f64),
}

/// What a button does.
#[derive(Clone, Debug)]
enum Act {
    /// Members set, then the status line's own text again.
    Set(Vec<(Path, Value)>, Option<String>),
    OpenUrl(String),
    ListAdd(String, Value),
    ListDuplicate(String),
    ListRemove(String),
    PhotobucketAdd,
    PhotobucketRemove,
    PhotobucketRefresh,
    /// A code-paste card's Open authorization page, Complete
    /// authorization, Refresh token.
    OAuthOpen(SignInService),
    OAuthComplete(SignInService),
    OAuthRefresh(SignInService),
    /// A Google card's Connect account (`OAuthListenerWindow`).
    Loopback(SignInService),
    /// Flickr's and Photobucket's Open authorization page and Complete
    /// authorization.
    OAuth1Open(SignInService),
    OAuth1Complete(SignInService),
    ImageShackLogin,
    PastebinLogin,
    /// `UserPasswordLogin`'s Log in (page, where the API key goes).
    UserPassword(String, Path),
    PushbulletRefresh,
    /// MEGA's Log in, with the two-factor code once it's asked for.
    MegaLogin(Option<String>),
    ImgurAlbums,
    /// A remote folder card's buttons (page, the selected folder's member).
    Folder(String, Path, FolderAct),
    /// `_openRemoteStorageBrowser`.
    RemoteStorageBrowser,
}

/// A remote folder card's buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FolderAct {
    Refresh,
    Open,
    Back,
    Root,
}

/// A row of MEGA's folder tree: a folder (`id`) or the disabled "…"
/// placeholder of a folder not loaded yet (`None`).
#[derive(Clone, Debug, PartialEq)]
struct TreeRow {
    id: Option<String>,
    label: String,
    depth: usize,
    expanded: bool,
    selected: bool,
}

struct Chk {
    key: String,
    text: String,
    value: bool,
    enabled: bool,
    bind: Bind,
}

struct Btn {
    key: String,
    text: String,
    enabled: bool,
    act: Act,
}

enum Ed {
    Text { key: String, value: String, password: bool, enabled: bool, bind: Bind },
    Combo { key: String, items: Vec<String>, selected: Option<usize>, enabled: bool, bind: Bind },
    Number { key: String, value: f64, min: f64, max: f64, decimals: usize, enabled: bool, bind: Bind },
    Hint { text: String, status: Option<bool> },
    With(Box<Ed>, Btn),
    Checks(Vec<Chk>),
}

enum Item {
    Row { label: String, ed: Ed, tip: Option<String>, preview: bool },
    Check(Chk, Option<String>),
    Hint(String),
    Buttons(Vec<Btn>),
    List { key: String, rows: Vec<String>, selected: Option<usize>, min_h: f32, max_h: f32, bind: Bind },
    Tree { key: String, rows: Vec<TreeRow> },
    Stack(f32, Vec<Item>),
}

struct Card {
    title: String,
    items: Vec<Item>,
}

enum Block {
    Card(Card),
    /// The remote storage browser's button row (`Margin="0,4,0,0"`).
    Action(Btn),
}

fn label_font() -> egui::FontId {
    font(13.0, 400)
}
fn title_font() -> egui::FontId {
    font(15.0, 600)
}
fn hint_font() -> egui::FontId {
    font(12.0, 400)
}
fn check_font() -> egui::FontId {
    content_font(13.0, 400)
}
fn button() -> Button {
    Button::compact(BUTTON_PAD)
}
fn check_box(c: &Chk) -> CheckBox<'_> {
    CheckBox { min_height: CONTROL_H, enabled: c.enabled, text_font: check_font(), ..CheckBox::new(&c.text) }
}
fn text_height(ui: &Ui) -> f32 {
    (1.0 + CONTROL_PAD.top + line_height(ui, &TextBox::font()) + CONTROL_PAD.bottom + 1.0).max(CONTROL_H)
}
fn combo_height(ui: &Ui) -> f32 {
    (1.0 + CONTROL_PAD.top + line_height(ui, &ComboBox::item_font()) + CONTROL_PAD.bottom + 1.0).max(CONTROL_H)
}
/// Fluent's `ListBoxItem`: `Padding="12,9,12,12"`.
fn list_row_height(ui: &Ui) -> f32 {
    9.0 + line_height(ui, &label_font()) + 12.0
}

fn ed_width(ui: &Ui, e: &Ed) -> f32 {
    match e {
        Ed::Text { value, password, .. } => {
            let shown = if *password { "\u{25CF}".repeat(value.chars().count()) } else { value.clone() };
            (text_width(ui, &shown, TextBox::font()) + CONTROL_PAD.left + CONTROL_PAD.right + 2.0).max(300.0)
        }
        Ed::Combo { items, selected, .. } => {
            let combo = ComboBox { padding: CONTROL_PAD, enabled: true };
            let item: Vec<ComboItem> = selected.and_then(|i| items.get(i)).map(ComboItem::text).into_iter().collect();
            combo.content_width(ui, &item).max(250.0)
        }
        Ed::Number { .. } => 130.0,
        Ed::Hint { text, .. } => text_width(ui, text, hint_font()),
        Ed::With(e, b) => ed_width(ui, e) + 6.0 + button().width(ui, &b.text),
        Ed::Checks(cs) => cs.iter().map(|c| check_box(c).width(ui)).sum::<f32>() + 12.0 * (cs.len().saturating_sub(1)) as f32,
    }
}

fn ed_height(ui: &Ui, e: &Ed, w: f32) -> f32 {
    match e {
        Ed::Text { .. } => text_height(ui),
        Ed::Combo { .. } => combo_height(ui),
        Ed::Number { .. } => NUMBER_H,
        Ed::Hint { text, .. } => controls::wrapped(ui, pos2(0.0, 0.0), w, text, hint_font(), egui::Color32::TRANSPARENT, false),
        Ed::With(e, b) => ed_height(ui, e, w - 6.0 - button().width(ui, &b.text)).max(CONTROL_H),
        Ed::Checks(_) => CONTROL_H + 2.0,
    }
}

fn item_width(ui: &Ui, i: &Item) -> f32 {
    match i {
        Item::Row { ed, .. } => LABEL_W + LABEL_GAP + ed_width(ui, ed),
        Item::Check(c, _) => check_box(c).width(ui),
        Item::Hint(t) => text_width(ui, t, hint_font()),
        Item::Buttons(bs) => bs.iter().map(|b| button().width(ui, &b.text)).sum::<f32>() + 6.0 * (bs.len().saturating_sub(1)) as f32,
        Item::List { rows, .. } => rows.iter().map(|r| text_width(ui, r, label_font()) + 24.0).fold(0.0, f32::max) + 2.0,
        Item::Tree { rows, .. } => {
            rows.iter().map(|r| 1.0 + 4.0 + TREE_INDENT * r.depth as f32 + 36.0 + 2.0 + text_width(ui, &r.label, label_font()) + 2.0 + 4.0 + 1.0).fold(0.0, f32::max)
        }
        Item::Stack(_, items) => items.iter().map(|i| item_width(ui, i)).fold(0.0, f32::max),
    }
}

fn item_height(ui: &Ui, i: &Item, w: f32) -> f32 {
    match i {
        Item::Row { ed, preview, .. } => {
            let h = line_height(ui, &label_font()).max(ed_height(ui, ed, w - LABEL_W - LABEL_GAP));
            if *preview { h + 12.0 } else { h }
        }
        Item::Check(..) => CONTROL_H + 2.0,
        Item::Hint(t) => controls::wrapped(ui, pos2(0.0, 0.0), w, t, hint_font(), egui::Color32::TRANSPARENT, false),
        Item::Buttons(_) => CONTROL_H,
        Item::List { rows, min_h, max_h, .. } => (list_row_height(ui) * rows.len() as f32 + 2.0).clamp(*min_h, *max_h),
        Item::Tree { rows, .. } => tree_content_height(rows.len()).clamp(180.0, 320.0),
        Item::Stack(spacing, items) => {
            let mut h = 0.0;
            for (n, i) in items.iter().enumerate() {
                if n > 0 {
                    h += spacing;
                }
                h += item_height(ui, i, w);
            }
            h
        }
    }
}

fn card_width(ui: &Ui, c: &Card) -> f32 {
    let title = text_width(ui, &c.title, title_font());
    c.items.iter().map(|i| item_width(ui, i)).fold(title, f32::max) + 2.0 + 2.0 * CARD_PAD
}

/// The texts the search looks at.
fn item_texts(i: &Item, out: &mut Vec<String>) {
    let ed_texts = |e: &Ed, out: &mut Vec<String>| {
        fn go(e: &Ed, out: &mut Vec<String>) {
            match e {
                Ed::Combo { items, .. } => out.extend(items.iter().cloned()),
                Ed::Hint { text, .. } => out.push(text.clone()),
                Ed::With(e, b) => {
                    go(e, out);
                    out.push(b.text.clone());
                }
                Ed::Checks(cs) => out.extend(cs.iter().map(|c| c.text.clone())),
                _ => {}
            }
        }
        go(e, out);
    };
    match i {
        Item::Row { label, ed, .. } => {
            out.push(label.clone());
            ed_texts(ed, out);
        }
        Item::Check(c, _) => out.push(c.text.clone()),
        Item::Hint(t) => out.push(t.clone()),
        Item::Buttons(bs) => out.extend(bs.iter().map(|b| b.text.clone())),
        Item::List { rows, .. } => out.extend(rows.iter().cloned()),
        Item::Tree { rows, .. } => out.extend(rows.iter().map(|r| r.label.clone())),
        Item::Stack(_, items) => items.iter().for_each(|i| item_texts(i, out)),
    }
}

fn card_text(c: &Card) -> String {
    let mut out = vec![c.title.clone()];
    c.items.iter().for_each(|i| item_texts(i, &mut out));
    out.join(" ")
}

/// What the controls of a frame did.
#[derive(Default)]
struct Events {
    inputs: Vec<(Bind, Input)>,
    acts: Vec<Act>,
}

struct Painter<'a> {
    ui: &'a mut Ui,
    page: String,
    ev: Events,
}

impl Painter<'_> {
    fn id(&self, key: &str) -> Id {
        Id::new(("destination-settings", self.page.as_str(), key))
    }

    fn tip(&mut self, rect: Rect, key: &str, tip: &Option<String>) {
        if let Some(t) = tip {
            let id = self.id(&format!("{key}-tip"));
            self.ui.interact(rect, id, egui::Sense::hover()).on_hover_text(t);
        }
    }

    fn check(&mut self, c: &Chk, x: f32, y: f32) {
        let cb = check_box(c);
        let w = cb.width(self.ui);
        let mut v = c.value;
        let id = self.id(&c.key);
        if cb.show(self.ui, Rect::from_min_size(pos2(x, y), vec2(w, CONTROL_H)), id, &mut v).changed() {
            self.ev.inputs.push((c.bind.clone(), Input::Bool(v)));
        }
    }

    fn button(&mut self, b: &Btn, rect: Rect) {
        let id = self.id(&b.key);
        if button().show(self.ui, rect, id, &b.text, b.enabled).clicked() {
            self.ev.acts.push(b.act.clone());
        }
    }

    fn ed(&mut self, e: &Ed, rect: Rect) {
        let p = palette(self.ui.ctx());
        match e {
            Ed::Text { key, value, password, enabled, bind } => {
                let th = text_height(self.ui);
                let r = Rect::from_min_size(pos2(rect.min.x, center_y(rect, th)), vec2(rect.width(), th));
                let tb = TextBox { padding: CONTROL_PAD, enabled: *enabled, password: *password, ..Default::default() };
                let mut v = value.clone();
                let id = self.id(key);
                tb.show(self.ui, r, id, &mut v);
                if v != *value {
                    self.ev.inputs.push((bind.clone(), Input::Text(v)));
                }
            }
            Ed::Combo { key, items, selected, enabled, bind } => {
                let ch = combo_height(self.ui);
                let w = ed_width(self.ui, e).min(rect.width());
                let r = Rect::from_min_size(pos2(rect.min.x, center_y(rect, ch)), vec2(w, ch));
                let combo = ComboBox { padding: CONTROL_PAD, enabled: *enabled };
                let ci: Vec<ComboItem> = items.iter().map(ComboItem::text).collect();
                let id = self.id(key);
                if let Some(i) = combo.show(self.ui, r, id, &ci, *selected) {
                    self.ev.inputs.push((bind.clone(), Input::Index(i)));
                }
                if ComboBox::is_open(self.ui.ctx(), id) {
                    self.ui.ctx().data_mut(|d| d.insert_temp(Id::new("settings-any-combo-next"), true));
                }
            }
            Ed::Number { key, value, min, max, decimals, enabled, bind } => {
                let r = Rect::from_min_size(pos2(rect.min.x, center_y(rect, NUMBER_H)), vec2(130.0, NUMBER_H));
                let mut v = *value;
                let id = self.id(key);
                let spinner = DecimalUpDown { minimum: *min, maximum: *max, increment: 1.0, decimals: *decimals, enabled: *enabled };
                if spinner.show(self.ui, r, id, &mut v) {
                    self.ev.inputs.push((bind.clone(), Input::Num(v)));
                }
            }
            Ed::Hint { text, status } => {
                let color = match status {
                    Some(true) => p.success,
                    Some(false) => p.error,
                    None => p.text_secondary,
                };
                controls::wrapped(self.ui, rect.min, rect.width(), text, hint_font(), color, true);
            }
            Ed::With(inner, b) => {
                let bw = button().width(self.ui, &b.text);
                self.ed(inner, Rect::from_min_max(rect.min, pos2(rect.max.x - bw - 6.0, rect.max.y)));
                self.button(b, Rect::from_min_size(pos2(rect.max.x - bw, center_y(rect, CONTROL_H)), vec2(bw, CONTROL_H)));
            }
            Ed::Checks(cs) => {
                let mut x = rect.min.x;
                for c in cs {
                    self.check(c, x, rect.min.y + 1.0);
                    x += check_box(c).width(self.ui) + 12.0;
                }
            }
        }
    }

    fn item(&mut self, i: &Item, rect: Rect) {
        let p = palette(self.ui.ctx());
        match i {
            Item::Row { label, ed, tip, preview } => {
                let r = if *preview { rect.shrink2(vec2(0.0, 6.0)) } else { rect };
                self.tip(r, label, tip);
                controls::label(self.ui, Rect::from_min_max(r.min, pos2(r.min.x + LABEL_W, r.max.y)), label, label_font(), p.text_secondary);
                self.ed(ed, Rect::from_min_max(pos2(r.min.x + LABEL_W + LABEL_GAP, r.min.y), r.max));
            }
            Item::Check(c, tip) => {
                let w = check_box(c).width(self.ui);
                self.tip(Rect::from_min_size(rect.min, vec2(w, rect.height())), &c.key, tip);
                self.check(c, rect.min.x, rect.min.y + 1.0);
            }
            Item::Hint(t) => {
                controls::wrapped(self.ui, rect.min, rect.width(), t, hint_font(), p.text_secondary, true);
            }
            Item::Buttons(bs) => {
                let mut x = rect.min.x;
                for b in bs {
                    let w = button().width(self.ui, &b.text);
                    self.button(b, Rect::from_min_size(pos2(x, rect.min.y), vec2(w, CONTROL_H)));
                    x += w + 6.0;
                }
            }
            Item::List { key, rows, selected, bind, .. } => self.list(key, rows, *selected, bind, rect),
            Item::Tree { key, rows } => self.tree(key, rows, rect),
            Item::Stack(spacing, items) => {
                let mut y = rect.min.y;
                for i in items {
                    let h = item_height(self.ui, i, rect.width());
                    self.item(i, Rect::from_min_size(pos2(rect.min.x, y), vec2(rect.width(), h)));
                    y += h + spacing;
                }
            }
        }
    }

    /// `ListBox.settings-list`: the main background, a 1px border, a row
    /// per item, the selected one on the accent.
    fn list(&mut self, key: &str, rows: &[String], selected: Option<usize>, bind: &Bind, r: Rect) {
        let p = palette(self.ui.ctx());
        let f = fluent(self.ui.ctx());
        self.ui.painter().rect_filled(r, CornerRadius::same(3), p.background_main);
        self.ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, p.border), StrokeKind::Inside);
        let row_h = list_row_height(self.ui);
        let clip = self.ui.painter().with_clip_rect(r.shrink(1.0).intersect(self.ui.clip_rect()));
        for (i, text) in rows.iter().enumerate() {
            let row = Rect::from_min_size(pos2(r.min.x + 1.0, r.min.y + 1.0 + row_h * i as f32), vec2(r.width() - 2.0, row_h));
            if row.min.y >= r.max.y {
                break;
            }
            let resp = self.ui.interact(row.intersect(r), self.id(&format!("{key}-row-{i}")), egui::Sense::click());
            let sel = selected == Some(i);
            if sel {
                sharex_ui::menu::paint_accent_gradient(self.ui, row, CornerRadius::same(3));
            } else if resp.hovered() {
                clip.rect_filled(row, CornerRadius::same(3), f.list_low);
            }
            let fg = if sel { p.accent_foreground } else { p.text_secondary };
            let g = galley(self.ui, text, label_font(), fg, row.width() - 24.0);
            clip.galley(pos2(row.min.x + 12.0, row.min.y + 9.0), g, fg);
            if resp.clicked() && !sel {
                self.ev.inputs.push((bind.clone(), Input::Index(i)));
            }
        }
    }

    /// `TreeView.settings-tree` (`Padding="4"`, `MinHeight="180"`,
    /// `MaxHeight="320"`, scrolling past it): a `TreeViewItem` per row
    /// (`Padding="2"`, `Margin="0,1"`), indented 16 a level; a folder's
    /// chevron expands it (or a double click), a click selects it; a
    /// folder not loaded yet shows the disabled "…".
    fn tree(&mut self, key: &str, rows: &[TreeRow], r: Rect) {
        let p = palette(self.ui.ctx());
        let f = fluent(self.ui.ctx());
        self.ui.painter().rect_filled(r, CornerRadius::same(3), p.background_main);
        self.ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, p.border), StrokeKind::Inside);
        let view = r.shrink(1.0);
        let scroll = super::layout::scroll_offset(self.ui, self.id(&format!("{key}-scroll")), view, tree_content_height(rows.len()) - 2.0);
        let clip = view.intersect(self.ui.clip_rect());
        let mut child = self.ui.new_child(egui::UiBuilder::new().max_rect(view));
        child.set_clip_rect(clip);
        for (i, row) in rows.iter().enumerate() {
            let rr = Rect::from_min_size(pos2(r.min.x + 5.0, r.min.y + 5.0 + 1.0 + TREE_STRIDE * i as f32 - scroll), vec2(r.width() - 10.0, CONTROL_H));
            if !rr.intersects(clip) {
                continue;
            }
            let indent = TREE_INDENT * row.depth as f32;
            let Some(id) = &row.id else {
                controls::label(&child, Rect::from_min_max(pos2(rr.min.x + indent + 36.0 + 2.0, rr.min.y), rr.max), &row.label, label_font(), f.base_medium_low);
                continue;
            };
            let resp = child.interact(rr.intersect(clip), self.id(&format!("{key}-row-{id}")), egui::Sense::click());
            settings_form::nav_item_background(child.painter(), rr, &resp, row.selected);
            let fg = if row.selected { p.accent_foreground } else { p.text };
            let chevron = Rect::from_min_size(pos2(rr.min.x + indent + 12.0, center_y(rr, 12.0)), Vec2::splat(12.0));
            crate::task_settings_window::paint_tree_chevron(&child, chevron, row.expanded, fg);
            let hit = Rect::from_min_max(pos2(rr.min.x + indent, rr.min.y), pos2(rr.min.x + indent + 36.0, rr.max.y)).intersect(clip);
            let ex = child.interact(hit, self.id(&format!("{key}-expand-{id}")), egui::Sense::click());
            controls::label(&child, Rect::from_min_max(pos2(rr.min.x + indent + 36.0 + 2.0, rr.min.y), rr.max), &row.label, label_font(), fg);
            if ex.clicked() || resp.double_clicked() {
                self.ev.inputs.push((Bind::MegaToggle(id.clone()), Input::Bool(!row.expanded)));
            } else if resp.clicked() && !row.selected {
                self.ev.inputs.push((Bind::MegaSelect(id.clone()), Input::Bool(true)));
            }
        }
        super::layout::scroll_bar(self.ui, view, tree_content_height(rows.len()) - 2.0, scroll, 16.0);
    }
}

/// A tree row's stride (`Margin="0,1"` around 32) and a level's indent.
const TREE_STRIDE: f32 = 1.0 + CONTROL_H + 1.0;
const TREE_INDENT: f32 = 16.0;

/// The tree's height for `rows` rows (its border and `Padding="4"`).
fn tree_content_height(rows: usize) -> f32 {
    1.0 + 4.0 + rows as f32 * TREE_STRIDE + 4.0 + 1.0
}

// ---------------------------------------------------------------------------
// The window.

/// What a finished job does on the window's thread: its changes to the
/// window, and the members it sets.
type Apply = Box<dyn FnOnce(&mut DestinationSettingsWindow, &Value) -> Vec<(Path, Value)> + Send>;

/// A button's work on a worker thread (ShareX awaits it on the UI thread).
struct Job {
    key: String,
    rx: mpsc::Receiver<Apply>,
}

/// A remote folder card's browser (`BuildRemoteFolderCard`'s locals):
/// the folder shown, the ones Open folder went through, its sub folders.
struct FolderBrowser {
    current: Value,
    history: Vec<Value>,
    items: Vec<Value>,
    selected: Option<usize>,
    /// Which load the list shows (a later one wins).
    generation: u64,
}

impl FolderBrowser {
    fn new(root: Value) -> Self {
        Self { current: root, history: Vec::new(), items: Vec::new(), selected: None, generation: 0 }
    }
}

/// MEGA's folder tree (`BuildMegaFolderTreeCard`'s items): the folders
/// loaded under each folder (by ID, the root's is empty), which are
/// expanded, which are loading.
#[derive(Default)]
struct MegaTree {
    children: HashMap<String, Vec<MegaFolderInfo>>,
    expanded: std::collections::HashSet<String>,
    loading: std::collections::HashSet<String>,
}

impl MegaTree {
    /// The rows shown, the folder `selected_id` selected (the root when
    /// it's blank).
    fn rows(&self, selected_id: &str) -> Vec<TreeRow> {
        let mut out = Vec::new();
        self.push(&MegaFolderInfo::root(), 0, selected_id, &mut out);
        out
    }

    fn push(&self, folder: &MegaFolderInfo, depth: usize, selected_id: &str, out: &mut Vec<TreeRow>) {
        let id = folder.id.clone().unwrap_or_default().trim().to_owned();
        let label = folder.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| u::DestinationSettings_Root_folder().into());
        let expanded = self.expanded.contains(&id);
        out.push(TreeRow { id: Some(id.clone()), label, depth, expanded, selected: id == selected_id });
        if !expanded {
            return;
        }
        // Loading: its items cleared; loaded: its folders; else "…".
        match self.children.get(&id) {
            Some(children) => children.iter().for_each(|c| self.push(c, depth + 1, selected_id, out)),
            None if self.loading.contains(&id) => {}
            None => out.push(TreeRow { id: None, label: "…".into(), depth: depth + 1, expanded: false, selected: false }),
        }
    }

    fn find(&self, id: &str) -> Option<MegaFolderInfo> {
        if id.is_empty() {
            return Some(MegaFolderInfo::root());
        }
        self.children.values().flatten().find(|f| f.id.as_deref() == Some(id)).cloned()
    }
}

pub struct DestinationSettingsWindow {
    /// The selected page (category, page).
    page: (usize, usize),
    expanded: Vec<bool>,
    search: String,
    nav_scroll: f32,
    nav_shown: Option<(usize, usize)>,
    scroll: HashMap<(usize, usize), f32>,
    /// Text boxes of the window only, by key.
    local: HashMap<String, String>,
    /// Status lines set by buttons, by key.
    status: HashMap<String, (String, Option<bool>)>,
    /// List editors' selections, by list.
    list_selected: HashMap<String, Option<usize>>,
    /// Decrypted `[JsonEncrypt]` values, by what's stored.
    plain: HashMap<String, String>,
    /// The page to show once the window is open (`NavigateToService`).
    service: Option<String>,
    normalized: bool,
    effects: Vec<Effect>,
    /// Opens authorization pages and the cards' links (the browser).
    opener: Arc<dyn UrlOpener>,
    jobs: Vec<Job>,
    /// MEGA's two-factor code being asked (`InputBoxWindowIntegration.Show`).
    two_factor: Option<mpsc::Receiver<Option<String>>>,
    /// The remote folder cards' browsers, by page.
    browsers: HashMap<String, FolderBrowser>,
    mega: MegaTree,
    /// Buttons pressed for the window (MEGA's Log in once the code's given).
    queued: Vec<Act>,
}

impl DestinationSettingsWindow {
    /// `new DestinationSettingsWindow(config)`, on the page of `service`
    /// when one is given (`NavigateToService`).
    pub fn new(service: Option<&str>) -> Self {
        let mut w = Self {
            page: (0, 0),
            expanded: vec![true; categories().len()],
            search: String::new(),
            nav_scroll: 0.0,
            nav_shown: None,
            scroll: HashMap::new(),
            local: HashMap::new(),
            status: HashMap::new(),
            list_selected: HashMap::new(),
            plain: HashMap::new(),
            service: service.map(str::to_owned),
            normalized: false,
            effects: Vec::new(),
            opener: Arc::new(crate::upload::SystemBrowser),
            jobs: Vec::new(),
            two_factor: None,
            browsers: HashMap::new(),
            mega: MegaTree::default(),
            queued: Vec::new(),
        };
        if let Some(s) = service {
            w.navigate_to_service(s);
        }
        w
    }

    /// The same window opening pages through `opener` (tests record them).
    #[cfg(test)]
    pub fn with_opener(mut self, opener: Arc<dyn UrlOpener>) -> Self {
        self.opener = opener;
        self
    }

    /// Whether a button's work is still running.
    pub fn busy(&self) -> bool {
        !self.jobs.is_empty() || self.two_factor.is_some()
    }

    /// Runs `work` on a worker thread; what it returns is applied on the
    /// window's thread when it's done. A button whose work is running does
    /// nothing until it's done.
    fn spawn(&mut self, key: &str, work: impl FnOnce() -> Apply + Send + 'static) {
        if self.jobs.iter().any(|j| j.key == key) {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("destination-settings".into()).spawn(move || {
            let _ = tx.send(work());
        });
        if let Err(e) = spawned {
            eprintln!("Destination settings: {e}");
            return;
        }
        self.jobs.push(Job { key: key.to_owned(), rx });
    }

    /// Applies the jobs that are done; returns the members they set.
    fn poll_jobs(&mut self, j: &Value) -> Vec<(Path, Value)> {
        let mut sets = Vec::new();
        let mut i = 0;
        while i < self.jobs.len() {
            match self.jobs[i].rx.try_recv() {
                Ok(apply) => {
                    self.jobs.remove(i);
                    sets.extend(apply(self, j));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.jobs.remove(i);
                }
                Err(mpsc::TryRecvError::Empty) => i += 1,
            }
        }
        if let Some(rx) = &self.two_factor {
            match rx.try_recv() {
                Ok(code) => {
                    self.two_factor = None;
                    // `if (code == null) return;`
                    if let Some(code) = code {
                        self.queued.push(Act::MegaLogin(Some(code)));
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => self.two_factor = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        sets
    }

    /// Shows the page with this id (`NavigateTo`), for `--render`.
    pub fn select_page(&mut self, id: &str) -> bool {
        for (c, cat) in categories().iter().enumerate() {
            if let Some(i) = cat.pages.iter().position(|p| p.id == id) {
                self.page = (c, i);
                return true;
            }
        }
        false
    }

    fn navigate_to_service(&mut self, service: &str) {
        if let Some(p) = find_service(service) {
            self.page = p;
        }
    }

    /// A `[JsonEncrypt]` value as the text box shows it.
    fn shown(&mut self, v: &Value) -> String {
        let s = str_of(v);
        match s.strip_prefix(TAG) {
            Some(enc) if !enc.is_empty() => {
                if let Some(p) = self.plain.get(&s) {
                    return p.clone();
                }
                let p = sharex_uploaders::dpapi::decrypt(enc).unwrap_or_default();
                self.plain.insert(s, p.clone());
                p
            }
            _ => s,
        }
    }

    /// A `[JsonEncrypt]` value as ShareX writes it.
    fn stored(&mut self, text: &str) -> Value {
        if text.is_empty() {
            return Value::String(String::new());
        }
        match sharex_uploaders::dpapi::encrypt(text) {
            Some(e) => {
                let s = format!("{TAG}{e}");
                self.plain.insert(s.clone(), text.to_owned());
                Value::String(s)
            }
            None => Value::String(text.to_owned()),
        }
    }

    fn status_of(&self, key: &str, default: (String, Option<bool>)) -> Ed {
        let (text, status) = self.status.get(key).cloned().unwrap_or(default);
        Ed::Hint { text, status }
    }

    fn local_text(&self, key: &str) -> String {
        self.local.get(key).cloned().unwrap_or_default()
    }

    // --- Page building --------------------------------------------------

    fn build(&mut self, def: &PageDef, j: &Value, config: &UploadersConfig) -> Vec<Block> {
        let mut blocks = Vec::new();
        for c in &def.cards {
            match c.special.as_deref() {
                Some("account") => {
                    if let Some(card) = self.account_card(&def.id, j) {
                        blocks.push(Block::Card(card));
                    }
                }
                Some("imgur-albums") => blocks.push(Block::Card(self.imgur_albums(j))),
                Some("photobucket-albums") => blocks.push(Block::Card(self.photobucket_albums(j))),
                Some("onedrive-folder") => blocks.push(Block::Card(self.folder_card("onedrive", path(&["OneDriveV2SelectedFolder"]), j))),
                Some("box-folder") => blocks.push(Block::Card(self.folder_card("box", path(&["BoxSelectedFolder"]), j))),
                Some("mega-folders") => blocks.push(Block::Card(self.mega_folders(j))),
                Some("img-fish") => blocks.push(Block::Card(self.img_fish(j))),
                Some("amazon-s3") => {
                    for card in self.amazon_s3(j, config) {
                        blocks.push(Block::Card(card));
                    }
                }
                Some("remote-storage-browser") => {
                    // `hasValidAccount`; `_openRemoteStorageBrowser` is given.
                    let valid = if def.id == "amazon-s3" {
                        sharex_uploaders::validator::file(&sharex_uploaders::FileDestination::AmazonS3, config)
                    } else {
                        j["FTPAccountList"].as_array().is_some_and(|l| l.iter().any(valid_remote_storage_account))
                    };
                    blocks.push(Block::Action(Btn {
                        key: "remote-storage-browser".into(),
                        text: u::DestinationSettings_Remote_storage_browser().into(),
                        enabled: valid,
                        act: Act::RemoteStorageBrowser,
                    }));
                }
                _ => {
                    if let Some(list) = &c.list {
                        blocks.push(Block::Card(self.list_card(c, list, j)));
                    } else if let Some(hint) = &c.hint {
                        let _ = hint;
                        blocks.push(Block::Card(Card {
                            title: u::DestinationSettings_Settings().into(),
                            items: vec![Item::Hint(u::DestinationSettings_No_additional_configuration_required().into())],
                        }));
                    } else {
                        let items = c.rows.iter().map(|r| self.row(r, &[], j, j)).collect();
                        blocks.push(Block::Card(Card { title: card_title(c), items }));
                    }
                }
            }
        }
        blocks
    }

    /// `EditorRow` for a member at `base` + its path in `root`.
    fn row(&mut self, r: &RowDef, base: &[Seg], root: &Value, config: &Value) -> Item {
        let mut p: Path = base.to_vec();
        p.extend(r.path.iter().map(|k| Seg::K(k.clone())));
        let v = at(root, &p).clone();
        let label = r.label_key.as_deref().and_then(u::lookup).map(str::to_owned).unwrap_or_else(|| r.label.clone());
        let key = format!("{p:?}");
        let tip = r.tooltip.clone();
        let row = |ed: Ed| Item::Row { label: format!("{label}:"), ed, tip: tip.clone(), preview: false };
        match r.kind.as_str() {
            "bool" => Item::Check(
                Chk { key, text: label.clone(), value: v.as_bool().unwrap_or(false), enabled: true, bind: Bind::Bool(p) },
                r.tooltip.clone(),
            ),
            "string" => {
                let value = if r.encrypt { self.shown(&v) } else { str_of(&v) };
                row(Ed::Text { key, value, password: r.secret, enabled: true, bind: Bind::Str { path: p, encrypt: r.encrypt } })
            }
            "stringlist" => {
                let value = v.as_array().map(|a| a.iter().map(str_of).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                row(Ed::Text { key, value, password: false, enabled: true, bind: Bind::StrList(p) })
            }
            "enum" => {
                let items: Vec<String> =
                    r.values.iter().map(|e| e.key.as_deref().and_then(u::lookup).map(str::to_owned).unwrap_or_else(|| e.text.clone())).collect();
                let selected = match &v {
                    Value::String(s) => r.values.iter().position(|e| e.name.eq_ignore_ascii_case(s)),
                    Value::Number(n) => n.as_u64().map(|n| n as usize).filter(|&n| n < r.values.len()),
                    _ => None,
                }
                .unwrap_or(0);
                let names = r.values.iter().map(|e| e.name.clone()).collect();
                row(Ed::Combo { key, items, selected: Some(selected), enabled: true, bind: Bind::Enum { path: p, names } })
            }
            "selection" => {
                let (list, field) = match r.path.last().map(String::as_str) {
                    Some(n) if n.starts_with("FTPSelected") => ("FTPAccountList", n),
                    Some(n) if n.starts_with("LocalhostSelected") => ("LocalhostAccountList", n),
                    Some(n) => ("", n),
                    None => ("", ""),
                };
                let _ = field;
                let items: Vec<String> = config[list].as_array().map(|a| a.iter().map(|x| list_item_text(list, x)).collect()).unwrap_or_default();
                let current = v.as_i64().unwrap_or(0);
                let selected = usize::try_from(clamp_selection(current, items.len())).ok();
                row(Ed::Combo { key, items, selected, enabled: true, bind: Bind::Int(p) })
            }
            "number" => {
                let (min, max, decimals, bind) = match r.ty.as_deref() {
                    Some("Int32") => (i32::MIN as f64, i32::MAX as f64, 0, Bind::Int(p)),
                    Some("Int64") => (i64::MIN as f64, i64::MAX as f64, 0, Bind::Int(p)),
                    _ => {
                        let value = v.as_f64().unwrap_or(0.0);
                        let d = if value.fract() == 0.0 { 0 } else { 2 };
                        (-1_000_000_000.0, 1_000_000_000.0, d, Bind::Dec(p))
                    }
                };
                row(Ed::Number { key, value: v.as_f64().unwrap_or(0.0), min, max, decimals, enabled: true, bind })
            }
            _ => row(Ed::Hint { text: u::DestinationSettings_Not_configured().into(), status: None }),
        }
    }

    /// `BuildListEditor`: the list, Add/Duplicate/Remove, then the
    /// selected item's editors (an FTP account's with its URL preview).
    fn list_card(&mut self, c: &CardDef, list: &str, j: &Value) -> Card {
        let title = if list == "LocalhostAccountList" { u::DestinationSettings_Accounts().to_owned() } else { card_title(c) };
        let items_json = j[list].as_array().cloned().unwrap_or_default();
        if c.strings {
            let value = items_json.iter().map(str_of).collect::<Vec<_>>().join(", ");
            return Card {
                title,
                items: vec![Item::Stack(
                    0.0,
                    vec![Item::Row {
                        label: String::new(),
                        ed: Ed::Text { key: list.into(), value, password: false, enabled: true, bind: Bind::StrList(path(&[list])) },
                        tip: None,
                        preview: false,
                    }],
                )],
            };
        }
        let rows: Vec<String> = items_json.iter().map(|x| list_item_text(list, x)).collect();
        let selected = *self.list_selected.entry(list.to_owned()).or_insert_with(|| (!rows.is_empty()).then_some(0));
        let selected = selected.filter(|&i| i < rows.len());
        let mut details = Vec::new();
        if let Some(i) = selected {
            let base = vec![Seg::K(list.to_owned()), Seg::I(i)];
            let item = &items_json[i];
            let mut preview_added = false;
            for r in &c.rows {
                details.push(self.row(r, &base, j, j));
                if list == "FTPAccountList" && r.path.last().is_some_and(|n| n == "HttpHomePathNoExtension") {
                    details.push(preview_row(ftp_preview(item)));
                    preview_added = true;
                }
            }
            if list == "FTPAccountList" && !preview_added {
                details.push(preview_row(ftp_preview(item)));
            }
        }
        let default = c.default_item.clone().unwrap_or(Value::Null);
        let b = |key: &str, text: &str, act: Act| Btn { key: format!("{list}-{key}"), text: text.into(), enabled: true, act };
        Card {
            title,
            items: vec![Item::Stack(
                6.0,
                vec![
                    Item::List { key: list.into(), rows, selected, min_h: 150.0, max_h: 230.0, bind: Bind::ListSelect(list.into()) },
                    Item::Buttons(vec![
                        b("add", u::DestinationSettings_Add(), Act::ListAdd(list.into(), default)),
                        b("duplicate", u::DestinationSettings_Duplicate(), Act::ListDuplicate(list.into())),
                        b("remove", u::DestinationSettings_Remove(), Act::ListRemove(list.into())),
                    ]),
                    Item::Stack(CARD_SPACING, details),
                ],
            )],
        }
    }

    // --- DestinationSettingsAccounts ------------------------------------

    fn account_card(&mut self, page: &str, j: &Value) -> Option<Card> {
        Some(match page {
            "imgur" | "gist" | "dropbox" | "onedrive" | "box" | "bitly" => self.basic_oauth2(page, j),
            "google-drive" | "youtube" | "google-cloud-storage" => self.loopback(page, j),
            "flickr" | "photobucket" => self.oauth1(page, j),
            "imageshack" => self.login_card(page, not_blank(&j["ImageShackSettings"]["Auth_token"]), vec![
                btn(page, "login", u::DestinationSettings_Log_in(), Act::ImageShackLogin),
                btn(page, "profile", u::DestinationSettings_Public_profile(), {
                    let user = str_of(&j["ImageShackSettings"]["Username"]);
                    if user.trim().is_empty() { Act::Set(Vec::new(), None) } else { Act::OpenUrl(format!("https://imageshack.com/user/{user}")) }
                }),
                btn(page, "images", u::DestinationSettings_My_images(), Act::OpenUrl("https://imageshack.com/my/images".into())),
            ]),
            "pastebin" => self.login_card(page, not_blank(&j["PastebinSettings"]["UserKey"]), vec![
                btn(page, "login", u::DestinationSettings_Log_in(), Act::PastebinLogin),
                btn(page, "register", u::DestinationSettings_Register(), Act::OpenUrl("https://pastebin.com/signup".into())),
            ]),
            "paste-ee" => Card {
                title: u::DestinationSettings_Account().into(),
                items: vec![Item::Buttons(vec![btn(page, "user-key", u::DestinationSettings_Get_user_key(), Act::OpenUrl(signin::paste_ee_authorization_url()))])],
            },
            "puush" => self.user_password(page, "puush", path(&["PuushAPIKey"]), j),
            "lobfile" => self.user_password(page, "LobFile", path(&["LithiioSettings", "UserAPIKey"]), j),
            "pushbullet" => self.pushbullet(j),
            "mega" => self.mega_account(j),
            _ => return None,
        })
    }

    /// The Account card's status line.
    fn status_row(&self, key: &str, default: (String, Option<bool>)) -> Item {
        Item::Row { label: u::DestinationSettings_Status().into(), ed: self.status_of(key, default), tip: None, preview: false }
    }

    fn connected(c: bool) -> (String, Option<bool>) {
        (if c { u::DestinationSettings_Connected() } else { u::DestinationSettings_Not_connected() }.to_owned(), Some(c))
    }

    /// `BasicOAuth2`: status, authorization code, open / complete /
    /// refresh / disconnect.
    fn basic_oauth2(&mut self, page: &str, j: &Value) -> Card {
        let service = SignInService::from_page_id(page).expect("an OAuth 2 page");
        let member = oauth_members(service)[0];
        let key = format!("{page}-account");
        let code_key = format!("{page}-code");
        let connected = oauth2_connected(&j[member]);
        Card {
            title: u::DestinationSettings_Account().into(),
            items: vec![
                self.status_row(&key, Self::connected(connected)),
                Item::Row {
                    label: u::DestinationSettings_Authorization_code().into(),
                    ed: Ed::Text { key: code_key.clone(), value: self.local_text(&code_key), password: false, enabled: true, bind: Bind::Local(code_key) },
                    tip: None,
                    preview: false,
                },
                Item::Buttons(vec![
                    btn(page, "open", u::DestinationSettings_Open_authorization_page(), Act::OAuthOpen(service)),
                    btn(page, "complete", u::DestinationSettings_Complete_authorization(), Act::OAuthComplete(service)),
                    btn(page, "refresh", u::DestinationSettings_Refresh_token(), Act::OAuthRefresh(service)),
                    btn(page, "disconnect", u::DestinationSettings_Disconnect(), Act::Set(clear_members(service), Some(key.clone()))),
                ]),
            ],
        }
    }

    /// `LoopbackOAuth`: status ("Connected as"), connect, disconnect.
    fn loopback(&mut self, page: &str, j: &Value) -> Card {
        let service = SignInService::from_page_id(page).expect("a loopback page");
        let (info, user) = (oauth_members(service)[0], oauth_members(service)[1]);
        let key = format!("{page}-account");
        let connected = oauth2_connected(&j[info]);
        let name = str_of(&j[user]["name"]);
        let text = if !connected {
            u::DestinationSettings_Not_connected().to_owned()
        } else if name.trim().is_empty() {
            u::DestinationSettings_Connected().to_owned()
        } else {
            u::DestinationSettings_Connected_as().replace("{0}", &name)
        };
        Card {
            title: u::DestinationSettings_Account().into(),
            items: vec![
                self.status_row(&key, (text, Some(connected))),
                Item::Buttons(vec![
                    btn(page, "connect", u::DestinationSettings_Connect_account(), Act::Loopback(service)),
                    btn(page, "disconnect", u::DestinationSettings_Disconnect(), Act::Set(clear_members(service), Some(key.clone()))),
                ]),
            ],
        }
    }

    /// `FlickrOAuth` / `PhotobucketOAuth`: status, verification code,
    /// open / complete / disconnect.
    fn oauth1(&mut self, page: &str, j: &Value) -> Card {
        let service = SignInService::from_page_id(page).expect("an OAuth 1 page");
        let key = format!("{page}-account");
        let code_key = format!("{page}-code");
        let connected = oauth1_connected(&j[oauth_members(service)[0]]);
        Card {
            title: u::DestinationSettings_Account().into(),
            items: vec![
                self.status_row(&key, Self::connected(connected)),
                Item::Row {
                    label: u::DestinationSettings_Verification_code().into(),
                    ed: Ed::Text { key: code_key.clone(), value: self.local_text(&code_key), password: false, enabled: true, bind: Bind::Local(code_key) },
                    tip: None,
                    preview: false,
                },
                Item::Buttons(vec![
                    btn(page, "open", u::DestinationSettings_Open_authorization_page(), Act::OAuth1Open(service)),
                    btn(page, "complete", u::DestinationSettings_Complete_authorization(), Act::OAuth1Complete(service)),
                    btn(page, "disconnect", u::DestinationSettings_Disconnect(), Act::Set(clear_members(service), Some(key.clone()))),
                ]),
            ],
        }
    }

    /// `ImageShackAccount` / `PastebinAccount`: status, then buttons.
    fn login_card(&mut self, page: &str, connected: bool, buttons: Vec<Btn>) -> Card {
        let key = format!("{page}-account");
        Card {
            title: u::DestinationSettings_Account().into(),
            items: vec![self.status_row(&key, Self::connected(connected)), Item::Buttons(buttons)],
        }
    }

    /// `UserPasswordLogin`: user name and password (not kept), the API
    /// key, log in.
    fn user_password(&mut self, page: &str, service: &str, key_path: Path, j: &Value) -> Card {
        let key = format!("{page}-account");
        let user_key = format!("{page}-username");
        let pass_key = format!("{page}-password");
        let api = self.shown(at(j, &key_path));
        let text = |label: &str, k: &str, value: String, password: bool, bind: Bind| Item::Row {
            label: label.into(),
            ed: Ed::Text { key: k.into(), value, password, enabled: true, bind },
            tip: None,
            preview: false,
        };
        Card {
            title: u::DestinationSettings_Service_account().replace("{0}", service),
            items: vec![
                text(u::DestinationSettings_Email_or_username(), &user_key, self.local_text(&user_key), false, Bind::Local(user_key.clone())),
                text(u::DestinationSettings_Password(), &pass_key, self.local_text(&pass_key), true, Bind::Local(pass_key.clone())),
                text(u::DestinationSettings_API_key(), &format!("{page}-api-key"), api, true, Bind::Str { path: key_path.clone(), encrypt: true }),
                Item::Buttons(vec![btn(page, "login", u::DestinationSettings_Log_in(), Act::UserPassword(page.into(), key_path))]),
                self.status_row(&key, (u::DestinationSettings_Enter_credentials_to_retrieve_API_key().into(), None)),
            ],
        }
    }

    /// `PushbulletAccount`: API key, device, refresh devices, status.
    fn pushbullet(&mut self, j: &Value) -> Card {
        let key = "pushbullet-account";
        let s = &j["PushbulletSettings"];
        let devices: Vec<String> = s["DeviceList"]
            .as_array()
            .map(|a| a.iter().map(|d| d["Name"].as_str().map(str::to_owned).unwrap_or_else(|| u::DestinationSettings_Unnamed_device().into())).collect())
            .unwrap_or_default();
        let selected = usize::try_from(clamp_selection(s["SelectedDevice"].as_i64().unwrap_or(0), devices.len())).ok();
        let api = self.shown(&s["UserAPIKey"]);
        Card {
            title: u::DestinationSettings_Account_and_devices().into(),
            items: vec![
                Item::Row {
                    label: u::DestinationSettings_API_key().into(),
                    ed: Ed::Text { key: "pushbullet-api-key".into(), value: api, password: true, enabled: true, bind: Bind::Str { path: path(&["PushbulletSettings", "UserAPIKey"]), encrypt: true } },
                    tip: None,
                    preview: false,
                },
                Item::Row {
                    label: u::DestinationSettings_Device().into(),
                    ed: Ed::Combo { key: "pushbullet-device".into(), items: devices.clone(), selected, enabled: true, bind: Bind::PushbulletDevice },
                    tip: None,
                    preview: false,
                },
                Item::Buttons(vec![btn("pushbullet", "refresh", u::DestinationSettings_Refresh_devices(), Act::PushbulletRefresh)]),
                Item::Row { label: u::DestinationSettings_Status().into(), ed: self.status_of(key, (device_count(devices.len()), None)), tip: None, preview: false },
            ],
        }
    }

    /// `MegaAccount`: e-mail, password, log in / disconnect, status.
    fn mega_account(&mut self, j: &Value) -> Card {
        let key = "mega-account";
        let connected = not_blank(&j["MegaSessionID"]) && not_blank(&j["MegaMasterKey"]);
        let password = self.shown(&j["MegaPassword"]);
        Card {
            title: u::DestinationSettings_Account().into(),
            items: vec![
                Item::Row {
                    label: u::DestinationSettings_Field_Email().into(),
                    ed: Ed::Text { key: "mega-email".into(), value: str_of(&j["MegaEmail"]), password: false, enabled: true, bind: Bind::Mega(path(&["MegaEmail"]), false) },
                    tip: None,
                    preview: false,
                },
                Item::Row {
                    label: u::DestinationSettings_Field_Password().into(),
                    ed: Ed::Text { key: "mega-password".into(), value: password, password: true, enabled: true, bind: Bind::Mega(path(&["MegaPassword"]), true) },
                    tip: None,
                    preview: false,
                },
                Item::Buttons(vec![
                    btn("mega", "login", u::DestinationSettings_Log_in(), Act::MegaLogin(None)),
                    btn("mega", "disconnect", u::DestinationSettings_Disconnect(), Act::Set(mega_disconnect(), Some(key.into()))),
                ]),
                self.status_row(key, Self::connected(connected)),
            ],
        }
    }

    // --- The page builder's own cards -------------------------------------

    /// `BuildImgurAlbumsCard`.
    fn imgur_albums(&mut self, j: &Value) -> Card {
        let key = "imgur-albums";
        let albums: Vec<&Value> = j["ImgurAlbumList"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
        let items: Vec<String> = albums.iter().map(|a| album_title(a)).collect();
        let selected_id = j["ImgurSelectedAlbum"]["id"].as_str();
        let selected = selected_id.and_then(|id| albums.iter().position(|a| a["id"].as_str() == Some(id)));
        let count = albums.len();
        let status = if count == 0 {
            u::DestinationSettings_No_albums_loaded().to_owned()
        } else if count == 1 {
            u::DestinationSettings_Album_count_loaded_singular().replace("{0}", "1")
        } else {
            u::DestinationSettings_Album_count_loaded_plural().replace("{0}", &count.to_string())
        };
        Card {
            title: u::DestinationSettings_Albums().into(),
            items: vec![
                Item::Row {
                    label: u::DestinationSettings_Upload_to_album().into(),
                    ed: Ed::Combo { key: "imgur-album".into(), items, selected, enabled: true, bind: Bind::ImgurAlbum },
                    tip: None,
                    preview: false,
                },
                Item::Row { label: u::DestinationSettings_Status().into(), ed: self.status_of(key, (status, None)), tip: None, preview: false },
                Item::Buttons(vec![btn("imgur", "refresh-albums", u::DestinationSettings_Refresh_albums(), Act::ImgurAlbums)]),
            ],
        }
    }

    /// `BuildPhotobucketAlbumsCard`.
    fn photobucket_albums(&mut self, j: &Value) -> Card {
        let key = "photobucket-albums";
        let account = &j["PhotobucketAccountInfo"];
        let (albums, selected, status) = if account.is_null() {
            (Vec::new(), None, u::DestinationSettings_Connect_Photobucket_account_first().to_owned())
        } else {
            let albums: Vec<String> = account["AlbumList"].as_array().map(|a| a.iter().map(str_of).collect()).unwrap_or_default();
            let n = albums.len();
            let active = account["ActiveAlbumID"].as_i64().unwrap_or(0);
            let selected = if (0..n as i64).contains(&active) { Some(active as usize) } else if n > 0 { Some(0) } else { None };
            let status = if n == 0 {
                u::DestinationSettings_No_albums_saved().to_owned()
            } else if n == 1 {
                u::DestinationSettings_Album_count_singular().replace("{0}", "1")
            } else {
                u::DestinationSettings_Album_count_plural().replace("{0}", &n.to_string())
            };
            (albums, selected, status)
        };
        let album_path = self.local.get("photobucket-album-path").cloned().unwrap_or_else(|| str_of(&account["AlbumID"]));
        Card {
            title: u::DestinationSettings_Albums().into(),
            items: vec![
                Item::Row {
                    label: u::DestinationSettings_Album_path().into(),
                    ed: Ed::Text { key: "photobucket-album-path".into(), value: album_path, password: false, enabled: true, bind: Bind::PhotobucketAlbumPath },
                    tip: None,
                    preview: false,
                },
                Item::List { key: "photobucket-albums".into(), rows: albums, selected, min_h: 100.0, max_h: 180.0, bind: Bind::PhotobucketAlbum },
                Item::Buttons(vec![
                    btn("photobucket", "add-path", u::DestinationSettings_Add_path(), Act::PhotobucketAdd),
                    btn("photobucket", "remove", u::DestinationSettings_Remove(), Act::PhotobucketRemove),
                    btn("photobucket", "refresh", u::DestinationSettings_Refresh(), Act::PhotobucketRefresh),
                ]),
                Item::Row { label: u::DestinationSettings_Status().into(), ed: self.status_of(key, (status, None)), tip: None, preview: false },
            ],
        }
    }

    /// `BuildRemoteFolderCard` (OneDrive, Box): the selected folder, the
    /// current folder's sub folders (loaded by Refresh), Open folder /
    /// Back / Root, and the browser's status.
    fn folder_card(&mut self, page: &str, member: Path, j: &Value) -> Card {
        let selected_key = format!("{page}-folder");
        let browse_key = format!("{page}-browse");
        let root = folder_root(page);
        let current = at(j, &member);
        // `getName(getSelected()) ?? Root folder`.
        let selected_name = (if current.is_null() { &root } else { current })["name"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| u::DestinationSettings_Root_folder().into());
        let browser = self.browsers.entry(page.to_owned()).or_insert_with(|| FolderBrowser::new(root));
        let rows = browser.items.iter().map(folder_name).collect();
        let selected = browser.selected;
        let act = |a: FolderAct| Act::Folder(page.into(), member.clone(), a);
        Card {
            title: u::DestinationSettings_Upload_folder().into(),
            items: vec![
                Item::Row { label: u::DestinationSettings_Selected_folder().into(), ed: self.status_of(&selected_key, (selected_name, None)), tip: None, preview: false },
                Item::List { key: format!("{page}-folders"), rows, selected, min_h: 130.0, max_h: 220.0, bind: Bind::FolderSelect(page.into(), member.clone()) },
                Item::Buttons(vec![
                    btn(page, "folder-refresh", u::DestinationSettings_Refresh(), act(FolderAct::Refresh)),
                    btn(page, "folder-open", u::DestinationSettings_Open_folder(), act(FolderAct::Open)),
                    btn(page, "folder-back", u::DestinationSettings_Back(), act(FolderAct::Back)),
                    btn(page, "folder-root", u::DestinationSettings_Root(), act(FolderAct::Root)),
                ]),
                Item::Row {
                    label: u::DestinationSettings_Browser_status().into(),
                    ed: self.status_of(&browse_key, (u::DestinationSettings_Select_Refresh_to_load_folders().into(), None)),
                    tip: None,
                    preview: false,
                },
            ],
        }
    }

    /// `BuildMegaFolderTreeCard`: the selected folder and the folder tree
    /// (each folder's sub folders loaded when it's expanded).
    fn mega_folders(&mut self, j: &Value) -> Card {
        let selected = &j["MegaSelectedFolder"];
        let name = selected["Name"].as_str().filter(|n| !n.trim().is_empty()).map(str::to_owned).unwrap_or_else(|| u::DestinationSettings_Root_folder().into());
        let selected_id = selected["ID"].as_str().unwrap_or_default().trim().to_owned();
        Card {
            title: u::DestinationSettings_Upload_folder().into(),
            items: vec![
                Item::Row { label: u::DestinationSettings_Selected_folder().into(), ed: Ed::Hint { text: name, status: None }, tip: None, preview: false },
                Item::Tree { key: "mega-tree".into(), rows: self.mega.rows(&selected_id) },
            ],
        }
    }

    /// `BuildImgFishCard`.
    fn img_fish(&mut self, j: &Value) -> Card {
        let s = &j["ImgFishSettings"];
        let api = self.shown(&s["APIKey"]);
        Card {
            title: u::DestinationSettings_Settings().into(),
            items: vec![
                Item::Row {
                    label: format!("{}:", format_label("APIKey")),
                    ed: Ed::Text { key: "img-fish-api-key".into(), value: api, password: true, enabled: true, bind: Bind::Str { path: path(&["ImgFishSettings", "APIKey"]), encrypt: true } },
                    tip: None,
                    preview: false,
                },
                Item::Row {
                    label: format!("{}:", format_label("FileIDLength")),
                    ed: Ed::Number {
                        key: "img-fish-length".into(),
                        value: s["FileIDLength"].as_f64().unwrap_or(8.0),
                        min: 8.0,
                        max: 32.0,
                        decimals: 0,
                        enabled: true,
                        bind: Bind::Int(path(&["ImgFishSettings", "FileIDLength"])),
                    },
                    tip: None,
                    preview: false,
                },
            ],
        }
    }

    /// `BuildAmazonS3Cards`: Settings (with the URL preview) and Advanced.
    fn amazon_s3(&mut self, j: &Value, config: &UploadersConfig) -> Vec<Card> {
        let s = &j["AmazonS3Settings"];
        let p = |m: &str| path(&["AmazonS3Settings", m]);
        let text = |key: &str, value: String, password: bool, enabled: bool, bind: Bind| Ed::Text { key: key.into(), value, password, enabled, bind };
        let row = |m: &str, ed: Ed| Item::Row { label: format!("{}:", format_label(m)), ed, tip: None, preview: false };
        let check = |m: &str, label: &str, enabled: bool| Chk {
            key: format!("s3-{m}"),
            text: format_label(label),
            value: s[m].as_bool().unwrap_or(false),
            enabled,
            bind: Bind::Bool(p(m)),
        };
        let str_bind = |m: &str| Bind::Str { path: p(m), encrypt: false };
        let endpoint = str_of(&s["Endpoint"]);
        let endpoints = sharex_uploaders::destinations::amazon_s3::ENDPOINTS;
        let selected = endpoints.iter().position(|e| e.endpoint.eq_ignore_ascii_case(&endpoint));
        let secret = self.shown(&s["SecretAccessKey"]);
        let preview = sharex_uploaders::destinations::amazon_s3::AmazonS3::new(config.AmazonS3Settings.clone()).preview_url();
        let cname = s["UseCustomCNAME"].as_bool().unwrap_or(false);
        let multipart = s["UseMultipartUpload"].as_bool().unwrap_or(false);
        let classes = ["STANDARD", "STANDARD_IA", "ONEZONE_IA", "INTELLIGENT_TIERING"];
        let class_texts = ["Amazon S3 Standard", "Amazon S3 Standard-Infrequent Access", "Amazon S3 One Zone-Infrequent Access", "Amazon S3 Intelligent-Tiering"];
        let class_items: Vec<String> = classes
            .iter()
            .zip(class_texts)
            .map(|(c, t)| u::lookup(&format!("AmazonS3StorageClass_{c}")).map(str::to_owned).unwrap_or_else(|| t.to_owned()))
            .collect();
        let class = s["StorageClass"].as_str().and_then(|c| classes.iter().position(|x| x.eq_ignore_ascii_case(c))).unwrap_or(0);
        let open = |key: &str, url: &str| Btn { key: key.into(), text: "...".into(), enabled: true, act: Act::OpenUrl(url.into()) };
        vec![
            Card {
                title: u::DestinationSettings_Settings().into(),
                items: vec![
                    row("AccessKeyID", Ed::With(
                        Box::new(text("s3-access-key", str_of(&s["AccessKeyID"]), false, true, str_bind("AccessKeyID"))),
                        open("s3-access-key-open", "https://console.aws.amazon.com/iam/home?#security_credential"),
                    )),
                    row("SecretAccessKey", text("s3-secret", secret, true, true, Bind::Str { path: p("SecretAccessKey"), encrypt: true })),
                    row("Endpoints", Ed::Combo {
                        key: "s3-endpoints".into(),
                        items: endpoints.iter().map(|e| e.name.to_owned()).collect(),
                        selected,
                        enabled: true,
                        bind: Bind::S3Endpoint,
                    }),
                    row("Endpoint", text("s3-endpoint", endpoint.clone(), false, true, str_bind("Endpoint"))),
                    row("Region", text("s3-region", str_of(&s["Region"]), false, true, str_bind("Region"))),
                    row("Bucket", Ed::With(
                        Box::new(text("s3-bucket", str_of(&s["Bucket"]), false, true, str_bind("Bucket"))),
                        open("s3-bucket-open", "https://console.aws.amazon.com/s3/home"),
                    )),
                    row("ObjectPrefix", text("s3-prefix", str_of(&s["ObjectPrefix"]), false, true, str_bind("ObjectPrefix"))),
                    Item::Check(check("UseCustomCNAME", "UseCustomCNAME", true), None),
                    row("CustomDomain", text("s3-domain", str_of(&s["CustomDomain"]), false, cname, str_bind("CustomDomain"))),
                    preview_row(preview),
                ],
            },
            Card {
                title: format_label("Advanced"),
                items: vec![
                    row("StorageClass", Ed::With(
                        Box::new(Ed::Combo {
                            key: "s3-class".into(),
                            items: class_items,
                            selected: Some(class),
                            enabled: true,
                            bind: Bind::Enum { path: p("StorageClass"), names: classes.iter().map(|c| (*c).to_owned()).collect() },
                        }),
                        Btn { key: "s3-class-help".into(), text: "?".into(), enabled: true, act: Act::OpenUrl("https://aws.amazon.com/s3/storage-classes/".into()) },
                    )),
                    Item::Check(check("SignedPayload", "SignedPayload", true), None),
                    Item::Check(check("UseMultipartUpload", "UseMultipartUpload", true), None),
                    row("MultipartConcurrency", Ed::Number {
                        key: "s3-concurrency".into(),
                        value: s["MultipartConcurrency"].as_f64().unwrap_or(4.0),
                        min: 1.0,
                        max: 16.0,
                        decimals: 0,
                        enabled: multipart,
                        bind: Bind::Int(p("MultipartConcurrency")),
                    }),
                    Item::Check(check("SetPublicACL", "SetPublicACL", true), None),
                    Item::Check(check("UsePathStyle", "UsePathStyle", true), None),
                    row("RemoveFileExtensionOn", Ed::Checks(vec![
                        check("RemoveExtensionImage", "Image", true),
                        check("RemoveExtensionVideo", "Video", true),
                        check("RemoveExtensionText", "Text", true),
                    ])),
                ],
            },
        ]
    }

    // --- Changes ----------------------------------------------------------

    /// What a control changed, as the members it sets.
    fn input(&mut self, j: &Value, config: &UploadersConfig, bind: Bind, input: Input) -> Vec<(Path, Value)> {
        match (bind, input) {
            (Bind::Str { path, encrypt }, Input::Text(t)) => vec![(path, if encrypt { self.stored(&t) } else { Value::String(t) })],
            (Bind::StrList(path), Input::Text(t)) => {
                let items: Vec<Value> = t.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| Value::String(s.into())).collect();
                vec![(path, Value::Array(items))]
            }
            (Bind::Bool(path), Input::Bool(b)) => vec![(path, Value::Bool(b))],
            (Bind::Enum { path, names }, Input::Index(i)) => names.get(i).map(|n| vec![(path, Value::String(n.clone()))]).unwrap_or_default(),
            (Bind::Int(path), Input::Index(i)) => vec![(path, json!(i))],
            (Bind::Int(path), Input::Num(n)) => vec![(path, json!(n.round() as i64))],
            (Bind::Dec(path), Input::Num(n)) => vec![(path, json!(n))],
            (Bind::Local(key), Input::Text(t)) => {
                self.local.insert(key, t);
                Vec::new()
            }
            (Bind::MegaToggle(id), Input::Bool(expand)) => {
                self.mega_toggle(config, id, expand);
                Vec::new()
            }
            (Bind::MegaSelect(id), _) => {
                // `tree.SelectionChanged`: the folder is the upload folder.
                match self.mega.find(&id).and_then(|f| serde_json::to_value(f).ok()) {
                    Some(folder) => vec![(path(&["MegaSelectedFolder"]), folder)],
                    None => Vec::new(),
                }
            }
            (Bind::FolderSelect(page, member), Input::Index(i)) => {
                // `list.SelectionChanged`: the folder is the upload folder.
                let Some(browser) = self.browsers.get_mut(&page) else { return Vec::new() };
                let Some(item) = browser.items.get(i).cloned() else { return Vec::new() };
                browser.selected = Some(i);
                let name = item["name"].as_str().map(str::to_owned).unwrap_or_else(|| u::DestinationSettings_Unnamed_folder().into());
                self.status.insert(format!("{page}-folder"), (name, None));
                vec![(member, item)]
            }
            (Bind::S3Endpoint, Input::Index(i)) => sharex_uploaders::destinations::amazon_s3::ENDPOINTS
                .get(i)
                .map(|e| {
                    vec![
                        (path(&["AmazonS3Settings", "Endpoint"]), Value::String(e.endpoint.into())),
                        (path(&["AmazonS3Settings", "Region"]), e.region.map_or(Value::Null, |r| Value::String(r.into()))),
                    ]
                })
                .unwrap_or_default(),
            (Bind::ImgurAlbum, Input::Index(i)) => {
                let album = j["ImgurAlbumList"].get(i).cloned().unwrap_or(Value::Null);
                vec![(path(&["ImgurSelectedAlbum"]), album)]
            }
            (Bind::PushbulletDevice, Input::Index(i)) => vec![(path(&["PushbulletSettings", "SelectedDevice"]), json!(i))],
            (Bind::Mega(p, encrypt), Input::Text(t)) => {
                // A different e-mail or password disconnects (`Disconnect`).
                let old = if encrypt { self.shown(at(j, &p)) } else { str_of(at(j, &p)) };
                let mut out = Vec::new();
                if old != t {
                    out.extend(mega_disconnect());
                    self.status.remove("mega-account");
                }
                out.push((p, if encrypt { self.stored(&t) } else { Value::String(t) }));
                out
            }
            (Bind::PhotobucketAlbumPath, Input::Text(t)) => {
                self.local.insert("photobucket-album-path".into(), t.clone());
                if j["PhotobucketAccountInfo"].is_null() { Vec::new() } else { vec![(path(&["PhotobucketAccountInfo", "AlbumID"]), Value::String(t))] }
            }
            (Bind::ListSelect(list), Input::Index(i)) => {
                self.list_selected.insert(list, Some(i));
                Vec::new()
            }
            (Bind::PhotobucketAlbum, Input::Index(i)) => {
                if j["PhotobucketAccountInfo"].is_null() { Vec::new() } else { vec![(path(&["PhotobucketAccountInfo", "ActiveAlbumID"]), json!(i))] }
            }
            _ => Vec::new(),
        }
    }

    /// What a button does, as the members it sets.
    fn act(&mut self, j: &Value, config: &UploadersConfig, act: Act) -> Vec<(Path, Value)> {
        match act {
            Act::Set(sets, status) => {
                if let Some(k) = status {
                    self.status.remove(&k);
                }
                sets
            }
            Act::OpenUrl(url) => {
                self.opener.open(&url);
                Vec::new()
            }
            Act::OAuthOpen(service) => {
                // `GetAuthorizationURLAsync`, the account stored (or cleared
                // without a page), the page opened, "Authorization page opened".
                let key = format!("{}-account", service.page_id());
                let mut c = config.clone();
                self.spawn(&key.clone(), move || {
                    let r = signin::open_authorization(service, &mut c);
                    let sets = members(&c, oauth_members(service));
                    Box::new(move |w, _| match r {
                        Ok(url) => {
                            if let Some(url) = url {
                                w.opener.open(&url);
                            }
                            w.status.insert(key, (u::DestinationSettings_Authorization_page_opened().into(), None));
                            sets
                        }
                        Err(e) => {
                            w.status.insert(key, (e.to_string(), Some(false)));
                            Vec::new()
                        }
                    })
                });
                Vec::new()
            }
            Act::OAuthComplete(service) => {
                let key = format!("{}-account", service.page_id());
                let code = self.local_text(&format!("{}-code", service.page_id()));
                let mut c = config.clone();
                self.spawn(&key.clone(), move || {
                    let r = signin::complete_authorization(service, &mut c, &code);
                    let sets = members(&c, oauth_members(service));
                    Box::new(move |w, _| {
                        let (status, sets) = match r {
                            Ok(true) => (Self::connected(true), sets),
                            Ok(false) => ((u::DestinationSettings_Authorization_failed().into(), Some(false)), sets),
                            Err(e) => ((e.to_string(), Some(false)), Vec::new()),
                        };
                        w.status.insert(key, status);
                        sets
                    })
                });
                Vec::new()
            }
            Act::OAuthRefresh(service) => {
                let key = format!("{}-account", service.page_id());
                let mut c = config.clone();
                self.spawn(&key.clone(), move || {
                    let r = signin::refresh_token(service, &mut c);
                    let sets = members(&c, oauth_members(service));
                    Box::new(move |w, _| {
                        let (status, sets) = match r {
                            Ok(true) => (Self::connected(true), sets),
                            Ok(false) => ((u::DestinationSettings_Token_refresh_failed().into(), Some(false)), sets),
                            Err(e) => ((e.to_string(), Some(false)), Vec::new()),
                        };
                        w.status.insert(key, status);
                        sets
                    })
                });
                Vec::new()
            }
            Act::Loopback(service) => {
                // `OAuthListenerWindowIntegration.Show`: the waiting window
                // over the sign-in; whatever it ends with (connected,
                // cancelled, failed) is the account (`setAccount(result?.…)`).
                let key = format!("{}-account", service.page_id());
                if self.jobs.iter().any(|j| j.key == key) {
                    return Vec::new();
                }
                let cancel = sharex_uploaders::http::CancellationToken::new();
                let done = Arc::new(AtomicBool::new(false));
                self.effects.push(Effect::OpenDialog(Box::new(super::oauth_listener::OAuthListenerWindow::new(cancel.clone(), done.clone()))));
                let mut c = config.clone();
                let opener = self.opener.clone();
                self.spawn(&key.clone(), move || {
                    let r = signin::connect_loopback(service, &mut c, &|url: &str| opener.open(url), &cancel);
                    if let Err(e) = &r {
                        eprintln!("{e}");
                        signin::disconnect(service, &mut c);
                    }
                    done.store(true, Ordering::SeqCst);
                    let sets = members(&c, oauth_members(service));
                    Box::new(move |w, _| {
                        w.status.remove(&key);
                        sets
                    })
                });
                Vec::new()
            }
            Act::OAuth1Open(service) => {
                let key = format!("{}-account", service.page_id());
                let mut c = config.clone();
                self.spawn(&key.clone(), move || {
                    let r = signin::open_authorization_oauth1(service, &mut c);
                    let sets = members(&c, &oauth_members(service)[..1]);
                    Box::new(move |w, _| match r {
                        Ok(url) => {
                            if let Some(url) = url {
                                w.opener.open(&url);
                            }
                            w.status.insert(key, (u::DestinationSettings_Authorization_page_opened().into(), None));
                            sets
                        }
                        Err(e) => {
                            w.status.insert(key, (e.to_string(), Some(false)));
                            Vec::new()
                        }
                    })
                });
                Vec::new()
            }
            Act::OAuth1Complete(service) => {
                let key = format!("{}-account", service.page_id());
                // Photobucket without an account does nothing; Flickr says
                // the authorization failed.
                if service == SignInService::Photobucket && config.PhotobucketOAuthInfo.is_none() {
                    return Vec::new();
                }
                let code = self.local_text(&format!("{}-code", service.page_id()));
                let mut c = config.clone();
                self.spawn(&key.clone(), move || {
                    let r = signin::complete_authorization_oauth1(service, &mut c, &code);
                    let sets = members(&c, oauth_members(service));
                    Box::new(move |w, _| {
                        let (status, sets) = match r {
                            Ok(true) => (Self::connected(true), sets),
                            Ok(false) => ((u::DestinationSettings_Authorization_failed().into(), Some(false)), sets),
                            Err(e) => ((e.to_string(), Some(false)), Vec::new()),
                        };
                        w.status.insert(key, status);
                        sets
                    })
                });
                Vec::new()
            }
            Act::ImageShackLogin | Act::PastebinLogin => {
                // `LoginImageShackAsync` / `LoginPastebinAsync`.
                let (key, member) = if matches!(act, Act::ImageShackLogin) { ("imageshack-account", "ImageShackSettings") } else { ("pastebin-account", "PastebinSettings") };
                let image_shack = matches!(act, Act::ImageShackLogin);
                let mut c = config.clone();
                self.spawn(key, move || {
                    let r = if image_shack { signin::login_image_shack(&mut c) } else { signin::login_pastebin(&mut c) };
                    let sets = members(&c, &[member]);
                    Box::new(move |w, _| {
                        let (status, sets) = match r {
                            Ok(true) => (Self::connected(true), sets),
                            Ok(false) => ((u::DestinationSettings_Login_failed().into(), Some(false)), sets),
                            Err(e) => ((e.to_string(), Some(false)), Vec::new()),
                        };
                        w.status.insert(key.into(), status);
                        sets
                    })
                });
                Vec::new()
            }
            Act::UserPassword(page, key_path) => {
                // puush's `LoginAsync`, LobFile's `FetchAPIKeyAsync`: the key
                // into the API key box, the password box cleared.
                let key = format!("{page}-account");
                let user = self.local_text(&format!("{page}-username"));
                let password = self.local_text(&format!("{page}-password"));
                let puush = page == "puush";
                self.spawn(&key.clone(), move || {
                    let r = if puush {
                        sharex_uploaders::destinations::file_hosts::Puush::new("").login(&user, &password)
                    } else {
                        sharex_uploaders::destinations::file_hosts::LobFile::new("").fetch_api_key(&user, &password)
                    };
                    Box::new(move |w, _| match r {
                        Ok(Some(api_key)) if !api_key.trim().is_empty() => {
                            w.status.insert(key, (u::DestinationSettings_API_key_retrieved().into(), Some(true)));
                            w.local.insert(format!("{page}-password"), String::new());
                            vec![(key_path, w.stored(&api_key))]
                        }
                        Ok(_) => {
                            w.status.insert(key, (u::DestinationSettings_Login_failed().into(), Some(false)));
                            Vec::new()
                        }
                        Err(e) => {
                            w.status.insert(key, (e.to_string(), Some(false)));
                            Vec::new()
                        }
                    })
                });
                Vec::new()
            }
            Act::PushbulletRefresh => {
                // `GetDeviceListAsync() ?? []`, then `Reload()`.
                let settings = sharex_uploaders::destinations::services::PushbulletSettings::from_value(&config.PushbulletSettings).unwrap_or_default();
                self.spawn("pushbullet-account", move || {
                    let r = sharex_uploaders::destinations::services::Pushbullet::new(settings).get_device_list();
                    Box::new(move |w, j| match r {
                        Ok(devices) => {
                            w.status.remove("pushbullet-account");
                            let previous = j["PushbulletSettings"]["SelectedDevice"].as_i64().unwrap_or(0);
                            vec![
                                (path(&["PushbulletSettings", "DeviceList"]), serde_json::to_value(&devices).unwrap_or(Value::Array(Vec::new()))),
                                (path(&["PushbulletSettings", "SelectedDevice"]), json!(clamp_selection(previous, devices.len()))),
                            ]
                        }
                        Err(e) => {
                            w.status.insert("pushbullet-account".into(), (e.to_string(), None));
                            Vec::new()
                        }
                    })
                });
                Vec::new()
            }
            Act::MegaLogin(code) => {
                // `LoginAsync()`; error -26 asks for the two-factor code
                // and logs in again with it.
                let mut c = config.clone();
                self.spawn("mega-account", move || {
                    let r = signin::mega_login(&mut c, code.as_deref());
                    let sets = members(&c, &["MegaPassword", "MegaSessionID", "MegaMasterKey"]);
                    Box::new(move |w, _| match r {
                        Ok(()) => {
                            w.status.insert("mega-account".into(), Self::connected(true));
                            sets
                        }
                        Err(sharex_uploaders::destinations::mega::MegaError::Api(-26)) if code.is_none() => {
                            let (tx, rx) = mpsc::channel();
                            let prompt = u::DestinationSettings_Enter_current_two_factor_authentication_code();
                            w.effects.push(Effect::OpenDialog(Box::new(super::input_box::InputBoxWindow::new(prompt, None, None, None, Some(tx)))));
                            w.two_factor = Some(rx);
                            Vec::new()
                        }
                        Err(e) => {
                            w.status.insert("mega-account".into(), (e.to_string(), Some(false)));
                            Vec::new()
                        }
                    })
                });
                Vec::new()
            }
            Act::ImgurAlbums => {
                if !OAuth2Info::check(config.ImgurOAuth2Info.as_ref()) {
                    self.status.insert("imgur-albums".into(), (u::DestinationSettings_Connect_Imgur_account_first().into(), None));
                    return Vec::new();
                }
                let mut c = config.clone();
                self.spawn("imgur-albums", move || {
                    let r = signin::refresh_imgur_albums(&mut c);
                    let mut sets = members(&c, &["ImgurOAuth2Info"]);
                    let albums = members(&c, &["ImgurAlbumList"]);
                    Box::new(move |w, j| {
                        match r {
                            Ok(Some(list)) => {
                                // `LoadAlbums`: the album with the selected id, or none.
                                w.status.remove("imgur-albums");
                                sets.extend(albums);
                                let id = j["ImgurSelectedAlbum"]["id"].as_str();
                                let selected = list.iter().find(|a| id.is_some() && a.id.as_deref() == id);
                                sets.push((path(&["ImgurSelectedAlbum"]), selected.and_then(|a| serde_json::to_value(a).ok()).unwrap_or(Value::Null)));
                            }
                            Ok(None) => {
                                w.status.insert("imgur-albums".into(), (u::DestinationSettings_Connect_Imgur_account_first().into(), None));
                            }
                            Err(e) => {
                                w.status.insert("imgur-albums".into(), (e.to_string(), None));
                            }
                        }
                        sets
                    })
                });
                Vec::new()
            }
            Act::Folder(page, member, what) => self.folder_act(config, &page, member, what),
            Act::RemoteStorageBrowser => {
                self.effects.push(Effect::OpenRemoteStorageBrowser);
                Vec::new()
            }
            Act::ListAdd(list, item) => {
                let mut items = j[&list].as_array().cloned().unwrap_or_default();
                items.push(item);
                self.list_selected.insert(list.clone(), Some(items.len() - 1));
                vec![(path(&[&list]), Value::Array(items))]
            }
            Act::ListDuplicate(list) => {
                let mut items = j[&list].as_array().cloned().unwrap_or_default();
                let Some(i) = self.list_selected.get(&list).copied().flatten().filter(|&i| i < items.len()) else { return Vec::new() };
                items.push(items[i].clone());
                self.list_selected.insert(list.clone(), Some(items.len() - 1));
                vec![(path(&[&list]), Value::Array(items))]
            }
            Act::ListRemove(list) => {
                let mut items = j[&list].as_array().cloned().unwrap_or_default();
                let Some(i) = self.list_selected.get(&list).copied().flatten().filter(|&i| i < items.len()) else { return Vec::new() };
                items.remove(i);
                // The details are cleared; nothing is selected.
                self.list_selected.insert(list.clone(), None);
                vec![(path(&[&list]), Value::Array(items))]
            }
            Act::PhotobucketAdd => {
                let account = &j["PhotobucketAccountInfo"];
                let album = self.local.get("photobucket-album-path").cloned().unwrap_or_else(|| str_of(&account["AlbumID"]));
                let mut albums = account["AlbumList"].as_array().cloned().unwrap_or_default();
                if account.is_null() || album.trim().is_empty() || albums.iter().any(|a| a.as_str() == Some(&album)) {
                    return Vec::new();
                }
                albums.push(Value::String(album));
                let last = albums.len() - 1;
                self.status.remove("photobucket-albums");
                vec![(path(&["PhotobucketAccountInfo", "AlbumList"]), Value::Array(albums)), (path(&["PhotobucketAccountInfo", "ActiveAlbumID"]), json!(last))]
            }
            Act::PhotobucketRemove => {
                let account = &j["PhotobucketAccountInfo"];
                let mut albums = account["AlbumList"].as_array().cloned().unwrap_or_default();
                let n = albums.len();
                let active = account["ActiveAlbumID"].as_i64().unwrap_or(0);
                let selected = if (0..n as i64).contains(&active) { active as usize } else { 0 };
                if account.is_null() || n <= 1 {
                    return Vec::new();
                }
                albums.remove(selected);
                let active = active.min(albums.len() as i64 - 1);
                vec![(path(&["PhotobucketAccountInfo", "AlbumList"]), Value::Array(albums)), (path(&["PhotobucketAccountInfo", "ActiveAlbumID"]), json!(active))]
            }
            Act::PhotobucketRefresh => {
                self.local.remove("photobucket-album-path");
                self.status.remove("photobucket-albums");
                Vec::new()
            }
        }
    }

    /// A remote folder card's Refresh, Open folder, Back and Root.
    fn folder_act(&mut self, config: &UploadersConfig, page: &str, member: Path, what: FolderAct) -> Vec<(Path, Value)> {
        let root = folder_root(page);
        let browser = self.browsers.entry(page.to_owned()).or_insert_with(|| FolderBrowser::new(root.clone()));
        let mut sets = Vec::new();
        match what {
            FolderAct::Refresh => {}
            FolderAct::Open => {
                let Some(selected) = browser.selected.and_then(|i| browser.items.get(i)).cloned() else { return sets };
                let previous = std::mem::replace(&mut browser.current, selected);
                browser.history.push(previous);
            }
            FolderAct::Back => {
                let Some(previous) = browser.history.pop() else { return sets };
                browser.current = previous;
            }
            FolderAct::Root => {
                browser.history.clear();
                browser.current = root.clone();
                let name = root["name"].as_str().map(str::to_owned).unwrap_or_else(|| u::DestinationSettings_Root_folder().into());
                self.status.insert(format!("{page}-folder"), (name, None));
                sets.push((member, root));
            }
        }
        self.load_folders(config, page);
        sets
    }

    /// `LoadAsync(folder)`: the list cleared, the folder's sub folders
    /// loaded (`GetPathInfoAsync` / `GetFilesAsync`), the count or the
    /// error in the browser's status.
    fn load_folders(&mut self, config: &UploadersConfig, page: &str) {
        let Some(browser) = self.browsers.get_mut(page) else { return };
        browser.items.clear();
        browser.selected = None;
        browser.generation += 1;
        let generation = browser.generation;
        let id = str_of(&browser.current["id"]);
        let page = page.to_owned();
        let info_member = if page == "box" { "BoxOAuth2Info" } else { "OneDriveV2OAuth2Info" };
        let info = if page == "box" { config.BoxOAuth2Info.clone() } else { config.OneDriveV2OAuth2Info.clone() };
        let job = format!("{page}-browse-{generation}");
        self.spawn(&job, move || {
            let had_info = info.is_some();
            let (r, info) = if page == "box" {
                let mut b = sharex_uploaders::destinations::oauth_services::BoxUploader::new(info.unwrap_or_default());
                let r = b.get_files(&id).map(|v| v.and_then(|v| v["entries"].as_array().cloned()).unwrap_or_default());
                let r = r.map(|entries| entries.into_iter().filter(|e| e["type"].as_str() == Some("folder")).map(box_entry).collect::<Vec<_>>());
                (r, b.auth_info)
            } else {
                let mut o = sharex_uploaders::destinations::oauth_services::OneDrive::new(info.unwrap_or_default());
                let r = o.get_path_info(&id).map(|v| v.and_then(|v| v["value"].as_array().cloned()).unwrap_or_default());
                (r.map(|items| items.into_iter().map(one_drive_item).collect::<Vec<_>>()), o.auth_info)
            };
            // A token refreshed on the way is the account's (ShareX's
            // uploader shares the config's `OAuth2Info`).
            let token = had_info.then(|| (path(&[info_member]), serde_json::to_value(&info).unwrap_or(Value::Null)));
            Box::new(move |w, _| {
                let Some(browser) = w.browsers.get_mut(&page).filter(|b| b.generation == generation) else { return token.into_iter().collect() };
                let status = match r {
                    Ok(items) => {
                        browser.items = items;
                        let n = browser.items.len();
                        match n {
                            0 => u::DestinationSettings_No_child_folders().to_owned(),
                            1 => u::DestinationSettings_Folder_count_singular().replace("{0}", "1"),
                            n => u::DestinationSettings_Folder_count_plural().replace("{0}", &n.to_string()),
                        }
                    }
                    Err(e) => e.to_string(),
                };
                w.status.insert(format!("{page}-browse"), (status, None));
                token.into_iter().collect()
            })
        });
    }

    /// MEGA's tree: a folder expanded (its sub folders loaded the first
    /// time, `LoadChildrenAsync`) or collapsed.
    fn mega_toggle(&mut self, config: &UploadersConfig, id: String, expand: bool) {
        if !expand {
            self.mega.expanded.remove(&id);
            return;
        }
        self.mega.expanded.insert(id.clone());
        if self.mega.children.contains_key(&id) || self.mega.loading.contains(&id) {
            return;
        }
        let Some(folder) = self.mega.find(&id) else { return };
        self.mega.loading.insert(id.clone());
        let c = config.clone();
        self.spawn(&format!("mega-tree-{id}"), move || {
            let r = signin::mega_folders(&c, Some(&folder));
            Box::new(move |w, _| {
                w.mega.loading.remove(&id);
                match r {
                    Ok(folders) => {
                        w.mega.children.insert(id, folders);
                    }
                    // The "…" again, to try once more.
                    Err(e) => {
                        eprintln!("{e}");
                        w.mega.children.remove(&id);
                    }
                }
                Vec::new()
            })
        });
    }

    /// ShareX's builder normalizes some members as it builds the pages:
    /// account and device selections are clamped to their lists (-1
    /// without items) and the Imgur album is the listed one or none.
    fn normalize(config: &mut UploadersConfig) {
        let Ok(j) = serde_json::to_value(&*config) else { return };
        let mut sets = Vec::new();
        for (list, members) in [
            ("FTPAccountList", ["FTPSelectedImage", "FTPSelectedText", "FTPSelectedFile"]),
            ("LocalhostAccountList", ["LocalhostSelectedImages", "LocalhostSelectedText", "LocalhostSelectedFiles"]),
        ] {
            let n = j[list].as_array().map_or(0, Vec::len);
            for m in members {
                let v = j[m].as_i64().unwrap_or(0);
                let c = clamp_selection(v, n);
                if c != v {
                    sets.push((path(&[m]), json!(c)));
                }
            }
        }
        let devices = j["PushbulletSettings"]["DeviceList"].as_array().map_or(0, Vec::len);
        let device = j["PushbulletSettings"]["SelectedDevice"].as_i64().unwrap_or(0);
        if clamp_selection(device, devices) != device && j["PushbulletSettings"].is_object() {
            sets.push((path(&["PushbulletSettings", "SelectedDevice"]), json!(clamp_selection(device, devices))));
        }
        let selected_id = j["ImgurSelectedAlbum"]["id"].as_str();
        let album = j["ImgurAlbumList"].as_array().and_then(|a| a.iter().find(|x| selected_id.is_some() && x["id"].as_str() == selected_id)).cloned();
        let album = album.unwrap_or(Value::Null);
        if album != j["ImgurSelectedAlbum"] {
            sets.push((path(&["ImgurSelectedAlbum"]), album));
        }
        apply(config, sets);
    }

    // --- Drawing ----------------------------------------------------------

    /// The pages the search leaves, with the cards it leaves on each.
    fn visible(&mut self, ui: &Ui, config: &UploadersConfig, j: &Value, terms: &[String]) -> Vec<Vec<bool>> {
        let _ = ui;
        categories()
            .iter()
            .map(|cat| {
                cat.pages
                    .iter()
                    .map(|p| {
                        if terms.is_empty() {
                            return true;
                        }
                        let title = page_title(p);
                        let blocks = self.build(p, j, config);
                        let mut text = vec![title];
                        for b in &blocks {
                            match b {
                                Block::Card(c) => text.push(card_text(c)),
                                Block::Action(b) => text.push(b.text.clone()),
                            }
                        }
                        matches(&text.join(" "), terms)
                    })
                    .collect()
            })
            .collect()
    }

    /// `SettingsNavigation`: the search box and the category tree.
    fn navigation(&mut self, ui: &mut Ui, full: Rect, visible: &[Vec<bool>]) {
        let p = palette(ui.ctx());
        let d = sharex_ui::theme::density(ui.ctx());
        let nav = Rect::from_min_size(full.min, vec2(NAV_WIDTH, full.height()));
        ui.painter().rect_filled(nav, 0.0, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_max(pos2(nav.max.x - 1.0, nav.min.y), nav.max), 0.0, p.border);
        let inner = Rect::from_min_max(pos2(nav.min.x + 8.0, nav.min.y + 8.0), pos2(nav.max.x - 1.0 - 8.0, nav.max.y - 8.0));
        let font_h = line_height(ui, &TextBox::font());
        let search_h = (1.0 + d.settings_search_padding.top + font_h + d.settings_search_padding.bottom + 1.0).max(40.0);
        let search = Rect::from_min_size(inner.min, vec2(inner.width(), search_h));
        let tb = TextBox { padding: d.settings_search_padding, placeholder: sharex_strings::avalonia::SettingsNavigation_Search_settings(), ..Default::default() };
        let before = self.search.clone();
        tb.show(ui, search, Id::new("destination-settings-search"), &mut self.search);
        if self.search != before && !self.search.trim().is_empty() {
            self.expanded.iter_mut().for_each(|e| *e = true);
        }
        icons::paint(ui.painter(), Rect::from_min_size(pos2(search.min.x + 8.0, center_y(search, 16.0)), Vec2::splat(16.0)), icon::Search, p.text);
        let tree = Rect::from_min_max(pos2(inner.min.x, search.max.y + d.settings_nav_spacing), inner.max);
        let item_h = d.settings_nav_item_min_height.max(line_height(ui, &label_font()) + d.settings_nav_item_margin.top + d.settings_nav_item_margin.bottom);
        // (category, page or None for the category's header, top).
        let mut rows: Vec<(usize, Option<usize>, f32)> = Vec::new();
        let mut y = 0.0;
        for (c, cat) in categories().iter().enumerate() {
            if !visible[c].iter().any(|v| *v) {
                continue;
            }
            y += 1.0;
            rows.push((c, None, y));
            y += item_h;
            if self.expanded[c] {
                for i in (0..cat.pages.len()).filter(|&i| visible[c][i]) {
                    y += 1.0;
                    rows.push((c, Some(i), y));
                    y += item_h + 1.0;
                }
            }
            y += 1.0;
        }
        let content_h = y;
        let max_scroll = (content_h - tree.height()).max(0.0);
        if ui.rect_contains_pointer(tree) {
            self.nav_scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        if self.nav_shown != Some(self.page)
            && let Some(&(_, _, top)) = rows.iter().find(|(c, i, _)| (*c, *i) == (self.page.0, Some(self.page.1)))
        {
            if top < self.nav_scroll {
                self.nav_scroll = top;
            } else if top + item_h > self.nav_scroll + tree.height() {
                self.nav_scroll = top + item_h - tree.height();
            }
            self.nav_shown = Some(self.page);
        }
        self.nav_scroll = self.nav_scroll.clamp(0.0, max_scroll);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(tree));
        child.set_clip_rect(tree.intersect(ui.clip_rect()));
        for &(c, i, top) in &rows {
            let r = Rect::from_min_size(pos2(tree.min.x, tree.min.y - self.nav_scroll + top), vec2(tree.width(), item_h));
            if !r.intersects(tree) {
                continue;
            }
            let resp = child.interact(r, Id::new(("destination-nav", c, i)), egui::Sense::click());
            let selected = i.is_some_and(|i| (c, i) == self.page);
            settings_form::nav_item_background(child.painter(), r, &resp, selected);
            let indent = if i.is_some() { 16.0 } else { 0.0 };
            let fg = if selected { p.accent_foreground } else { p.text };
            let mut toggled = false;
            if i.is_none() {
                let cr = Rect::from_min_size(pos2(r.min.x + indent + 12.0, center_y(r, 12.0)), Vec2::splat(12.0));
                let hit = Rect::from_min_max(pos2(r.min.x + indent, r.min.y), pos2(r.min.x + indent + 36.0, r.max.y));
                let ex = child.interact(hit, Id::new(("destination-nav-expand", c)), egui::Sense::click());
                crate::task_settings_window::paint_tree_chevron(&child, cr, self.expanded[c], fg);
                if ex.clicked() {
                    self.expanded[c] = !self.expanded[c];
                    toggled = true;
                }
            }
            let title = match i {
                Some(i) => page_title(&categories()[c].pages[i]),
                None => category_title(&categories()[c].id).to_owned(),
            };
            let x = r.min.x + indent + 36.0 + d.settings_nav_item_margin.left;
            controls::label(&child, Rect::from_min_max(pos2(x, r.min.y), pos2(r.max.x - d.settings_nav_item_margin.right, r.max.y)), &title, label_font(), fg);
            settings_form::nav_item_focus(child.painter(), r, &resp);
            if resp.clicked() && !toggled && !selected {
                // A category selects its first page (`SelectedNavigationItem`).
                let page = i.unwrap_or(0);
                if visible[c].get(page).copied().unwrap_or(false) || i.is_none() {
                    self.page = (c, page);
                }
            }
        }
        super::layout::scroll_bar(&child, tree, content_h, self.nav_scroll, 16.0);
    }

    /// The page in `column`: the title, the cards, the actions.
    fn page(&mut self, ui: &mut Ui, column: Rect, blocks: &[Block], terms: &[String], def: &PageDef, icon_key: &str) -> Events {
        let p = palette(ui.ctx());
        let title = page_title(def);
        let tf = font(24.0, 600);
        let title_h = line_height(ui, &tf).max(22.0);
        let shown: Vec<&Block> = blocks
            .iter()
            .filter(|b| match b {
                Block::Card(c) => terms.is_empty() || matches(&format!("{title} {}", card_text(c)), terms),
                Block::Action(_) => true,
            })
            .collect();
        // `HorizontalAlignment="Left"`, `MaxWidth="780"`: as wide as its
        // widest child.
        let avail = (column.width() - PAGE_MARGIN.left - PAGE_MARGIN.right).max(0.0);
        let mut desired = 22.0 + 9.0 + text_width(ui, &title, tf.clone());
        for b in &shown {
            desired = desired.max(match b {
                Block::Card(c) => card_width(ui, c),
                Block::Action(b) => button().width(ui, &b.text),
            });
        }
        let width = desired.min(PAGE_MAX).min(avail);
        let key = (self.page.0, self.page.1);
        let scroll = self.scroll.get(&key).copied().unwrap_or(0.0);
        let x = column.min.x + PAGE_MARGIN.left;
        let top = column.min.y + PAGE_MARGIN.top - scroll;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(column));
        child.set_clip_rect(column.intersect(ui.clip_rect()));
        // The title: the category's icon, 9px, the page's title.
        let r = Rect::from_min_size(pos2(x, top), vec2(width, title_h));
        icons::paint(child.painter(), Rect::from_min_size(pos2(r.min.x, center_y(r, 22.0)), Vec2::splat(22.0)), icon_key, p.accent_icon);
        controls::label(&child, Rect::from_min_max(pos2(r.min.x + 22.0 + 9.0, r.min.y), pos2(column.max.x, r.max.y)), &title, tf, p.accent_text);
        let mut y = top + title_h + 14.0;
        let mut painter = Painter { ui: &mut child, page: def.id.clone(), ev: Events::default() };
        for b in shown {
            match b {
                Block::Card(c) => {
                    let inner_w = width - 2.0 - 2.0 * CARD_PAD;
                    let ix = x + 1.0 + CARD_PAD;
                    let mut heights = Vec::with_capacity(c.items.len());
                    let tlh = line_height(painter.ui, &title_font());
                    let mut h = 1.0 + tlh + 3.0;
                    for i in &c.items {
                        let ih = item_height(painter.ui, i, inner_w);
                        heights.push(ih);
                        h += CARD_SPACING + ih;
                    }
                    let card = Rect::from_min_size(pos2(x, y), vec2(width, 1.0 + CARD_PAD + h + CARD_PAD + 1.0));
                    painter.ui.painter().add(Shape::Vec(vec![
                        egui::epaint::RectShape::filled(card, CornerRadius::same(6), p.background_panel).into(),
                        egui::epaint::RectShape::stroke(card, CornerRadius::same(6), Stroke::new(1.0, p.border), StrokeKind::Inside).into(),
                    ]));
                    let mut iy = y + 1.0 + CARD_PAD;
                    painter.ui.painter().galley(pos2(ix, iy + 1.0), galley(painter.ui, &c.title, title_font(), p.text, f32::INFINITY), p.text);
                    iy += 1.0 + tlh + 3.0;
                    for (i, ih) in c.items.iter().zip(heights) {
                        iy += CARD_SPACING;
                        painter.item(i, Rect::from_min_size(pos2(ix, iy), vec2(inner_w, ih)));
                        iy += ih;
                    }
                    y = card.max.y + CARD_GAP;
                }
                Block::Action(b) => {
                    y += 4.0;
                    let w = button().width(painter.ui, &b.text);
                    painter.button(b, Rect::from_min_size(pos2(x, y), vec2(w, CONTROL_H)));
                    y += CONTROL_H;
                }
            }
        }
        let height = y + PAGE_MARGIN.bottom - top;
        let ev = std::mem::take(&mut painter.ev);
        let max = (height - column.height()).max(0.0);
        let mut s = scroll;
        if ui.rect_contains_pointer(column) && !settings_form::any_combo_open(ui.ctx()) {
            s -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        s = s.clamp(0.0, max);
        self.scroll.insert(key, s);
        super::layout::scroll_bar(&child, column, height, s, 16.0);
        ev
    }
}

fn btn(page: &str, key: &str, text: &str, act: Act) -> Btn {
    Btn { key: format!("{page}-{key}"), text: text.into(), enabled: true, act }
}

/// `PreviewRow`: "URL preview:" and the URL (`Margin="0,6"`).
fn preview_row(url: String) -> Item {
    Item::Row { label: format!("{}:", format_label("URLPreview")), ed: Ed::Hint { text: url, status: None }, tip: None, preview: true }
}

fn card_title(c: &CardDef) -> String {
    match c.title.as_deref() {
        None | Some("Settings") => u::DestinationSettings_Settings().into(),
        Some(t) => translated_label(t),
    }
}

/// `ImgurAlbumData`'s title, or its id.
fn album_title(a: &Value) -> String {
    match a["title"].as_str() {
        Some(t) if !t.trim().is_empty() => t.to_owned(),
        _ => str_of(&a["id"]),
    }
}

/// `OneDrive.RootFolder` / `Box.RootFolder`.
fn folder_root(page: &str) -> Value {
    let d = UploadersConfig::default();
    if page == "box" { d.BoxSelectedFolder } else { d.OneDriveV2SelectedFolder }
}

/// MEGA's `Disconnect`.
fn mega_disconnect() -> Vec<(Path, Value)> {
    vec![
        (path(&["MegaSessionID"]), Value::String(String::new())),
        (path(&["MegaMasterKey"]), Value::String(String::new())),
        (path(&["MegaSelectedFolder"]), UploadersConfig::default().MegaSelectedFolder),
    ]
}

/// `UploadersConfigValidator.IsValidRemoteStorageAccount`.
fn valid_remote_storage_account(a: &Value) -> bool {
    let blank = |v: &Value| v.as_str().is_none_or(|s| s.trim().is_empty());
    if blank(&a["Host"]) {
        return false;
    }
    match a["Protocol"].as_str().unwrap_or("FTP") {
        "FTP" | "FTPS" => a["Port"].as_i64().unwrap_or(0) > 0,
        "SFTP" => !blank(&a["Username"]) && (!blank(&a["Password"]) || !blank(&a["Keypath"])),
        _ => false,
    }
}

/// The members a sign-in reads and writes.
fn oauth_members(service: SignInService) -> &'static [&'static str] {
    match service {
        SignInService::Imgur => &["ImgurOAuth2Info"],
        SignInService::Gist => &["GistOAuth2Info"],
        SignInService::Dropbox => &["DropboxOAuth2Info"],
        SignInService::OneDrive => &["OneDriveV2OAuth2Info"],
        SignInService::Box => &["BoxOAuth2Info"],
        SignInService::Bitly => &["BitlyOAuth2Info"],
        SignInService::GoogleDrive => &["GoogleDriveOAuth2Info", "GoogleDriveUserInfo"],
        SignInService::GoogleCloudStorage => &["GoogleCloudStorageOAuth2Info", "GoogleCloudStorageUserInfo"],
        SignInService::YouTube => &["YouTubeOAuth2Info", "YouTubeUserInfo"],
        SignInService::Flickr => &["FlickrOAuthInfo"],
        SignInService::Photobucket => &["PhotobucketOAuthInfo", "PhotobucketAccountInfo"],
    }
}

/// Disconnect: the account's members cleared (`signin::disconnect`).
fn clear_members(service: SignInService) -> Vec<(Path, Value)> {
    oauth_members(service).iter().map(|m| (path(&[m]), Value::Null)).collect()
}

/// `names` as `config` has them, to set on the window's config.
fn members(config: &UploadersConfig, names: &[&str]) -> Vec<(Path, Value)> {
    let v = serde_json::to_value(config).unwrap_or(Value::Null);
    names.iter().map(|n| (path(&[n]), v[*n].clone())).collect()
}

/// "{0} device(s)" (`PushbulletAccount.Reload`).
fn device_count(count: usize) -> String {
    match count {
        0 => u::DestinationSettings_No_devices_loaded().to_owned(),
        1 => u::DestinationSettings_Device_count_singular().replace("{0}", "1"),
        n => u::DestinationSettings_Device_count_plural().replace("{0}", &n.to_string()),
    }
}

/// A folder's name in the remote folder list (`Unnamed folder` without).
fn folder_name(item: &Value) -> String {
    item["name"].as_str().map(str::to_owned).unwrap_or_else(|| u::DestinationSettings_Unnamed_folder().into())
}

/// A `GetPathInfoAsync` item as ShareX keeps it (`OneDriveFileInfo`).
fn one_drive_item(v: Value) -> Value {
    json!({"id": v["id"], "name": v["name"], "webUrl": v["webUrl"]})
}

/// A `GetFilesAsync` entry as ShareX keeps it (`BoxFileEntry`).
fn box_entry(v: Value) -> Value {
    json!({"type": v["type"], "id": v["id"], "sequence_id": v["sequence_id"], "etag": v["etag"], "name": v["name"], "shared_link": v["shared_link"], "parent": v["parent"]})
}

/// Sets members of the config (`DestinationMember.SetValue`).
fn apply(config: &mut UploadersConfig, sets: Vec<(Path, Value)>) {
    if sets.is_empty() {
        return;
    }
    let mut v = match serde_json::to_value(&*config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Destination settings: {e}");
            return;
        }
    };
    for (p, value) in sets {
        set_at(&mut v, &p, value);
    }
    match serde_json::from_value(v) {
        Ok(c) => *config = c,
        Err(e) => eprintln!("Destination settings: {e}"),
    }
}

// ---------------------------------------------------------------------------
// `--render destination-<page> --state <state>`: states drawn from made-up
// data (nothing signs in): what a card shows once ShareX's code has run.

/// A made-up OAuth 2 account that `CheckOAuth` accepts.
fn fixture_oauth2() -> Value {
    json!({
        "Client_ID": "fixture-client",
        "Client_Secret": "fixture-secret",
        "Token": {"access_token": "fixture-access", "refresh_token": "fixture-refresh", "expires_in": 3600, "token_type": "bearer", "scope": null, "ExpireDate": "2099-01-01T00:00:00Z"},
        "Proof": null
    })
}

/// Made-up folders of a remote folder card.
fn fixture_folders(page: &str, names: &[&str]) -> Vec<Value> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let item = json!({"id": format!("fixture-{i}"), "name": n, "type": "folder"});
            if page == "box" { box_entry(item) } else { one_drive_item(item) }
        })
        .collect()
}

fn mega_folder(id: &str, name: &str) -> MegaFolderInfo {
    MegaFolderInfo { id: Some(id.into()), name: Some(name.into()) }
}

/// The config a state is drawn with.
pub fn render_config(page: &str, state: Option<&str>) -> UploadersConfig {
    let mut config = UploadersConfig::default();
    let mut sets: Vec<(Path, Value)> = Vec::new();
    let state = state.unwrap_or("");
    let albums = json!([{"id": "fixture1", "title": "Fixture album one"}, {"id": "fixture2", "title": "Fixture album two"}]);
    match (page, state) {
        ("imgur", "connected-display") => {
            sets.push((path(&["ImgurOAuth2Info"]), fixture_oauth2()));
            sets.push((path(&["ImgurAlbumList"]), albums.clone()));
            sets.push((path(&["ImgurSelectedAlbum"]), albums[0].clone()));
        }
        ("imgur", "cached-list") => sets.push((path(&["ImgurAlbumList"]), albums.clone())),
        ("imgur", "selected") => {
            sets.push((path(&["ImgurAlbumList"]), albums.clone()));
            sets.push((path(&["ImgurSelectedAlbum"]), albums[1].clone()));
        }
        (p, "connected-display") if SignInService::from_page_id(p).is_some_and(|s| s.flow() != signin::SignInFlow::OAuth1) => {
            let service = SignInService::from_page_id(p).expect("service");
            let m = oauth_members(service);
            sets.push((path(&[m[0]]), fixture_oauth2()));
            if m.len() > 1 {
                sets.push((path(&[m[1]]), json!({"sub": "fixture", "name": "Reference Fixture", "given_name": "Reference", "picture": null, "locale": "en"})));
            }
        }
        ("flickr" | "photobucket", "connected-display") => {
            let member = if page == "flickr" { "FlickrOAuthInfo" } else { "PhotobucketOAuthInfo" };
            let info = json!({"Description": null, "OAuthVersion": "1.0", "ConsumerKey": "fixture-key", "ConsumerSecret": "fixture-secret",
                "ConsumerPrivateKey": null, "SignatureMethod": 0, "AuthToken": null, "AuthSecret": null, "AuthVerifier": null,
                "UserToken": "fixture-token", "UserSecret": "fixture-user-secret"});
            sets.push((path(&[member]), info));
            if page == "photobucket" {
                sets.push((path(&["PhotobucketAccountInfo"]), json!({"Username": "fixture", "Subdomain": "fixture", "AlbumID": "fixture", "AlbumList": ["fixture"], "ActiveAlbumID": 0})));
            }
        }
        ("photobucket", "empty" | "cached-list" | "selected" | "album-path") => {
            let list = if state == "empty" { json!([]) } else { json!(["fixture", "fixture/Screenshots", "fixture/Recordings"]) };
            let active = if state == "selected" { 1 } else { 0 };
            sets.push((path(&["PhotobucketAccountInfo"]), json!({"Username": "fixture", "Subdomain": "fixture", "AlbumID": "fixture", "AlbumList": list, "ActiveAlbumID": active})));
        }
        ("imageshack", "connected-display") => sets.push((path(&["ImageShackSettings", "Auth_token"]), json!("fixture-token"))),
        ("imageshack", "filled") => {
            sets.push((path(&["ImageShackSettings", "Username"]), json!("fixture")));
            sets.push((path(&["ImageShackSettings", "Password"]), json!("fixture-password")));
        }
        ("pastebin", "connected-display") => sets.push((path(&["PastebinSettings", "UserKey"]), json!("fixture-user-key"))),
        ("pastebin", "filled") => {
            sets.push((path(&["PastebinSettings", "Username"]), json!("fixture")));
            sets.push((path(&["PastebinSettings", "Password"]), json!("fixture-password")));
        }
        ("puush", "api-key-retrieved") => sets.push((path(&["PuushAPIKey"]), json!("fixture-api-key"))),
        ("lobfile", "api-key-retrieved") => sets.push((path(&["LithiioSettings", "UserAPIKey"]), json!("fixture-api-key"))),
        ("mega", "filled") => {
            sets.push((path(&["MegaEmail"]), json!("someone@example.com")));
            sets.push((path(&["MegaPassword"]), json!("fixture-password")));
        }
        ("mega", "connected-display" | "loaded-children" | "nested" | "selected-child" | "loading") => {
            sets.push((path(&["MegaEmail"]), json!("someone@example.com")));
            sets.push((path(&["MegaSessionID"]), json!("fixture-session")));
            sets.push((path(&["MegaMasterKey"]), json!("AAAAAAAAAAAAAAAAAAAAAA")));
            if state == "selected-child" {
                sets.push((path(&["MegaSelectedFolder"]), json!({"ID": "fixture-1", "Name": "Screenshots"})));
            }
        }
        ("pushbullet", "filled") => sets.push((path(&["PushbulletSettings", "UserAPIKey"]), json!("fixture-api-key"))),
        ("pushbullet", "cached-menu" | "selected") => {
            sets.push((path(&["PushbulletSettings", "UserAPIKey"]), json!("fixture-api-key")));
            sets.push((path(&["PushbulletSettings", "DeviceList"]), json!([{"Key": "fixture-phone", "Name": "Fixture phone"}, {"Key": "fixture-laptop", "Name": "Fixture laptop"}])));
            sets.push((path(&["PushbulletSettings", "SelectedDevice"]), json!(if state == "selected" { 1 } else { 0 })));
        }
        ("onedrive" | "box", "saved-folder") => {
            let member = if page == "box" { "BoxSelectedFolder" } else { "OneDriveV2SelectedFolder" };
            sets.push((path(&[member]), fixture_folders(page, &["Screenshots"]).remove(0)));
        }
        _ => {}
    }
    apply(&mut config, sets);
    config
}

impl DestinationSettingsWindow {
    /// What the window itself holds in a state: typed text, a status line,
    /// a folder list, MEGA's tree. `Err` for a state the page hasn't.
    pub fn seed_render_state(&mut self, page: &str, state: &str) -> Result<(), String> {
        let account = format!("{page}-account");
        let service = SignInService::from_page_id(page);
        let status = |text: &str, c: Option<bool>| (text.to_owned(), c);
        match state {
            "" | "default" | "defaults" | "disconnected" | "empty" | "connected-display" | "collapsed-root" | "selected-root" | "root" | "saved-folder"
            | "cached-list" | "selected" | "cached-menu" => {}
            "code-entered" if service.is_some_and(|s| s.flow() != signin::SignInFlow::Loopback) => {
                self.local.insert(format!("{page}-code"), "fixture-code-1234".into());
            }
            "authorization-opened" if service.is_some_and(|s| s.flow() != signin::SignInFlow::Loopback) => {
                self.status.insert(account, status(u::DestinationSettings_Authorization_page_opened(), None));
            }
            "authorization-failed" if service.is_some_and(|s| s.flow() != signin::SignInFlow::Loopback) => {
                self.local.insert(format!("{page}-code"), "fixture-code-1234".into());
                self.status.insert(account, status(u::DestinationSettings_Authorization_failed(), Some(false)));
            }
            "token-refresh-failed" if service.is_some_and(|s| s.flow() == signin::SignInFlow::CodePaste) => {
                self.status.insert(account, status(u::DestinationSettings_Token_refresh_failed(), Some(false)));
            }
            "login-failed" if matches!(page, "imageshack" | "pastebin" | "puush" | "lobfile") => {
                self.status.insert(account, status(u::DestinationSettings_Login_failed(), Some(false)));
            }
            "api-key-retrieved" if matches!(page, "puush" | "lobfile") => {
                self.local.insert(format!("{page}-username"), "someone@example.com".into());
                self.status.insert(account, status(u::DestinationSettings_API_key_retrieved(), Some(true)));
            }
            "filled" if matches!(page, "puush" | "lobfile") => {
                self.local.insert(format!("{page}-username"), "someone@example.com".into());
                self.local.insert(format!("{page}-password"), "fixture-password".into());
            }
            "filled" => {}
            "not-connected" if page == "imgur" => {
                self.status.insert("imgur-albums".into(), status(u::DestinationSettings_Connect_Imgur_account_first(), None));
            }
            "album-path" if page == "photobucket" => {
                self.local.insert("photobucket-album-path".into(), "fixture/New album".into());
            }
            "device-error" if page == "pushbullet" => {
                self.status.insert("pushbullet-account".into(), status("The remote server returned an error: (401) Unauthorized.", None));
            }
            "listed" | "nested" | "loading" | "no-child-folders" | "error" if matches!(page, "onedrive" | "box") => {
                let root = folder_root(page);
                let mut b = FolderBrowser::new(root.clone());
                let browse = format!("{page}-browse");
                match state {
                    "listed" => {
                        b.items = fixture_folders(page, &["Documents", "Pictures", "Screenshots"]);
                        self.status.insert(browse, status(&u::DestinationSettings_Folder_count_plural().replace("{0}", "3"), None));
                    }
                    "nested" => {
                        let parent = fixture_folders(page, &["Pictures"]).remove(0);
                        b.history.push(root);
                        b.current = parent;
                        b.items = fixture_folders(page, &["2026", "Screenshots"]);
                        b.selected = Some(1);
                        self.status.insert(format!("{page}-folder"), status("Screenshots", None));
                        self.status.insert(browse, status(&u::DestinationSettings_Folder_count_plural().replace("{0}", "2"), None));
                    }
                    "no-child-folders" => {
                        self.status.insert(browse, status(u::DestinationSettings_No_child_folders(), None));
                    }
                    "error" => {
                        self.status.insert(browse, status("An error occurred while sending the request.", None));
                    }
                    _ => {}
                }
                self.browsers.insert(page.to_owned(), b);
            }
            "loaded-children" | "nested" | "selected-child" | "loading" if page == "mega" => {
                self.mega.expanded.insert(String::new());
                if state == "loading" {
                    self.mega.loading.insert(String::new());
                } else {
                    self.mega.children.insert(
                        String::new(),
                        vec![mega_folder("fixture-0", "Documents"), mega_folder("fixture-1", "Screenshots"), mega_folder("fixture-2", "Shared")],
                    );
                }
                if state == "nested" {
                    self.mega.expanded.insert("fixture-1".into());
                    self.mega.children.insert("fixture-1".into(), vec![mega_folder("fixture-10", "2026"), mega_folder("fixture-11", "Archive")]);
                    self.mega.expanded.insert("fixture-11".into());
                }
            }
            other => return Err(format!("no state {other} on destination page {page}")),
        }
        Ok(())
    }

    /// The page scrolled to `offset` (`--scroll`).
    pub fn set_scroll(&mut self, offset: f32) {
        self.scroll.insert(self.page, offset);
    }
}

impl Dialog for DestinationSettingsWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(u::DestinationSettingsWindow_ShareX_Destination_settings()), 1100.0, 760.0, 880.0, 620.0)
    }

    fn single(&self) -> Option<&'static str> {
        Some("destination-settings")
    }

    fn service(&self) -> Option<&str> {
        self.service.as_deref()
    }

    fn navigate(&mut self, service: &str) {
        self.navigate_to_service(service);
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        // `onClosed`: `SettingManager.SaveUploadersConfigAsync`.
        vec![Effect::SaveUploadersConfig]
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx) -> Vec<Effect> {
        let store = cx.uploaders.clone();
        let mut guard = store.wait();
        let config = guard.as_mut().expect("loaded");
        if !std::mem::replace(&mut self.normalized, true) {
            Self::normalize(config);
        }
        let j = serde_json::to_value(&*config).unwrap_or(Value::Null);
        // `OnWindowKeyDown` (ShareX #9304): Escape closes the window.
        if sharex_ui::popup::escape_pressed(ui.ctx()) {
            self.effects.push(Effect::Close);
        }
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        let open = ui.ctx().data_mut(|d| d.remove_temp::<bool>(Id::new("settings-any-combo-next"))).unwrap_or(false);
        ui.ctx().data_mut(|d| d.insert_temp(Id::new("settings-any-combo"), open));
        let terms: Vec<String> = self.search.split(' ').map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect();
        let visible = self.visible(ui, config, &j, &terms);
        if !visible[self.page.0].get(self.page.1).copied().unwrap_or(false)
            && let Some(first) = visible.iter().enumerate().find_map(|(c, v)| v.iter().position(|x| *x).map(|i| (c, i)))
        {
            self.page = first;
        }
        self.navigation(ui, full, &visible);
        let column = Rect::from_min_max(pos2(full.min.x + NAV_WIDTH, full.min.y), full.max);
        let mut sets = self.poll_jobs(&j);
        for act in std::mem::take(&mut self.queued) {
            sets.extend(self.act(&j, config, act));
        }
        if visible.iter().flatten().any(|v| *v) {
            let cat = &categories()[self.page.0];
            let def = &cat.pages[self.page.1];
            let blocks = self.build(def, &j, config);
            let ev = self.page(ui, column, &blocks, &terms, def, &cat.icon);
            for (bind, input) in ev.inputs {
                sets.extend(self.input(&j, config, bind, input));
            }
            for act in ev.acts {
                sets.extend(self.act(&j, config, act));
            }
        }
        if self.busy() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        if !sets.is_empty() {
            apply(config, sets);
            ui.ctx().request_repaint();
        }
        std::mem::take(&mut self.effects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_parses_and_matches_services() {
        assert_eq!(categories().len(), 4);
        assert_eq!(categories().iter().map(|c| c.pages.len()).sum::<usize>(), 48);
        let id = |s: &str| find_service(s).map(|(c, i)| categories()[c].pages[i].id.as_str());
        assert_eq!(id("Imgur"), Some("imgur"));
        assert_eq!(id("Amazon S3"), Some("amazon-s3"));
        assert_eq!(id("FTPS"), Some("ftp"));
        assert_eq!(id("Paste.ee"), Some("paste-ee"));
        assert_eq!(id("bit.ly"), Some("bitly"));
        assert_eq!(id("Custom image uploader"), None);
    }

    #[test]
    fn labels_format_as_sharex_does() {
        assert_eq!(format_label("AccessKeyID"), "Access Key ID");
        assert_eq!(format_label("UseCustomCNAME"), "Use Custom CNAME");
        assert_eq!(format_label("URLPreview"), "URL Preview");
        assert_eq!(format_label("RemoveFileExtensionOn"), "Remove File Extension On");
    }

    #[test]
    fn members_are_set_through_json() {
        let mut c = UploadersConfig::default();
        apply(&mut c, vec![(path(&["ImgurDirectLink"]), json!(false)), (path(&["AmazonS3Settings", "Bucket"]), json!("b"))]);
        assert!(!c.ImgurDirectLink);
        assert_eq!(c.AmazonS3Settings.Bucket.as_deref(), Some("b"));
    }

    #[test]
    fn selections_are_clamped_as_the_builder_does() {
        let mut c = UploadersConfig::default();
        DestinationSettingsWindow::normalize(&mut c);
        assert_eq!(c.FTPSelectedImage, -1);
        assert_eq!(c.LocalhostSelectedFiles, -1);
        assert_eq!(c.PushbulletSettings["SelectedDevice"], json!(-1));
    }

    #[test]
    fn ftp_preview_urls() {
        let a = json!({"Host": "ftp.example.com", "Port": 21, "SubFolderPath": "shots", "BrowserProtocol": "https",
            "HttpHomePath": "", "HttpHomePathAutoAddSubFolderPath": true, "HttpHomePathNoExtension": false});
        assert_eq!(ftp_preview(&a), "https://example.com/shots/example.png");
        let b = json!({"Host": "h", "SubFolderPath": "s", "BrowserProtocol": "http", "HttpHomePath": "cdn.example.com/i",
            "HttpHomePathAutoAddSubFolderPath": false, "HttpHomePathNoExtension": true});
        assert_eq!(ftp_preview(&b), "http://cdn.example.com/i/example");
    }

    // --- The cards' buttons, against the 127.0.0.1 mock ---------------------
    //
    // Every service's request goes to `mock_server.py` through the URL
    // rewriter; pages to open are recorded, never opened. Keys and accounts
    // are made up.

    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use crate::test_mock::{Mock, ok, rewritten, text_ok};

    #[derive(Default)]
    struct Recorder(Mutex<Vec<String>>);

    impl UrlOpener for Recorder {
        fn open(&self, url: &str) {
            self.0.lock().unwrap().push(url.to_owned());
        }
    }

    impl Recorder {
        fn urls(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }
    }

    fn window(page: &str) -> (DestinationSettingsWindow, Arc<Recorder>) {
        let recorder = Arc::new(Recorder::default());
        let mut w = DestinationSettingsWindow::new(None).with_opener(recorder.clone());
        assert!(w.select_page(page));
        (w, recorder)
    }

    /// The jobs and the buttons they press run until they're done, their
    /// changes applied, as the window's frames do.
    fn settle(w: &mut DestinationSettingsWindow, config: &mut UploadersConfig) {
        let start = Instant::now();
        while !w.jobs.is_empty() || !w.queued.is_empty() {
            assert!(start.elapsed() < Duration::from_secs(30), "a job didn't finish");
            let j = serde_json::to_value(&*config).unwrap();
            let mut sets = w.poll_jobs(&j);
            for act in std::mem::take(&mut w.queued) {
                sets.extend(w.act(&j, config, act));
            }
            apply(config, sets);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn press(w: &mut DestinationSettingsWindow, config: &mut UploadersConfig, act: Act) {
        let j = serde_json::to_value(&*config).unwrap();
        let sets = w.act(&j, config, act);
        apply(config, sets);
        settle(w, config);
    }

    fn status(w: &DestinationSettingsWindow, key: &str) -> Option<(String, Option<bool>)> {
        w.status.get(key).cloned()
    }

    /// An account `CheckOAuth` accepts, its token good until 2099.
    fn account() -> OAuth2Info {
        serde_json::from_value(json!({
            "Client_ID": "made-up-client", "Client_Secret": "made-up-secret",
            "Token": {"access_token": "made-up-access", "refresh_token": "made-up-refresh", "expires_in": 3600, "token_type": "bearer", "ExpireDate": "2099-01-01T00:00:00Z"}
        }))
        .unwrap()
    }

    #[test]
    fn code_paste_opens_the_page_then_completes_with_the_code() {
        let mock = Mock::start();
        let _r = rewritten(&mock);
        let (mut w, opened) = window("imgur");
        let mut config = UploadersConfig::default();
        // Open authorization page: Imgur's page is made locally.
        press(&mut w, &mut config, Act::OAuthOpen(SignInService::Imgur));
        let urls = opened.urls();
        assert_eq!(urls.len(), 1);
        assert!(urls[0].starts_with("https://api.imgur.com/oauth2/authorize?client_id"), "{}", urls[0]);
        assert!(urls[0].ends_with("&response_type=pin"));
        assert!(config.ImgurOAuth2Info.is_some(), "the client is stored");
        assert_eq!(status(&w, "imgur-account"), Some((u::DestinationSettings_Authorization_page_opened().into(), None)));
        // Complete authorization with the PIN.
        mock.reset(json!({"responses": [ok(r#"{"access_token":"made-up-new","refresh_token":"made-up-r","expires_in":3600,"token_type":"bearer"}"#)]}));
        w.local.insert("imgur-code".into(), "123456".into());
        press(&mut w, &mut config, Act::OAuthComplete(SignInService::Imgur));
        let log = mock.log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].target, "/__/https/api.imgur.com/oauth2/token");
        assert!(log[0].body_text().contains("123456"));
        assert_eq!(status(&w, "imgur-account"), Some((u::DestinationSettings_Connected().into(), Some(true))));
        let token = config.ImgurOAuth2Info.as_ref().and_then(|i| i.Token.clone()).unwrap();
        assert_eq!(token.access_token.as_str(), "made-up-new");
        // A refused code.
        mock.reset(json!({"responses": [{"status": 400, "reason": "Bad Request", "headers": [["Content-Type", "application/json"]], "body": "{\"error\":\"invalid_grant\"}"}]}));
        press(&mut w, &mut config, Act::OAuthComplete(SignInService::Imgur));
        assert_eq!(status(&w, "imgur-account").map(|s| s.1), Some(Some(false)));
        // An empty code doesn't ask the service.
        mock.reset(json!({"responses": []}));
        w.local.insert("imgur-code".into(), "  ".into());
        press(&mut w, &mut config, Act::OAuthComplete(SignInService::Imgur));
        assert!(mock.log().is_empty());
        assert_eq!(status(&w, "imgur-account"), Some((u::DestinationSettings_Authorization_failed().into(), Some(false))));
    }

    #[test]
    fn refresh_token_where_the_service_cant() {
        let (mut w, _) = window("bitly");
        let mut config = UploadersConfig { BitlyOAuth2Info: Some(account()), ..Default::default() };
        press(&mut w, &mut config, Act::OAuthRefresh(SignInService::Bitly));
        assert_eq!(status(&w, "bitly-account"), Some((u::DestinationSettings_Token_refresh_failed().into(), Some(false))));
        // Disconnect forgets the account; the status follows the config.
        let j = serde_json::to_value(&config).unwrap();
        let sets = w.act(&j, &config, Act::Set(clear_members(SignInService::Bitly), Some("bitly-account".into())));
        apply(&mut config, sets);
        assert!(config.BitlyOAuth2Info.is_none());
        assert_eq!(status(&w, "bitly-account"), None);
    }

    #[test]
    fn google_sign_in_through_the_loopback_listener() {
        let mock = Mock::start();
        let _r = rewritten(&mock);
        mock.reset(json!({"responses": [
            ok(r#"{"access_token":"made-up-google","refresh_token":"made-up-r","expires_in":3600,"token_type":"Bearer"}"#),
            ok(r#"{"sub":"1","name":"Made Up User","given_name":"Made","locale":"en"}"#),
        ]}));
        let (mut w, opened) = window("google-drive");
        let mut config = UploadersConfig::default();
        let j = serde_json::to_value(&config).unwrap();
        let sets = w.act(&j, &config, Act::Loopback(SignInService::GoogleDrive));
        apply(&mut config, sets);
        // The waiting window opens over it.
        assert!(matches!(w.effects.as_slice(), [Effect::OpenDialog(_)]));
        // The browser would come back to the listener with the code.
        let start = Instant::now();
        let url = loop {
            if let Some(u) = opened.urls().pop() {
                break u;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "no page opened");
            std::thread::sleep(Duration::from_millis(5));
        };
        let query = |k: &str| url.split(['?', '&']).find_map(|p| p.strip_prefix(&format!("{k}="))).map(|v| sharex_uploaders::url_helpers::url_decode(v, 1)).unwrap();
        let redirect = query("redirect_uri");
        let state = query("state");
        let addr = redirect.trim_start_matches("http://").trim_end_matches('/').to_owned();
        let mut s = std::net::TcpStream::connect(addr.as_str()).unwrap();
        use std::io::{Read, Write};
        write!(s, "GET /?code=made-up-code&state={state} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").unwrap();
        let mut page = String::new();
        let _ = s.read_to_string(&mut page);
        settle(&mut w, &mut config);
        let log = mock.log();
        assert_eq!(log[0].target, "/__/https/oauth2.googleapis.com/token");
        assert!(log[0].body_text().contains("made-up-code"));
        let token = config.GoogleDriveOAuth2Info.as_ref().and_then(|i| i.Token.clone()).unwrap();
        assert_eq!(token.access_token.as_str(), "made-up-google");
        assert_eq!(config.GoogleDriveUserInfo.as_ref().and_then(|u| u.name.clone()).as_deref(), Some("Made Up User"));
    }

    #[test]
    fn a_cancelled_google_sign_in_forgets_the_account() {
        // ShareX's `setAccount(result?.OAuth2Info, result?.UserInfo)` with
        // no result.
        let (mut w, _) = window("youtube");
        let mut config = UploadersConfig { YouTubeOAuth2Info: Some(account()), ..Default::default() };
        let j = serde_json::to_value(&config).unwrap();
        let sets = w.act(&j, &config, Act::Loopback(SignInService::YouTube));
        apply(&mut config, sets);
        let Some(Effect::OpenDialog(mut listener)) = w.effects.pop() else { panic!("no waiting window") };
        // Its Cancel (closing it) stops the listener.
        let store = crate::upload::UploadersConfigStore::loaded(UploadersConfig::default());
        let mut settings = sharex_settings::ApplicationConfig::default();
        listener.closed(&mut crate::dialogs::Cx { settings: &mut settings, uploaders: &store });
        settle(&mut w, &mut config);
        assert!(config.YouTubeOAuth2Info.is_none());
        assert_eq!(status(&w, "youtube-account"), None);
    }

    #[test]
    fn pastebin_and_user_password_logins() {
        let mock = Mock::start();
        let _r = rewritten(&mock);
        // Pastebin: the user key is saved.
        mock.reset(json!({"responses": [text_ok("made-up-user-key")]}));
        let (mut w, _) = window("pastebin");
        let mut config = UploadersConfig::default();
        config.PastebinSettings.Username = Some("made-up".into());
        config.PastebinSettings.Password = sharex_uploaders::dpapi::Secret::new("made-up-password");
        press(&mut w, &mut config, Act::PastebinLogin);
        assert_eq!(mock.log()[0].target, "/__/https/pastebin.com/api/api_login.php");
        assert_eq!(config.PastebinSettings.UserKey.as_str(), "made-up-user-key");
        assert_eq!(status(&w, "pastebin-account"), Some((u::DestinationSettings_Connected().into(), Some(true))));
        // puush: the API key into its box, the password cleared.
        mock.reset(json!({"responses": [text_ok("0,made-up-api-key,,0")]}));
        let (mut w, _) = window("puush");
        w.local.insert("puush-username".into(), "someone@example.com".into());
        w.local.insert("puush-password".into(), "made-up-password".into());
        press(&mut w, &mut config, Act::UserPassword("puush".into(), path(&["PuushAPIKey"])));
        assert_eq!(mock.log()[0].target, "/__/https/puush.me/api/auth");
        assert_eq!(config.PuushAPIKey.as_str(), "made-up-api-key");
        assert_eq!(w.local_text("puush-password"), "");
        assert_eq!(status(&w, "puush-account"), Some((u::DestinationSettings_API_key_retrieved().into(), Some(true))));
        // A refused login.
        mock.reset(json!({"responses": [text_ok("-1")]}));
        press(&mut w, &mut config, Act::UserPassword("puush".into(), path(&["PuushAPIKey"])));
        assert_eq!(status(&w, "puush-account"), Some((u::DestinationSettings_Login_failed().into(), Some(false))));
    }

    #[test]
    fn pushbullet_devices_and_imgur_albums_load() {
        let mock = Mock::start();
        let _r = rewritten(&mock);
        mock.reset(json!({"responses": [ok(r#"{"devices":[{"iden":"d1","nickname":"Phone"},{"iden":"d2","nickname":"Laptop"},{"iden":"d3"}]}"#)]}));
        let (mut w, _) = window("pushbullet");
        let mut config = UploadersConfig::default();
        press(&mut w, &mut config, Act::PushbulletRefresh);
        assert_eq!(mock.log()[0].target, "/__/https/api.pushbullet.com/v2/devices");
        assert_eq!(config.PushbulletSettings["DeviceList"], json!([{"Key": "d1", "Name": "Phone"}, {"Key": "d2", "Name": "Laptop"}]));
        assert_eq!(config.PushbulletSettings["SelectedDevice"], json!(0));
        assert_eq!(status(&w, "pushbullet-account"), None, "the count follows the list");
        assert_eq!(device_count(2), "2 devices loaded");

        // Imgur: not connected says so, without asking.
        mock.reset(json!({"responses": [ok(r#"{"data":[{"id":"a1","title":"One"},{"id":"a2","title":""}],"success":true,"status":200}"#)]}));
        let (mut w, _) = window("imgur");
        press(&mut w, &mut config, Act::ImgurAlbums);
        assert!(mock.log().is_empty());
        assert_eq!(status(&w, "imgur-albums"), Some((u::DestinationSettings_Connect_Imgur_account_first().into(), None)));
        config.ImgurOAuth2Info = Some(account());
        config.ImgurSelectedAlbum = json!({"id": "a2"});
        press(&mut w, &mut config, Act::ImgurAlbums);
        assert!(mock.log()[0].target.starts_with("/__/https/api.imgur.com/3/account/me/albums"));
        assert_eq!(config.ImgurAlbumList.as_array().map(Vec::len), Some(2));
        assert_eq!(config.ImgurSelectedAlbum["id"], json!("a2"), "the chosen album stays chosen");
        assert_eq!(status(&w, "imgur-albums"), None);
    }

    #[test]
    fn remote_folders_browse_open_back_and_root() {
        let mock = Mock::start();
        let _r = rewritten(&mock);
        mock.reset(json!({"responses": [
            ok(r#"{"value":[{"id":"f1","name":"Pictures"},{"id":"f2","name":"Documents"}]}"#),
            ok(r#"{"value":[]}"#),
            ok(r#"{"value":[{"id":"f1","name":"Pictures"}]}"#),
        ]}));
        let (mut w, _) = window("onedrive");
        let mut config = UploadersConfig { OneDriveV2OAuth2Info: Some(account()), ..Default::default() };
        let member = path(&["OneDriveV2SelectedFolder"]);
        let j = serde_json::to_value(&config).unwrap();
        w.build(&categories()[w.page.0].pages[w.page.1], &j, &config);
        press(&mut w, &mut config, Act::Folder("onedrive".into(), member.clone(), FolderAct::Refresh));
        assert_eq!(w.browsers["onedrive"].items.len(), 2);
        assert_eq!(status(&w, "onedrive-browse"), Some((u::DestinationSettings_Folder_count_plural().replace("{0}", "2"), None)));
        // Choosing one makes it the upload folder.
        let j = serde_json::to_value(&config).unwrap();
        let sets = w.input(&j, &config, Bind::FolderSelect("onedrive".into(), member.clone()), Input::Index(0));
        apply(&mut config, sets);
        assert_eq!(config.OneDriveV2SelectedFolder, json!({"id": "f1", "name": "Pictures", "webUrl": null}));
        assert_eq!(status(&w, "onedrive-folder"), Some(("Pictures".into(), None)));
        // Open folder, then Back.
        press(&mut w, &mut config, Act::Folder("onedrive".into(), member.clone(), FolderAct::Open));
        assert_eq!(status(&w, "onedrive-browse"), Some((u::DestinationSettings_No_child_folders().into(), None)));
        press(&mut w, &mut config, Act::Folder("onedrive".into(), member.clone(), FolderAct::Back));
        assert_eq!(w.browsers["onedrive"].items.len(), 1);
        let targets: Vec<String> = mock.log().iter().map(|r| r.target.clone()).collect();
        assert!(targets[0].starts_with("/__/https/graph.microsoft.com/v1.0/me/drive/root/children"), "{}", targets[0]);
        assert!(targets[1].starts_with("/__/https/graph.microsoft.com/v1.0/me/drive/items/f1/children"), "{}", targets[1]);
        // Root: the root is the upload folder again.
        mock.reset(json!({"responses": [ok(r#"{"value":[]}"#)]}));
        press(&mut w, &mut config, Act::Folder("onedrive".into(), member, FolderAct::Root));
        assert_eq!(config.OneDriveV2SelectedFolder, UploadersConfig::default().OneDriveV2SelectedFolder);
        assert!(w.browsers["onedrive"].history.is_empty());
    }

    #[test]
    fn mega_asks_for_the_two_factor_code() {
        let mock = Mock::start();
        let _r = rewritten(&mock);
        // `us0` (a version 1 account), `us` refused with -26.
        mock.reset(json!({"responses": [ok(r#"[{"v":1}]"#), ok("[-26]")]}));
        let (mut w, _) = window("mega");
        let mut config = UploadersConfig { MegaEmail: Some("someone@example.com".into()), ..Default::default() };
        config.MegaPassword = sharex_uploaders::dpapi::Secret::new("made-up-password");
        press(&mut w, &mut config, Act::MegaLogin(None));
        assert!(matches!(w.effects.as_slice(), [Effect::OpenDialog(_)]), "the code is asked");
        assert!(w.two_factor.is_some());
        // The code given: logged in again with it (refused here: -9).
        mock.reset(json!({"responses": [ok(r#"[{"v":1}]"#), ok("[-9]")]}));
        let (tx, rx) = mpsc::channel();
        w.two_factor = Some(rx);
        tx.send(Some("123456".into())).unwrap();
        let j = serde_json::to_value(&config).unwrap();
        let sets = w.poll_jobs(&j);
        apply(&mut config, sets);
        settle(&mut w, &mut config);
        let log = mock.log();
        assert_eq!(log.len(), 2);
        assert!(log[1].body_text().contains(r#""mfa":"123456""#), "{}", log[1].body_text());
        assert_eq!(status(&w, "mega-account").map(|s| s.1), Some(Some(false)));
        assert!(config.MegaSessionID.is_empty());
    }

    #[test]
    fn mega_tree_without_a_session_shows_the_placeholder() {
        // `LoadChildrenAsync` throws "Not connected" before any request.
        let (mut w, _) = window("mega");
        let mut config = UploadersConfig::default();
        let j = serde_json::to_value(&config).unwrap();
        let sets = w.input(&j, &config, Bind::MegaToggle(String::new()), Input::Bool(true));
        apply(&mut config, sets);
        settle(&mut w, &mut config);
        let rows = w.mega.rows("");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].expanded && rows[0].selected);
        assert_eq!((rows[1].id.as_deref(), rows[1].label.as_str(), rows[1].depth), (None, "…", 1));
        // Selecting a loaded folder makes it the upload folder.
        w.mega.children.insert(String::new(), vec![MegaFolderInfo { id: Some("h1".into()), name: Some("Shots".into()) }]);
        let j = serde_json::to_value(&config).unwrap();
        let sets = w.input(&j, &config, Bind::MegaSelect("h1".into()), Input::Bool(true));
        apply(&mut config, sets);
        assert_eq!(config.MegaSelectedFolder, json!({"ID": "h1", "Name": "Shots"}));
        assert!(w.mega.rows("h1")[1].selected);
    }

    #[test]
    fn render_states_exist_for_every_card() {
        for (page, state) in [
            ("imgur", "connected-display"),
            ("imgur", "code-entered"),
            ("google-drive", "connected-display"),
            ("flickr", "connected-display"),
            ("box", "listed"),
            ("onedrive", "nested"),
            ("mega", "nested"),
            ("pushbullet", "cached-menu"),
            ("puush", "api-key-retrieved"),
        ] {
            let (mut w, _) = window(page);
            w.seed_render_state(page, state).unwrap();
            let config = render_config(page, Some(state));
            let j = serde_json::to_value(&config).unwrap();
            let _ = w.build(&categories()[w.page.0].pages[w.page.1], &j, &config);
        }
        assert!(signin::is_connected(SignInService::Flickr, &render_config("flickr", Some("connected-display"))));
        assert!(window("gist").0.seed_render_state("gist", "listed").is_err());
    }
}
