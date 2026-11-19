//! `ShareX/Presentation/CustomUploaderSettings`: the uploader library on
//! the left (search, New, Import, the list, duplicate/export/remove,
//! Export all, Clear all) and the selected uploader's editor on the right
//! (Overview, Request, Body, Response, Test), bound to `UploadersConfig`:
//! every change applies at once. Text boxes of the syntax take a menu of
//! tokens on a right click; `.sxcu` files dropped on the window are
//! imported.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

use egui::{Id, Rect, Vec2, pos2, vec2};
use sharex_strings::sharex as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, CheckBox, ComboBox, ComboItem, TextBox, content_font};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, MenuEntry, Placement};
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{center_y, font, line_height};
use sharex_uploaders::UploadersConfig;
use sharex_uploaders::custom::item::StringMap;
use sharex_uploaders::custom::{CustomUploaderItem, CustomUploaderParser};
use sharex_uploaders::enums::{CustomUploaderBody, CustomUploaderDestinationType as Dest, HttpMethod};
use sharex_uploaders::http::ResponseInfo;

use super::layout;
use super::response::ResponseData;
use super::{Cx, Dialog, Effect, Spec};

/// A section's icon and title.
type Section = (&'static str, fn() -> &'static str);

const SECTIONS: [Section; 5] = [
    (icon::Dashboard, s::CustomUploaderSettingsWindow_Overview),
    (icon::Send, s::CustomUploaderSettingsWindow_Request),
    (icon::Braces, s::CustomUploaderSettingsWindow_Body),
    (icon::Reply, s::CustomUploaderSettingsWindow_Response),
    (icon::Flask, s::CustomUploaderSettingsWindow_Test),
];

/// `InputTokens` / `OutputTokens`: what the syntax menus insert.
const INPUT_TOKENS: [&str; 6] = ["{input}", "{filename}", "{random:input1|input2}", "{select:input1|input2}", "{inputbox:title|default_value}", "{base64:input}"];
const OUTPUT_TOKENS: [&str; 12] = [
    "{response}",
    "{responseurl}",
    "{header:header_name}",
    "{json:path}",
    "{xml:path}",
    "{regex:pattern|group}",
    "{filename}",
    "{random:input1|input2}",
    "{select:input1|input2}",
    "{inputbox:title|default_value}",
    "{outputbox:title|text}",
    "{base64:input}",
];

/// Which key/value list of an uploader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Pairs {
    Parameters,
    Headers,
    Arguments,
}

/// A text field of the selected uploader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Name,
    RequestUrl,
    FileFormName,
    Data,
    Url,
    ThumbnailUrl,
    DeletionUrl,
    ErrorMessage,
    SyntaxResponse,
    SyntaxExpression,
    /// A key/value row's value.
    Value(Pairs, usize),
}

fn field_text(item: &CustomUploaderItem, f: Field) -> String {
    let v = |o: &Option<String>| o.clone().unwrap_or_default();
    match f {
        Field::Name => v(&item.name),
        Field::RequestUrl => v(&item.request_url),
        Field::FileFormName => v(&item.file_form_name),
        Field::Data => v(&item.data),
        Field::Url => v(&item.url),
        Field::ThumbnailUrl => v(&item.thumbnail_url),
        Field::DeletionUrl => v(&item.deletion_url),
        Field::ErrorMessage => v(&item.error_message),
        _ => String::new(),
    }
}

fn set_field(item: &mut CustomUploaderItem, f: Field, text: String) {
    // The editor's setters write `value ?? string.Empty`.
    match f {
        Field::Name => item.name = Some(text),
        Field::RequestUrl => item.request_url = Some(text),
        Field::FileFormName => item.file_form_name = Some(text),
        Field::Data => item.data = Some(text),
        Field::Url => item.url = Some(text),
        Field::ThumbnailUrl => item.thumbnail_url = Some(text),
        Field::DeletionUrl => item.deletion_url = Some(text),
        Field::ErrorMessage => item.error_message = Some(text),
        _ => {}
    }
}

/// What a test sends (`RunTestAsync`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestKind {
    Image,
    Text,
    File,
    UrlShortener,
    UrlSharing,
}

/// `CustomUploaderSettingsViewModel.RunTestAsync`: the app's logo as
/// "Test.png", the text as "Test.txt", or ShareX's website shortened or
/// shared. `None` when there's nothing to test.
pub fn run_test(kind: TestKind, item: CustomUploaderItem, text: Option<String>) -> Option<sharex_uploaders::UploadResult> {
    use sharex_uploaders::custom::{CustomFileUploader, CustomTextUploader, CustomUrlSharer, CustomUrlShortener};
    use sharex_uploaders::uploader::{self, GenericUploader};
    const WEBSITE: &str = "https://getsharex.com";
    let logo = || {
        let image = image::RgbaImage::from_raw(64, 64, crate::app_icon::rgba64().to_vec()).unwrap_or_default();
        let png = sharex_capture::clipboard::png_bytes(&image).unwrap_or_default();
        sharex_uploaders::http::shared_stream(std::io::Cursor::new(png))
    };
    // `result.Errors.Add(uploader.Errors)`.
    let with_errors = |r: Result<Option<sharex_uploaders::UploadResult>, String>, errors: &sharex_uploaders::result::UploaderErrorManager| {
        r.map(|o| {
            o.map(|mut res| {
                res.errors.extend(errors);
                res
            })
        })
    };
    let outcome = match kind {
        TestKind::Image | TestKind::File => {
            let mut u = GenericUploader::File(Box::new(CustomFileUploader::new(item)));
            let r = u.upload(&logo(), "Test.png");
            let errors = u.uploader().errors.clone();
            with_errors(r, &errors)
        }
        TestKind::Text => {
            let text = text.filter(|t| !t.is_empty())?;
            let mut u = CustomTextUploader::new(item);
            let r = uploader::upload_text(&mut u, &text, "Test.txt");
            with_errors(r, &u.base.errors)
        }
        TestKind::UrlShortener => {
            let mut u = CustomUrlShortener::new(item);
            let r = uploader::shorten_url(&mut u, WEBSITE);
            with_errors(r, &u.base.errors)
        }
        TestKind::UrlSharing => {
            let mut u = CustomUrlSharer::new(item);
            let r = uploader::share_url(&mut u, WEBSITE);
            with_errors(r, &u.base.errors)
        }
    };
    match outcome {
        Ok(r) => r,
        Err(e) => {
            let mut r = sharex_uploaders::UploadResult::default();
            r.errors.add(e);
            Some(r)
        }
    }
}

/// What the response window shows of a test's result.
pub fn response_data(r: &sharex_uploaders::UploadResult) -> ResponseData {
    let v = |o: &Option<String>| o.clone().unwrap_or_default();
    ResponseData {
        url: v(&r.url),
        shortened_url: v(&r.shortened_url),
        thumbnail_url: v(&r.thumbnail_url),
        deletion_url: v(&r.deletion_url),
        is_error: r.is_error(),
        errors: r.errors_to_string().unwrap_or_default(),
        response: v(&r.response),
        response_info: r.response_info.clone(),
    }
}

/// The syntax test overlay's state (`OpenSyntaxTest`).
struct SyntaxTest {
    response_info: ResponseInfo,
    response_text: String,
    expression: String,
}

impl SyntaxTest {
    /// `UpdateSyntaxTestResult`.
    fn result(&self) -> String {
        if self.expression.is_empty() {
            return String::new();
        }
        let mut info = self.response_info.clone();
        info.response_text = Some(self.response_text.clone());
        let parser = CustomUploaderParser { file_name: Some("example.png".into()), response_info: Some(info), url_encode: true, ..Default::default() };
        match parser.parse(&self.expression) {
            Ok(r) => r,
            Err(e) => format!("{}\r\n{e}", s::CustomUploaderSettingsWindow_Error()),
        }
    }
}

type TestResult = Option<sharex_uploaders::UploadResult>;

pub struct CustomUploaderSettingsWindow {
    selected: Option<usize>,
    pub section: usize,
    search: String,
    status: String,
    /// The key/value rows as typed (duplicates and empty names kept), per
    /// uploader and list; the model holds the valid ones.
    rows: HashMap<(usize, Pairs), Vec<(String, String)>>,
    menu: Option<(Flyout<&'static str>, Id, Field)>,
    testing: Option<mpsc::Receiver<TestResult>>,
    last_result: TestResult,
    syntax: Option<SyntaxTest>,
    /// Files dropped since the last frame.
    dropped: Vec<PathBuf>,
    /// The text upload test's text, once its window answers.
    pending_text_test: Option<mpsc::Receiver<Option<String>>>,
    /// "Clear all..." was confirmed.
    pub clear_confirmed: bool,
}

impl CustomUploaderSettingsWindow {
    /// `new CustomUploaderSettingsViewModel(config)`: opens on the image
    /// uploader's selection (`Reload`).
    pub fn new(config: &UploadersConfig) -> Self {
        let n = config.CustomUploadersList.0.len();
        let selected = (n > 0).then(|| config.CustomImageUploaderSelected.clamp(0, n as i32 - 1) as usize);
        Self {
            selected,
            section: 0,
            search: String::new(),
            status: s::CustomUploaderSettingsWindow_ChangesApplied().to_owned(),
            rows: HashMap::new(),
            menu: None,
            testing: None,
            last_result: None,
            syntax: None,
            dropped: Vec::new(),
            pending_text_test: None,
            clear_confirmed: false,
        }
    }

    /// `ApplyFilter`: the uploaders whose title, host or request URL has
    /// the search text.
    fn filtered(&self, list: &[CustomUploaderItem]) -> Vec<usize> {
        let q = self.search.trim().to_lowercase();
        list.iter()
            .enumerate()
            .filter(|(_, u)| {
                q.is_empty()
                    || u.display_name().to_lowercase().contains(&q)
                    || host(u).to_lowercase().contains(&q)
                    || u.request_url.clone().unwrap_or_default().to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// `Add`: appended, selected, the search cleared.
    fn add(&mut self, config: &mut UploadersConfig, item: CustomUploaderItem) {
        self.search.clear();
        config.CustomUploadersList.0.push(item);
        self.selected = Some(config.CustomUploadersList.0.len() - 1);
        normalize_selections(config);
    }

    /// `ImportFiles`.
    pub fn import(&mut self, config: &mut UploadersConfig, paths: &[PathBuf]) -> usize {
        let mut imported = 0;
        for path in paths.iter().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("sxcu"))) {
            let text = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => {
                    self.status = e.to_string();
                    continue;
                }
            };
            match CustomUploaderItem::from_json(text.trim_start_matches('\u{feff}')) {
                Ok(Some(mut item)) => {
                    if let Err(e) = item.check_backward_compatibility() {
                        self.status = e;
                        continue;
                    }
                    self.add(config, item);
                    imported += 1;
                }
                Ok(None) => {}
                Err(e) => self.status = e,
            }
        }
        if imported > 0 {
            self.status = s::CustomUploaderSettingsWindow_ImportedCount().replace("{0}", &imported.to_string());
        }
        imported
    }

    fn rows(&mut self, index: usize, which: Pairs, item: &CustomUploaderItem) -> &mut Vec<(String, String)> {
        self.rows.entry((index, which)).or_insert_with(|| {
            let map = match which {
                Pairs::Parameters => &item.parameters,
                Pairs::Headers => &item.headers,
                Pairs::Arguments => &item.arguments,
            };
            map.iter().flatten().map(|(k, v)| (k.clone(), v.clone().unwrap_or_default())).collect()
        })
    }
}

fn host(u: &CustomUploaderItem) -> String {
    sharex_uploaders::url_helpers::get_host_name(u.request_url.as_deref().unwrap_or(""))
}

/// `CustomUploaderKeyValueCollection.Synchronize`: the first row of each
/// non-empty name, as a dictionary.
fn synchronize(rows: &[(String, String)]) -> StringMap {
    let mut out: StringMap = Vec::new();
    for (k, v) in rows {
        if !k.is_empty() && !out.iter().any(|(e, _)| e == k) {
            out.push((k.clone(), Some(v.clone())));
        }
    }
    out
}

/// `NormalizeSelections`.
fn normalize_selections(c: &mut UploadersConfig) {
    let n = c.CustomUploadersList.0.len() as i32;
    let clamp = |v: i32| if n == 0 { 0 } else { v.clamp(0, n - 1) };
    c.CustomImageUploaderSelected = clamp(c.CustomImageUploaderSelected);
    c.CustomTextUploaderSelected = clamp(c.CustomTextUploaderSelected);
    c.CustomFileUploaderSelected = clamp(c.CustomFileUploaderSelected);
    c.CustomURLShortenerSelected = clamp(c.CustomURLShortenerSelected);
    c.CustomURLSharingServiceSelected = clamp(c.CustomURLSharingServiceSelected);
}

/// `FixSelectionsAfterRemoval`.
fn fix_selections_after_removal(c: &mut UploadersConfig, removed: i32) {
    let reset = (c.CustomUploadersList.0.len() as i32 - 1).max(0);
    let fix = |v: i32| {
        if v == removed {
            reset
        } else if v > removed {
            v - 1
        } else {
            v
        }
    };
    c.CustomImageUploaderSelected = fix(c.CustomImageUploaderSelected);
    c.CustomTextUploaderSelected = fix(c.CustomTextUploaderSelected);
    c.CustomFileUploaderSelected = fix(c.CustomFileUploaderSelected);
    c.CustomURLShortenerSelected = fix(c.CustomURLShortenerSelected);
    c.CustomURLSharingServiceSelected = fix(c.CustomURLSharingServiceSelected);
}

/// The fonts of the window's styles.
fn section_title_font() -> egui::FontId {
    font(20.0, 600)
}
fn card_title_font() -> egui::FontId {
    font(14.0, 600)
}
fn field_label_font() -> egui::FontId {
    font(12.0, 600)
}
fn mono() -> egui::FontId {
    egui::FontId::new(14.0, egui::FontFamily::Monospace)
}

/// `TextBox.dense` (`MinHeight="30"`, `Padding="7,4"`), monospace for
/// `syntax-input`.
fn dense(syntax: bool) -> TextBox<'static> {
    TextBox { padding: Thickness::hv(7.0, 4.0), font: syntax.then(mono), ..TextBox::default() }
}

/// A `Button.compact` (`MinHeight="30"`, `Padding="10,4"`, the global
/// `Height="40"`) with a 15px icon and/or a text, `Spacing="6"`.
fn compact_button(ui: &egui::Ui, rect: Rect, id: Id, key: Option<&str>, text: &str, primary: bool, enabled: bool) -> bool {
    let p = palette(ui.ctx());
    let b = Button { padding: Thickness::hv(10.0, 4.0), primary, ..Button::default() };
    let r = b.show(ui, rect, id, "", enabled);
    let color = if !enabled {
        sharex_ui::border::group_content(fluent(ui.ctx()).base_medium_low, p.control_background, 0.35, p.background_panel)
    } else if primary {
        p.accent_foreground
    } else if r.hovered() {
        fluent(ui.ctx()).base_high
    } else {
        p.text
    };
    let f = button_font(key.is_some());
    let tw = if text.is_empty() { 0.0 } else { layout::width(ui, text, f.clone()) };
    let iw = if key.is_some() { 15.0 } else { 0.0 };
    let gap = if key.is_some() && !text.is_empty() { 6.0 } else { 0.0 };
    let total = iw + gap + tw;
    let mut x = rect.min.x + ((rect.width() - total) / 2.0).round_ties_even();
    if let Some(k) = key {
        icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(rect, 15.0)), Vec2::splat(15.0)), k, color);
        x += 15.0 + gap;
    }
    if !text.is_empty() {
        layout::text(ui, pos2(x, center_y(rect, line_height(ui, &f))), text, f, color, f32::INFINITY);
    }
    r.clicked()
}

/// A button's text: a `TextBlock` beside an icon (Segoe UI, the button
/// style's Normal weight), or plain string `Content` (Inter).
fn button_font(with_icon: bool) -> egui::FontId {
    if with_icon { font(13.0, 400) } else { content_font(13.0, 400) }
}

fn compact_width(ui: &egui::Ui, key: bool, text: &str) -> f32 {
    let tw = if text.is_empty() { 0.0 } else { layout::width(ui, text, button_font(key)) };
    let iw = if key { 15.0 } else { 0.0 };
    let gap = if key && !text.is_empty() { 6.0 } else { 0.0 };
    2.0 + 20.0 + iw + gap + tw
}

/// `Border.card`: panel background, 6px corners (`Padding="12"`).
fn card(ui: &egui::Ui, rect: Rect) {
    layout::panel(ui, rect, 6.0);
}

impl Dialog for CustomUploaderSettingsWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::CustomUploaderSettingsWindow_Title()), 1120.0, 760.0, 900.0, 620.0)
    }

    fn single(&self) -> Option<&'static str> {
        Some("custom-uploader-settings")
    }

    fn dropped(&mut self, files: Vec<PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        self.dropped.extend(files);
        Vec::new()
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        // `Closed`: `SaveUploadersConfigAsync`, `RefreshMenus`.
        vec![Effect::SaveUploadersConfig]
    }

    fn confirmed(&mut self) {
        self.clear_confirmed = true;
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx) -> Vec<Effect> {
        let store = cx.uploaders.clone();
        let mut guard = store.wait();
        let config = guard.as_mut().expect("loaded");
        let mut out = Vec::new();
        if std::mem::take(&mut self.clear_confirmed) {
            self.clear(config);
        }
        if !self.dropped.is_empty() {
            let files = std::mem::take(&mut self.dropped);
            self.import(config, &files);
        }
        if let Some(rx) = &self.pending_text_test {
            match rx.try_recv() {
                Ok(Some(text)) => {
                    self.pending_text_test = None;
                    let index = config.CustomTextUploaderSelected;
                    self.start_test(config, TestKind::Text, index, Some(text));
                }
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => self.pending_text_test = None,
                Err(mpsc::TryRecvError::Empty) => ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)),
            }
        }
        if let Some(rx) = &self.testing {
            match rx.try_recv() {
                Ok(result) => {
                    self.testing = None;
                    self.status = match &result {
                        None => s::CustomUploaderSettingsWindow_TestCancelled(),
                        Some(r) if r.is_error() => s::CustomUploaderSettingsWindow_TestFailed(),
                        Some(_) => s::CustomUploaderSettingsWindow_TestCompleted(),
                    }
                    .to_owned();
                    if let Some(r) = &result {
                        out.push(Effect::ShowResponse(Box::new(response_data(r))));
                    }
                    self.last_result = result;
                }
                Err(mpsc::TryRecvError::Empty) => ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)),
                Err(_) => self.testing = None,
            }
        }
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        // The library: 260px, toolbar background, a 1px right border.
        let lib = Rect::from_min_size(full.min, vec2(260.0, full.height()));
        ui.painter().rect_filled(lib, 0.0, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_max(pos2(lib.max.x - 1.0, lib.min.y), lib.max), 0.0, p.border);
        self.library(ui, Rect::from_min_max(lib.min + vec2(10.0, 10.0), lib.max - vec2(11.0, 10.0)), config, &mut out);
        let editor = Rect::from_min_max(pos2(lib.max.x + 16.0, full.min.y + 12.0), pos2(full.max.x - 16.0, full.max.y - 10.0));
        self.editor(ui, editor, config, &mut out);
        if self.syntax.is_some() {
            self.syntax_overlay(ui, full);
            if sharex_ui::popup::escape_pressed(ui.ctx()) {
                self.syntax = None;
            }
        }
        if let Some((flyout, id, field)) = &mut self.menu {
            let o = flyout.show(ui, full);
            let (id, field) = (*id, *field);
            if o.closed {
                self.menu = None;
            }
            if let Some(token) = o.command {
                self.insert_token(ui, config, id, field, token);
            }
        }
        out
    }
}

impl CustomUploaderSettingsWindow {
    fn start_test(&mut self, config: &UploadersConfig, kind: TestKind, index: i32, text: Option<String>) {
        let Some(item) = usize::try_from(index).ok().and_then(|i| config.CustomUploadersList.0.get(i)).cloned() else { return };
        if self.testing.is_some() {
            return;
        }
        self.selected = Some(index as usize);
        self.status = s::CustomUploaderSettingsWindow_Testing().replace("{0}", &item.display_name());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(run_test(kind, item, text));
        });
        self.testing = Some(rx);
    }

    /// `InsertToken`: the token replaces the selection, the caret after it.
    fn insert_token(&mut self, ui: &egui::Ui, config: &mut UploadersConfig, id: Id, field: Field, token: &str) {
        use egui::text::{CCursor, CCursorRange};
        let text = match field {
            Field::SyntaxResponse | Field::SyntaxExpression => match &self.syntax {
                Some(st) if field == Field::SyntaxExpression => st.expression.clone(),
                Some(st) => st.response_text.clone(),
                None => return,
            },
            Field::Value(which, row) => {
                let Some(i) = self.selected else { return };
                self.rows.get(&(i, which)).and_then(|r| r.get(row)).map(|r| r.1.clone()).unwrap_or_default()
            }
            f => match self.selected.and_then(|i| config.CustomUploadersList.0.get(i)) {
                Some(item) => field_text(item, f),
                None => return,
            },
        };
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
        let chars: Vec<char> = text.chars().collect();
        let (a, b) = state.cursor.char_range().map_or((chars.len(), chars.len()), |r| {
            let (x, y) = (r.primary.index.0.min(chars.len()), r.secondary.index.0.min(chars.len()));
            (x.min(y), x.max(y))
        });
        let text = chars[..a].iter().collect::<String>() + token + &chars[b..].iter().collect::<String>();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(a + token.chars().count()))));
        state.store(ui.ctx(), id);
        ui.memory_mut(|m| m.request_focus(id));
        match field {
            Field::SyntaxExpression => {
                if let Some(st) = &mut self.syntax {
                    st.expression = text;
                }
            }
            Field::SyntaxResponse => {
                if let Some(st) = &mut self.syntax {
                    st.response_text = text;
                }
            }
            Field::Value(which, row) => {
                let Some(i) = self.selected else { return };
                if let Some(r) = self.rows.get_mut(&(i, which)).and_then(|r| r.get_mut(row)) {
                    r.1 = text;
                }
                self.sync_rows(config, i, which);
            }
            f => {
                if let Some(item) = self.selected.and_then(|i| config.CustomUploadersList.0.get_mut(i)) {
                    set_field(item, f, text);
                }
            }
        }
    }

    fn sync_rows(&mut self, config: &mut UploadersConfig, index: usize, which: Pairs) {
        let Some(rows) = self.rows.get(&(index, which)) else { return };
        let Some(item) = config.CustomUploadersList.0.get_mut(index) else { return };
        let map = Some(synchronize(rows));
        match which {
            Pairs::Parameters => item.parameters = map,
            Pairs::Headers => item.headers = map,
            Pairs::Arguments => item.arguments = map,
        }
    }

    /// A syntax text box: monospace, the token menu on a right click.
    /// Returns whether the text changed.
    #[allow(clippy::too_many_arguments)]
    fn syntax_box(&mut self, ui: &mut egui::Ui, rect: Rect, id: Id, field: Field, text: &mut String, output: bool, multiline: bool) -> bool {
        let before = text.clone();
        let tb = TextBox { multiline, ..dense(true) };
        tb.show(ui, rect, id, text);
        if ui.rect_contains_pointer(rect) && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary)) {
            let tokens: &[&'static str] = if output { &OUTPUT_TOKENS } else { &INPUT_TOKENS };
            let entries = tokens.iter().map(|t| MenuEntry::item(*t, "", *t)).collect();
            let at = ui.ctx().pointer_latest_pos().unwrap_or(rect.min);
            let flyout = Flyout::menu(id.with("tokens"), entries, Rect::from_min_size(at, Vec2::ZERO), Placement::Pointer(at));
            self.menu = Some((flyout, id, field));
        }
        *text != before
    }

    fn library(&mut self, ui: &mut egui::Ui, area: Rect, config: &mut UploadersConfig, out: &mut Vec<Effect>) {
        let p = palette(ui.ctx());
        let count = config.CustomUploadersList.0.len();
        let filtered = self.filtered(&config.CustomUploadersList.0);
        // Row 0: the title and count, the guide button.
        let title_font = font(16.0, 600);
        let lf = field_label_font();
        let mut y = area.min.y;
        layout::text(ui, pos2(area.min.x, y), s::CustomUploaderSettingsWindow_UploaderLibrary(), title_font.clone(), p.text_secondary, area.width() - 40.0);
        let count_text = if self.search.trim().is_empty() {
            let f = if count == 1 { s::CustomUploaderSettingsWindow_UploaderCount() } else { s::CustomUploaderSettingsWindow_UploaderCountPlural() };
            f.replace("{0}", &count.to_string())
        } else {
            s::CustomUploaderSettingsWindow_FilteredUploaderCount().replace("{0}", &filtered.len().to_string()).replace("{1}", &count.to_string())
        };
        let th = line_height(ui, &title_font);
        layout::text(ui, pos2(area.min.x, y + th + 1.0), &count_text, lf.clone(), p.text_secondary, area.width() - 40.0);
        let row0 = th + 1.0 + line_height(ui, &lf);
        let guide = Rect::from_min_size(pos2(area.max.x - 32.0, y + ((row0 - 30.0) / 2.0).round_ties_even()), vec2(32.0, 30.0));
        if compact_button(ui, guide, ui.id().with("guide"), Some(icon::Help), "", false, true) {
            out.push(Effect::OpenUrl("https://getsharex.com/docs/custom-uploader".into()));
        }
        y += row0.max(30.0) + 8.0;
        // Row 1: search.
        let search = Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), 30.0));
        TextBox { padding: Thickness::new(29.0, 4.0, 7.0, 4.0), placeholder: s::CustomUploaderSettingsWindow_SearchUploaders(), ..TextBox::default() }.show(
            ui,
            search,
            ui.id().with("search"),
            &mut self.search,
        );
        icons::paint(ui.painter(), Rect::from_min_size(pos2(search.min.x + 9.0, center_y(search, 15.0)), Vec2::splat(15.0)), icon::Search, p.text);
        y = search.max.y + 8.0;
        // Row 2: New, Import.
        let half = ((area.width() - 5.0) / 2.0).floor();
        let new = Rect::from_min_size(pos2(area.min.x, y), vec2(half, 40.0));
        let import = Rect::from_min_max(pos2(new.max.x + 5.0, y), pos2(area.max.x, y + 40.0));
        if compact_button(ui, new, ui.id().with("new"), Some(icon::Plus), s::CustomUploaderSettingsWindow_New(), true, true) {
            self.add(config, CustomUploaderItem::init());
            self.status = s::CustomUploaderSettingsWindow_NewCreated().to_owned();
        }
        if compact_button(ui, import, ui.id().with("import"), Some(icon::Download), s::CustomUploaderSettingsWindow_Import(), false, true) {
            let files = rfd::FileDialog::new()
                .set_title(s::CustomUploaderSettingsWindow_ImportDialogTitle())
                .add_filter(s::CustomUploaderSettingsWindow_CustomUploaderFileType(), &["sxcu"])
                .pick_files()
                .unwrap_or_default();
            self.import(config, &files);
        }
        y = new.max.y + 8.0;
        // Row 4: the selection's buttons and Export all / Clear all.
        let bottom = Rect::from_min_max(pos2(area.min.x, area.max.y - 86.0), area.max);
        let has_selection = self.selected.is_some();
        let third = ((area.width() - 8.0) / 3.0).floor();
        let mut action = None;
        for (i, k) in [icon::Copy, icon::Upload, icon::Trash].into_iter().enumerate() {
            let x = match i {
                0 => bottom.min.x,
                1 => bottom.min.x + third + 4.0,
                _ => area.max.x - third,
            };
            let w = if i == 1 { area.max.x - third - 4.0 - x } else { third };
            let r = Rect::from_min_size(pos2(x, bottom.min.y), vec2(w, 40.0));
            if compact_button(ui, r, ui.id().with(("sel", i)), Some(k), "", false, has_selection) {
                action = Some(i);
            }
        }
        let half4 = ((area.width() - 4.0) / 2.0).floor();
        let export_all = Rect::from_min_size(pos2(bottom.min.x, bottom.min.y + 46.0), vec2(half4, 40.0));
        let clear = Rect::from_min_max(pos2(export_all.max.x + 4.0, export_all.min.y), pos2(area.max.x, export_all.max.y));
        let has_uploaders = count > 0;
        if compact_button(ui, export_all, ui.id().with("export-all"), None, s::CustomUploaderSettingsWindow_ExportAll(), false, has_uploaders)
            && let Some(folder) = rfd::FileDialog::new().set_title(s::CustomUploaderSettingsWindow_ExportAllDialogTitle()).pick_folder()
        {
            let mut exported = 0;
            for item in &config.CustomUploadersList.0 {
                match std::fs::write(folder.join(item.file_name()), item.to_sxcu()) {
                    Ok(()) => exported += 1,
                    Err(e) => self.status = e.to_string(),
                }
            }
            self.status = s::CustomUploaderSettingsWindow_ExportedCount().replace("{0}", &exported.to_string());
        }
        if compact_button(ui, clear, ui.id().with("clear"), None, s::CustomUploaderSettingsWindow_ClearAll(), false, has_uploaders) {
            out.push(Effect::ConfirmClearCustomUploaders);
        }
        match action {
            Some(0) => {
                if let Some(item) = self.selected.and_then(|i| config.CustomUploadersList.0.get(i)).cloned() {
                    self.add(config, item);
                    self.status = s::CustomUploaderSettingsWindow_Duplicated().to_owned();
                }
            }
            Some(1) => self.export_selected(config),
            Some(2) => self.remove_selected(config),
            _ => {}
        }
        // Row 3: the list (or its empty message).
        let view = Rect::from_min_max(pos2(area.min.x, y), pos2(area.max.x, bottom.min.y - 8.0));
        let list = &config.CustomUploadersList.0;
        let filtered = self.filtered(list);
        if filtered.is_empty() {
            let msg = if list.is_empty() { s::CustomUploaderSettingsWindow_CreateOrImport() } else { s::CustomUploaderSettingsWindow_NoSearchMatches() };
            let f = field_label_font();
            let lines = sharex_ui::controls::wrap_lines(ui, msg, &f, view.width() - 32.0);
            let lh = line_height(ui, &f);
            let mut ty = center_y(view, 24.0 + 8.0 + lh * lines.len() as f32);
            icons::paint(ui.painter(), Rect::from_min_size(pos2((view.center().x - 12.0).round(), ty), Vec2::splat(24.0)), icon::Search, p.text_secondary);
            ty += 32.0;
            for line in &lines {
                sharex_ui::controls::text_centered_at(ui, view.center().x, ty, line, f.clone(), p.text_secondary);
                ty += lh;
            }
            return;
        }
        let tf = font(13.0, 600);
        let stack = line_height(ui, &tf) + line_height(ui, &lf);
        let item_h = 6.0 + stack.max(29.0) + 6.0;
        let pitch = item_h + 2.0;
        let content = pitch * filtered.len() as f32;
        let offset = layout::scroll_offset(ui, ui.id().with("library-scroll"), view, content);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
        child.set_clip_rect(view.intersect(ui.clip_rect()));
        let mut clicked = None;
        for (row, &i) in filtered.iter().enumerate() {
            let item = &list[i];
            let r = Rect::from_min_size(pos2(view.min.x, view.min.y + 1.0 + pitch * row as f32 - offset), vec2(view.width(), item_h));
            if !r.intersects(view) {
                continue;
            }
            let resp = child.interact(r.intersect(view), child.id().with(("uploader", i)), egui::Sense::click());
            let (fg, icon_color) = if self.selected == Some(i) {
                border::border(child.painter(), r, 4.0, 0.0, Some(Brush::accent(ui.ctx())), None);
                (p.accent_foreground, p.accent_foreground)
            } else {
                if resp.hovered() {
                    border::border(child.painter(), r, 4.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
                }
                (p.text_secondary, p.accent_icon)
            };
            let cell = r.shrink2(vec2(7.0, 6.0));
            let tile = Rect::from_min_size(pos2(cell.min.x, center_y(cell, 29.0)), Vec2::splat(29.0));
            border::border(child.painter(), tile, 4.0, 0.0, Some(Brush::Solid(p.background_panel)), None);
            icons::paint(child.painter(), Rect::from_center_size(tile.center(), Vec2::splat(15.0)), icon::Braces, icon_color);
            let tx = tile.max.x + 8.0;
            let ty = center_y(cell, stack);
            layout::text(&child, pos2(tx, ty), &item.display_name(), tf.clone(), fg, cell.max.x - tx);
            layout::text(&child, pos2(tx, ty + line_height(ui, &tf)), &host(item), lf.clone(), fg, cell.max.x - tx);
            if resp.clicked() {
                clicked = Some(i);
            }
        }
        layout::scroll_bar(ui, view, content, offset, 16.0);
        if let Some(i) = clicked {
            self.selected = Some(i);
        }
    }

    /// `ExportSelected`.
    fn export_selected(&mut self, config: &UploadersConfig) {
        let Some(item) = self.selected.and_then(|i| config.CustomUploadersList.0.get(i)) else {
            self.status = s::CustomUploaderSettingsWindow_SelectToExport().to_owned();
            return;
        };
        let path = rfd::FileDialog::new()
            .set_title(s::CustomUploaderSettingsWindow_ExportDialogTitle())
            .set_file_name(item.file_name())
            .add_filter(s::CustomUploaderSettingsWindow_CustomUploaderFileType(), &["sxcu"])
            .save_file();
        let Some(path) = path else { return };
        if item.request_url.as_deref().is_none_or(|u| u.trim().is_empty()) {
            self.status = s::CustomUploaderSettingsWindow_RequestURLRequired().to_owned();
            return;
        }
        if item.destination_type == Dest::None {
            self.status = s::CustomUploaderSettingsWindow_DestinationRequired().to_owned();
            return;
        }
        match std::fs::write(&path, item.to_sxcu()) {
            Ok(()) => self.status = s::CustomUploaderSettingsWindow_ExportedOne().replace("{0}", &item.display_name()),
            Err(e) => self.status = e.to_string(),
        }
    }

    /// `RemoveSelected`.
    fn remove_selected(&mut self, config: &mut UploadersConfig) {
        let Some(index) = self.selected.filter(|&i| i < config.CustomUploadersList.0.len()) else { return };
        config.CustomUploadersList.0.remove(index);
        fix_selections_after_removal(config, index as i32);
        self.rows.clear();
        let n = config.CustomUploadersList.0.len();
        self.selected = (n > 0).then(|| index.min(n - 1));
        self.status = s::CustomUploaderSettingsWindow_Removed().to_owned();
    }

    /// `Clear` after its confirmation.
    pub fn clear(&mut self, config: &mut UploadersConfig) {
        config.CustomUploadersList.0.clear();
        normalize_selections(config);
        self.rows.clear();
        self.selected = None;
        self.status = s::CustomUploaderSettingsWindow_AllRemoved().to_owned();
    }

    fn editor(&mut self, ui: &mut egui::Ui, area: Rect, config: &mut UploadersConfig, out: &mut Vec<Effect>) {
        let p = palette(ui.ctx());
        // The status line: a 1px top border, `Padding="2,8,2,0"`.
        let lf = field_label_font();
        let status_h = 1.0 + 8.0 + line_height(ui, &lf).max(14.0);
        let status = Rect::from_min_max(pos2(area.min.x, area.max.y - status_h), area.max);
        ui.painter().rect_filled(Rect::from_min_size(status.min, vec2(status.width(), 1.0)), 0.0, p.border);
        let srow = Rect::from_min_max(pos2(status.min.x + 2.0, status.min.y + 9.0), pos2(status.max.x - 2.0, status.max.y));
        icons::paint(ui.painter(), Rect::from_min_size(pos2(srow.min.x, center_y(srow, 14.0)), Vec2::splat(14.0)), icon::CircleCheck, p.accent_icon);
        layout::text(ui, pos2(srow.min.x + 21.0, center_y(srow, line_height(ui, &lf))), &self.status, lf.clone(), p.text_secondary, srow.width() - 21.0);
        let body = Rect::from_min_max(area.min, pos2(area.max.x, status.min.y - 10.0));
        let Some(index) = self.selected.filter(|&i| i < config.CustomUploadersList.0.len()) else {
            // No selection: a card with the braces and "Select an uploader".
            card(ui, body);
            let tf = font(17.0, 600);
            let lh = line_height(ui, &tf);
            let mut y = center_y(body, 34.0 + 10.0 + lh);
            icons::paint(ui.painter(), Rect::from_min_size(pos2((body.center().x - 17.0).round(), y), Vec2::splat(34.0)), icon::Braces, p.text_secondary);
            y += 44.0;
            sharex_ui::controls::text_centered_at(ui, body.center().x, y, s::CustomUploaderSettingsWindow_SelectUploaderToEdit(), tf, p.text_secondary);
            return;
        };
        // The header: a 42px tile, the title (20px) and host.
        // The row is as tall as the tile or the text, both centred in it.
        let item = &config.CustomUploadersList.0[index];
        let hf = font(20.0, 600);
        let stack = line_height(ui, &hf) + 1.0 + line_height(ui, &lf);
        let header_row = Rect::from_min_size(body.min, vec2(body.width(), 42.0f32.max(stack)));
        let tile = Rect::from_min_size(pos2(body.min.x, center_y(header_row, 42.0)), Vec2::splat(42.0));
        border::border(ui.painter(), tile, 6.0, 0.0, Some(Brush::Solid(p.background_panel)), None);
        icons::paint(ui.painter(), Rect::from_center_size(tile.center(), Vec2::splat(21.0)), icon::Braces, p.accent_icon);
        let ty = center_y(header_row, stack);
        let tw = body.max.x - tile.max.x - 10.0;
        layout::text(ui, pos2(tile.max.x + 10.0, ty), &item.display_name(), hf.clone(), p.text_secondary, tw);
        layout::text(ui, pos2(tile.max.x + 10.0, ty + line_height(ui, &hf) + 1.0), &host(item), lf.clone(), p.text_secondary, tw);
        let header_h = 42.0f32.max(stack);
        // The section strip, across the editor.
        let item_font = font(13.0, 600);
        let item_h = 6.0 + line_height(ui, &item_font).max(15.0) + 6.0;
        let strip = Rect::from_min_size(pos2(body.min.x, body.min.y + header_h + 10.0), vec2(body.width(), 4.0 + item_h + 4.0));
        border::border(ui.painter(), strip, 6.0, 0.0, Some(Brush::Solid(p.background_toolbar)), None);
        let mut x = strip.min.x + 4.0;
        for (i, (key, title)) in SECTIONS.iter().enumerate() {
            let text = title();
            let w = 11.0 + 15.0 + 6.0 + layout::width(ui, text, item_font.clone()) + 11.0;
            let r = Rect::from_min_size(pos2(x, strip.min.y + 4.0), vec2(w, item_h));
            let resp = ui.interact(r, ui.id().with(("section", i)), egui::Sense::click());
            let color = if self.section == i {
                border::border(ui.painter(), r, 4.0, 0.0, Some(Brush::accent(ui.ctx())), None);
                p.accent_foreground
            } else {
                if resp.hovered() {
                    border::border(ui.painter(), r, 4.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
                }
                p.text_secondary
            };
            icons::paint(ui.painter(), Rect::from_min_size(pos2(r.min.x + 11.0, center_y(r, 15.0)), Vec2::splat(15.0)), key, color);
            layout::text(ui, pos2(r.min.x + 32.0, center_y(r, line_height(ui, &item_font))), text, item_font.clone(), color, f32::INFINITY);
            if resp.clicked() {
                self.section = i;
            }
            x += w + 3.0;
        }
        let content = Rect::from_min_max(pos2(body.min.x, strip.max.y + 10.0), pos2(body.max.x - 4.0, body.max.y));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(content));
        c.set_clip_rect(content.intersect(ui.clip_rect()));
        match self.section {
            0 => self.overview(&mut c, content, config, index),
            1 => self.request(&mut c, content, config, index),
            2 => self.body(&mut c, content, config, index),
            3 => self.response(&mut c, content, config, index),
            _ => self.test(&mut c, content, config, out),
        }
    }

    /// The section's title (`TextBlock.section-title`); returns its height.
    fn section_title(ui: &egui::Ui, area: Rect, text: &str) -> f32 {
        let f = section_title_font();
        layout::text(ui, area.min, text, f.clone(), palette(ui.ctx()).accent_text, area.width());
        line_height(ui, &f)
    }

    fn overview(&mut self, ui: &mut egui::Ui, area: Rect, config: &mut UploadersConfig, index: usize) {
        let p = palette(ui.ctx());
        let (tf, lf) = (card_title_font(), field_label_font());
        let mut y = area.min.y + Self::section_title(ui, area, s::CustomUploaderSettingsWindow_Overview()) + 10.0;
        // Identity: title, the Name label and box, 7px apart.
        let card_h = 2.0 + 24.0 + line_height(ui, &tf) + 7.0 + line_height(ui, &lf) + 7.0 + 30.0;
        let identity = Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), card_h));
        card(ui, identity);
        let inner = identity.shrink(13.0);
        layout::text(ui, inner.min, s::CustomUploaderSettingsWindow_Identity(), tf.clone(), p.text_secondary, inner.width());
        let ly = inner.min.y + line_height(ui, &tf) + 7.0;
        layout::text(ui, pos2(inner.min.x, ly), s::CustomUploaderSettingsWindow_Name(), lf.clone(), p.text_secondary, inner.width());
        let name_rect = Rect::from_min_size(pos2(inner.min.x, ly + line_height(ui, &lf) + 7.0), vec2(inner.width(), 30.0));
        let item = &mut config.CustomUploadersList.0[index];
        let mut name = field_text(item, Field::Name);
        let tb = TextBox { placeholder: s::CustomUploaderSettingsWindow_NamePlaceholder(), ..dense(false) };
        let before = name.clone();
        tb.show(ui, name_rect, ui.id().with(("name", index)), &mut name);
        if name != before {
            set_field(item, Field::Name, name);
        }
        y = identity.max.y + 10.0;
        // Capabilities: five choice cards in two columns, 7px apart.
        let choice_h = 2.0 + 14.0 + 32.0;
        let caps_h = 2.0 + 24.0 + line_height(ui, &tf) + 8.0 + choice_h * 3.0 + 14.0;
        let caps = Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), caps_h));
        card(ui, caps);
        let inner = caps.shrink(13.0);
        layout::text(ui, inner.min, s::CustomUploaderSettingsWindow_Capabilities(), tf.clone(), p.text_secondary, inner.width());
        let gy = inner.min.y + line_height(ui, &tf) + 8.0;
        let col_w = ((inner.width() - 7.0) / 2.0).floor();
        let flags = [
            (Dest::ImageUploader, s::CustomUploaderSettingsWindow_ImageUploader()),
            (Dest::TextUploader, s::CustomUploaderSettingsWindow_TextUploader()),
            (Dest::FileUploader, s::CustomUploaderSettingsWindow_FileUploader()),
            (Dest::URLShortener, s::CustomUploaderSettingsWindow_URLShortener()),
            (Dest::URLSharingService, s::CustomUploaderSettingsWindow_URLSharingService()),
        ];
        for (i, (flag, text)) in flags.iter().enumerate() {
            let (row, col) = (i / 2, i % 2);
            let x = if col == 0 { inner.min.x } else { inner.max.x - col_w };
            let r = Rect::from_min_size(pos2(x, gy + (choice_h + 7.0) * row as f32), vec2(col_w, choice_h));
            border::border(ui.painter(), r, 4.0, 1.0, Some(Brush::Solid(p.background_main)), Some(p.border));
            let cb_rect = Rect::from_min_size(pos2(r.min.x + 10.0, r.min.y + 8.0), vec2(r.width() - 20.0, 32.0));
            let mut on = item.destination_type.contains(*flag);
            let cb = CheckBox { text_font: content_font(13.0, 400), ..CheckBox::new(text) };
            if cb.show(ui, cb_rect, ui.id().with(("cap", index, i)), &mut on).changed() {
                if on {
                    item.destination_type.insert(*flag)
                } else {
                    item.destination_type.remove(*flag)
                }
            }
        }
        y = caps.max.y + 10.0;
        // Endpoint: the path icon, the title and the request URL.
        let ep_h = 2.0 + 24.0 + line_height(ui, &tf) + 2.0 + line_height(ui, &lf);
        let ep = Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), ep_h));
        card(ui, ep);
        let inner = ep.shrink(13.0);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.min.x, center_y(inner, 20.0)), Vec2::splat(20.0)), icon::FilePath, p.accent_icon);
        let tx = inner.min.x + 30.0;
        layout::text(ui, pos2(tx, inner.min.y), s::CustomUploaderSettingsWindow_Endpoint(), tf.clone(), p.text_secondary, inner.max.x - tx);
        let url = item.request_url.clone().unwrap_or_default();
        layout::text(ui, pos2(tx, inner.min.y + line_height(ui, &tf) + 2.0), &url, lf, p.text_secondary, inner.max.x - tx);
    }

    fn request(&mut self, ui: &mut egui::Ui, area: Rect, config: &mut UploadersConfig, index: usize) {
        let p = palette(ui.ctx());
        let (tf, lf) = (card_title_font(), field_label_font());
        let mut y = area.min.y + Self::section_title(ui, area, s::CustomUploaderSettingsWindow_Request()) + 10.0;
        let card_h = 2.0 + 24.0 + line_height(ui, &tf) + 7.0 + line_height(ui, &lf) + 3.0 + 30.0;
        let ep = Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), card_h));
        card(ui, ep);
        let inner = ep.shrink(13.0);
        layout::text(ui, inner.min, s::CustomUploaderSettingsWindow_Endpoint(), tf.clone(), p.text_secondary, inner.width());
        let ly = inner.min.y + line_height(ui, &tf) + 7.0;
        layout::text(ui, pos2(inner.min.x, ly), s::CustomUploaderSettingsWindow_Method(), lf.clone(), p.text_secondary, 120.0);
        layout::text(ui, pos2(inner.min.x + 127.0, ly), s::CustomUploaderSettingsWindow_RequestURL(), lf.clone(), p.text_secondary, inner.width());
        let by = ly + line_height(ui, &lf) + 3.0;
        let methods: Vec<ComboItem> = HttpMethod::ALL.iter().map(|m| ComboItem::text(m.name())).collect();
        let item = &mut config.CustomUploadersList.0[index];
        let sel = HttpMethod::ALL.iter().position(|m| *m == item.request_method);
        let combo = Rect::from_min_size(pos2(inner.min.x, by), vec2(120.0, 30.0));
        if let Some(i) = (ComboBox { padding: Thickness::hv(7.0, 4.0), enabled: true }).show(ui, combo, ui.id().with(("method", index)), &methods, sel) {
            item.request_method = HttpMethod::ALL[i].clone();
        }
        let url_rect = Rect::from_min_max(pos2(inner.min.x + 127.0, by), pos2(inner.max.x, by + 30.0));
        let mut url = field_text(item, Field::RequestUrl);
        if self.syntax_box(ui, url_rect, ui.id().with(("request-url", index)), Field::RequestUrl, &mut url, false, false) {
            set_field(&mut config.CustomUploadersList.0[index], Field::RequestUrl, url);
        }
        y = ep.max.y + 10.0;
        let half = ((area.width() - 8.0) / 2.0).floor();
        let left = Rect::from_min_size(pos2(area.min.x, y), vec2(half, 300.0));
        let right = Rect::from_min_max(pos2(area.max.x - half, y), pos2(area.max.x, y + 300.0));
        self.pairs(ui, left, config, index, Pairs::Parameters, s::CustomUploaderSettingsWindow_Parameters());
        self.pairs(ui, right, config, index, Pairs::Headers, s::CustomUploaderSettingsWindow_Headers());
    }

    /// `CustomUploaderKeyValueEditor`: a panel (`Padding="12"`, 4px
    /// corners), the header and an add button, then the rows.
    fn pairs(&mut self, ui: &mut egui::Ui, rect: Rect, config: &mut UploadersConfig, index: usize, which: Pairs, header: &str) {
        let p = palette(ui.ctx());
        border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
        let inner = rect.shrink(13.0);
        let hf = font(13.0, 600);
        let head = Rect::from_min_size(inner.min, vec2(inner.width(), 30.0));
        layout::text(ui, pos2(head.min.x, center_y(head, line_height(ui, &hf))), header, hf, p.text_secondary, head.width() - 34.0);
        let add = Rect::from_min_size(pos2(head.max.x - 30.0, head.min.y), Vec2::splat(30.0));
        let item = config.CustomUploadersList.0[index].clone();
        let mut changed = false;
        if compact_button(ui, add, ui.id().with(("add", index, which as u8)), Some(icon::Plus), "", false, true) {
            self.rows(index, which, &item).push((String::new(), String::new()));
            changed = true;
        }
        let rows_area = Rect::from_min_max(pos2(inner.min.x, head.max.y + 7.0), inner.max);
        let count = self.rows(index, which, &item).len();
        let mut remove = None;
        let key_w = ((rows_area.width() - 30.0 - 8.0) * 0.38).round();
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rows_area));
        child.set_clip_rect(rows_area.intersect(ui.clip_rect()));
        for r in 0..count {
            let y = rows_area.min.y + 33.0 * r as f32;
            let (mut k, mut v) = self.rows(index, which, &item)[r].clone();
            let key_rect = Rect::from_min_size(pos2(rows_area.min.x, y), vec2(key_w, 30.0));
            let val_rect = Rect::from_min_max(pos2(key_rect.max.x + 4.0, y), pos2(rows_area.max.x - 34.0, y + 30.0));
            let del = Rect::from_min_size(pos2(rows_area.max.x - 30.0, y), Vec2::splat(30.0));
            let kb = TextBox { placeholder: s::CustomUploaderKeyValueEditor_Name(), ..dense(false) };
            let kid = child.id().with(("key", index, which as u8, r));
            kb.show(&mut child, key_rect, kid, &mut k);
            let vid = child.id().with(("value", index, which as u8, r));
            self.syntax_box(&mut child, val_rect, vid, Field::Value(which, r), &mut v, false, false);
            if compact_button(&child, del, child.id().with(("del", index, which as u8, r)), Some(icon::Trash), "", false, true) {
                remove = Some(r);
            }
            let rows = self.rows(index, which, &item);
            if rows[r] != (k.clone(), v.clone()) {
                rows[r] = (k, v);
                changed = true;
            }
        }
        if let Some(r) = remove {
            self.rows(index, which, &item).remove(r);
            changed = true;
        }
        if changed {
            self.sync_rows(config, index, which);
        }
    }

    fn body(&mut self, ui: &mut egui::Ui, area: Rect, config: &mut UploadersConfig, index: usize) {
        let p = palette(ui.ctx());
        let (tf, lf) = (card_title_font(), field_label_font());
        let mut y = area.min.y + Self::section_title(ui, area, s::CustomUploaderSettingsWindow_Body()) + 10.0;
        let card_h = 2.0 + 24.0 + line_height(ui, &lf) + 5.0 + 30.0;
        let bt = Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), card_h));
        card(ui, bt);
        let inner = bt.shrink(13.0);
        layout::text(ui, inner.min, s::CustomUploaderSettingsWindow_BodyType(), lf.clone(), p.text_secondary, inner.width());
        let bodies: Vec<ComboItem> =
            CustomUploaderBody::ALL.iter().map(|b| ComboItem::text(sharex_uploaders::factory::service_name("CustomUploaderBody", b.name()))).collect();
        let combo_style = ComboBox { padding: Thickness::hv(7.0, 4.0), enabled: true };
        let item = &mut config.CustomUploadersList.0[index];
        let sel = CustomUploaderBody::ALL.iter().position(|b| *b == item.body);
        // Sized to its selected item, as Fluent's `ComboBox` (`MaxWidth="360"`).
        let shown: Vec<ComboItem> = sel.map(|i| vec![bodies[i].clone()]).unwrap_or_default();
        let w = combo_style.content_width(ui, &shown).min(360.0);
        let combo = Rect::from_min_size(pos2(inner.min.x, inner.min.y + line_height(ui, &lf) + 5.0), vec2(w, 30.0));
        if let Some(i) = combo_style.show(ui, combo, ui.id().with(("body", index)), &bodies, sel) {
            item.body = CustomUploaderBody::ALL[i].clone();
        }
        y = bt.max.y + 10.0;
        let body = item.body.clone();
        let arguments = matches!(body, CustomUploaderBody::MultipartFormData | CustomUploaderBody::FormURLEncoded);
        let data = matches!(body, CustomUploaderBody::JSON | CustomUploaderBody::XML);
        if arguments {
            // `ColumnDefinitions="*,250"`: the arguments, and the file's form
            // name for multipart bodies (the column stays when it's hidden).
            let left = Rect::from_min_size(pos2(area.min.x, y), vec2(area.width() - 250.0 - 8.0, 330.0));
            self.pairs(ui, left, config, index, Pairs::Arguments, s::CustomUploaderSettingsWindow_Arguments());
            if body == CustomUploaderBody::MultipartFormData {
                let desc = sharex_ui::controls::wrap_lines(ui, s::CustomUploaderSettingsWindow_FileFormNameDescription(), &lf, 250.0 - 26.0);
                let h = 2.0 + 24.0 + line_height(ui, &tf) + 7.0 + line_height(ui, &lf) + 7.0 + 30.0 + 7.0 + line_height(ui, &lf) * desc.len() as f32;
                let r = Rect::from_min_size(pos2(area.max.x - 250.0, y), vec2(250.0, h));
                card(ui, r);
                let inner = r.shrink(13.0);
                layout::text(ui, inner.min, s::CustomUploaderSettingsWindow_UploadedFile(), tf.clone(), p.text_secondary, inner.width());
                let ly = inner.min.y + line_height(ui, &tf) + 7.0;
                layout::text(ui, pos2(inner.min.x, ly), s::CustomUploaderSettingsWindow_FileFormName(), lf.clone(), p.text_secondary, inner.width());
                let br = Rect::from_min_size(pos2(inner.min.x, ly + line_height(ui, &lf) + 7.0), vec2(inner.width(), 30.0));
                let mut v = field_text(&config.CustomUploadersList.0[index], Field::FileFormName);
                if self.syntax_box(ui, br, ui.id().with(("file-form", index)), Field::FileFormName, &mut v, false, false) {
                    set_field(&mut config.CustomUploadersList.0[index], Field::FileFormName, v);
                }
                let mut dy = br.max.y + 7.0;
                for line in &desc {
                    layout::text(ui, pos2(inner.min.x, dy), line, lf.clone(), p.text_secondary, f32::INFINITY);
                    dy += line_height(ui, &lf);
                }
            }
        } else if data {
            let r = Rect::from_min_max(pos2(area.min.x, y), pos2(area.max.x, y + 2.0 + 24.0 + 40.0 + 7.0 + 300.0));
            card(ui, r);
            let inner = r.shrink(13.0);
            let head = Rect::from_min_size(inner.min, vec2(inner.width(), 40.0));
            layout::text(ui, pos2(head.min.x, center_y(head, line_height(ui, &tf))), s::CustomUploaderSettingsWindow_RequestData(), tf.clone(), p.text_secondary, head.width());
            let json = body == CustomUploaderBody::JSON;
            let mut bx = head.max.x;
            if json {
                let w = compact_width(ui, false, s::CustomUploaderSettingsWindow_Minify());
                bx -= w;
                if compact_button(ui, Rect::from_min_size(pos2(bx, head.min.y), vec2(w, 40.0)), ui.id().with("minify"), None, s::CustomUploaderSettingsWindow_Minify(), false, true) {
                    self.format_data(config, index, false);
                }
                bx -= 5.0;
            }
            let w = compact_width(ui, false, s::CustomUploaderSettingsWindow_Beautify());
            bx -= w;
            if compact_button(ui, Rect::from_min_size(pos2(bx, head.min.y), vec2(w, 40.0)), ui.id().with("beautify"), None, s::CustomUploaderSettingsWindow_Beautify(), false, true) {
                self.format_data(config, index, true);
            }
            let data_rect = Rect::from_min_max(pos2(inner.min.x, head.max.y + 7.0), inner.max);
            let mut v = field_text(&config.CustomUploadersList.0[index], Field::Data);
            if self.syntax_box(ui, data_rect, ui.id().with(("data", index)), Field::Data, &mut v, false, true) {
                set_field(&mut config.CustomUploadersList.0[index], Field::Data, v);
            }
        } else {
            // `HasNoBodyEditor`: a card with the code icon and a note.
            let h = 2.0 + 24.0 + 40.0 + 28.0 + 8.0 + line_height(ui, &lf);
            let r = Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), h));
            card(ui, r);
            let mut iy = r.min.y + 1.0 + 12.0 + 20.0;
            icons::paint(ui.painter(), Rect::from_min_size(pos2((r.center().x - 14.0).round(), iy), Vec2::splat(28.0)), icon::FileCode, p.text_secondary);
            iy += 36.0;
            sharex_ui::controls::text_centered_at(ui, r.center().x, iy, s::CustomUploaderSettingsWindow_NoAdditionalBodyFields(), lf, p.text_secondary);
        }
    }

    /// `FormatData`: JSON indented or minified (XML is left as it is).
    fn format_data(&mut self, config: &mut UploadersConfig, index: usize, indented: bool) {
        let item = &mut config.CustomUploadersList.0[index];
        let data = item.data.clone().unwrap_or_default();
        if data.trim().is_empty() || item.body != CustomUploaderBody::JSON {
            return;
        }
        let formatted = serde_json::from_str::<serde_json::Value>(&data)
            .map_err(|e| e.to_string())
            .and_then(|v| if indented { serde_json::to_string_pretty(&v) } else { serde_json::to_string(&v) }.map_err(|e| e.to_string()))
            .map(|s| s.replace('\n', "\r\n"));
        match formatted {
            Ok(t) => item.data = Some(t),
            Err(e) => self.status = e,
        }
    }

    fn response(&mut self, ui: &mut egui::Ui, area: Rect, config: &mut UploadersConfig, index: usize) {
        let p = palette(ui.ctx());
        let tf = card_title_font();
        let mut y = area.min.y + Self::section_title(ui, area, s::CustomUploaderSettingsWindow_Response()) + 10.0;
        let fields = [
            (Field::Url, s::CustomUploaderSettingsWindow_ResultURL()),
            (Field::ThumbnailUrl, s::CustomUploaderSettingsWindow_ThumbnailURL()),
            (Field::DeletionUrl, s::CustomUploaderSettingsWindow_DeletionURL()),
            (Field::ErrorMessage, s::CustomUploaderSettingsWindow_ErrorMessage()),
        ];
        for (i, (field, title)) in fields.iter().enumerate() {
            let extra = if i == 0 { 5.0 + 40.0 } else { 0.0 };
            let h = 2.0 + 24.0 + line_height(ui, &tf) + 5.0 + 30.0 + extra;
            let r = Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), h));
            card(ui, r);
            let inner = r.shrink(13.0);
            layout::text(ui, inner.min, title, tf.clone(), p.text_secondary, inner.width());
            let br = Rect::from_min_size(pos2(inner.min.x, inner.min.y + line_height(ui, &tf) + 5.0), vec2(inner.width(), 30.0));
            let mut v = field_text(&config.CustomUploadersList.0[index], *field);
            if self.syntax_box(ui, br, ui.id().with(("response", index, i)), *field, &mut v, true, false) {
                set_field(&mut config.CustomUploadersList.0[index], *field, v);
            }
            if i == 0 {
                let text = s::CustomUploaderSettingsWindow_TestURLSyntaxButton();
                let w = compact_width(ui, true, text);
                let b = Rect::from_min_size(pos2(inner.max.x - w, br.max.y + 5.0), vec2(w, 40.0));
                if compact_button(ui, b, ui.id().with("syntax-test"), Some(icon::Flask), text, false, true) {
                    self.open_syntax_test(config, index);
                }
            }
            y = r.max.y + 8.0;
        }
    }

    /// `OpenSyntaxTest`: the last test's response, or a sample.
    fn open_syntax_test(&mut self, config: &UploadersConfig, index: usize) {
        let info = self.last_result.as_ref().and_then(|r| r.response_info.clone()).unwrap_or_else(|| ResponseInfo {
            response_text: Some("{\r\n    \"status\": 200,\r\n    \"data\": {\r\n        \"link\": \"https:\\/\\/example.com\\/image.png\"\r\n    }\r\n}".into()),
            response_url: Some("https://example.com/upload".into()),
            ..Default::default()
        });
        let url = config.CustomUploadersList.0[index].url.clone().unwrap_or_default();
        self.syntax = Some(SyntaxTest {
            response_text: info.response_text.clone().unwrap_or_default(),
            response_info: info,
            expression: if url.is_empty() { "{json:data.link}".into() } else { url },
        });
    }

    fn syntax_overlay(&mut self, ui: &mut egui::Ui, full: Rect) {
        let p = palette(ui.ctx());
        // `ShareX.Brush.Overlay.Modal` over the window, a 720px card.
        ui.painter().rect_filled(full, 0.0, p.overlay_modal);
        let _ = ui.interact(full, ui.id().with("modal"), egui::Sense::click());
        let tf = card_title_font();
        let lh = line_height(ui, &tf);
        let h = (2.0 + 32.0 + 40.0 + 12.0 + (lh + 4.0) * 3.0 + 230.0 + 30.0 + 90.0 + 20.0 + 12.0 + 40.0).min(600.0);
        let w = 720.0f32.min(full.width());
        let r = Rect::from_center_size(full.center(), vec2(w, h));
        let r = Rect::from_min_size(pos2(r.min.x.round(), r.min.y.round()), r.size());
        card(ui, r);
        let inner = r.shrink(17.0);
        let head = Rect::from_min_size(inner.min, vec2(inner.width(), 40.0));
        icons::paint(ui.painter(), Rect::from_min_size(pos2(head.min.x, center_y(head, 21.0)), Vec2::splat(21.0)), icon::Flask, p.accent_icon);
        let hf = font(19.0, 600);
        layout::text(ui, pos2(head.min.x + 30.0, center_y(head, line_height(ui, &hf))), s::CustomUploaderSettingsWindow_TestURLSyntax(), hf, p.text_secondary, head.width() - 70.0);
        let x = Rect::from_min_size(pos2(head.max.x - 32.0, center_y(head, 30.0)), vec2(32.0, 30.0));
        let mut close = compact_button(ui, x, ui.id().with("syntax-close-x"), Some(icon::Close), "", false, true);
        let mut y = head.max.y + 12.0;
        let st = self.syntax.as_mut().expect("open");
        let mut text = std::mem::take(&mut st.response_text);
        let mut expr = std::mem::take(&mut st.expression);
        layout::text(ui, pos2(inner.min.x, y), s::CustomUploaderSettingsWindow_ResponseText(), tf.clone(), p.text_secondary, inner.width());
        y += lh + 4.0;
        let resp_rect = Rect::from_min_size(pos2(inner.min.x, y), vec2(inner.width(), 230.0));
        self.syntax_box(ui, resp_rect, ui.id().with("syntax-response"), Field::SyntaxResponse, &mut text, false, true);
        y = resp_rect.max.y + 10.0;
        layout::text(ui, pos2(inner.min.x, y), s::CustomUploaderSettingsWindow_URLSyntax(), tf.clone(), p.text_secondary, inner.width());
        y += lh + 4.0;
        let expr_rect = Rect::from_min_size(pos2(inner.min.x, y), vec2(inner.width(), 30.0));
        self.syntax_box(ui, expr_rect, ui.id().with("syntax-expression"), Field::SyntaxExpression, &mut expr, true, false);
        y = expr_rect.max.y + 10.0;
        let Some(st) = self.syntax.as_mut() else { return };
        st.response_text = text;
        st.expression = expr;
        let mut result = st.result();
        layout::text(ui, pos2(inner.min.x, y), s::CustomUploaderSettingsWindow_Result(), tf, p.text_secondary, inner.width());
        y += lh + 4.0;
        let result_rect = Rect::from_min_size(pos2(inner.min.x, y), vec2(inner.width(), 90.0));
        TextBox { multiline: true, wrap: true, read_only: true, ..dense(true) }.show(ui, result_rect, ui.id().with("syntax-result"), &mut result);
        let ctext = s::CustomUploaderSettingsWindow_Close();
        let cw = compact_width(ui, false, ctext).max(84.0);
        let cb = Rect::from_min_size(pos2(inner.max.x - cw, inner.max.y - 40.0), vec2(cw, 40.0));
        close |= compact_button(ui, cb, ui.id().with("syntax-close"), None, ctext, false, true);
        if close {
            self.syntax = None;
        }
    }

    fn test(&mut self, ui: &mut egui::Ui, area: Rect, config: &mut UploadersConfig, out: &mut Vec<Effect>) {
        let p = palette(ui.ctx());
        let tf = card_title_font();
        let y = area.min.y + Self::section_title(ui, area, s::CustomUploaderSettingsWindow_Test()) + 10.0;
        let names: Vec<ComboItem> = config.CustomUploadersList.0.iter().map(|u| ComboItem::text(u.display_name())).collect();
        let n = names.len() as i32;
        let get = |v: i32| if n == 0 { None } else { Some(v.clamp(0, n - 1) as usize) };
        let cards = [
            (icon::Image, s::CustomUploaderSettingsWindow_ImageUploader(), TestKind::Image),
            (icon::FileText, s::CustomUploaderSettingsWindow_TextUploader(), TestKind::Text),
            (icon::FileCode, s::CustomUploaderSettingsWindow_FileUploader(), TestKind::File),
            (icon::Link, s::CustomUploaderSettingsWindow_URLShortener(), TestKind::UrlShortener),
            (icon::Share, s::CustomUploaderSettingsWindow_URLSharingService(), TestKind::UrlSharing),
        ];
        let half = ((area.width() - 8.0) / 2.0).floor();
        let head_h = line_height(ui, &tf).max(18.0);
        let h = 2.0 + 24.0 + head_h + 7.0 + 40.0;
        let enabled = self.testing.is_none();
        for (i, (key, title, kind)) in cards.iter().enumerate() {
            let (row, col) = (i / 2, i % 2);
            let x = if col == 0 { area.min.x } else { area.max.x - half };
            let r = Rect::from_min_size(pos2(x, y + (h + 8.0) * row as f32), vec2(half, h));
            card(ui, r);
            let inner = r.shrink(13.0);
            let head = Rect::from_min_size(inner.min, vec2(inner.width(), head_h));
            icons::paint(ui.painter(), Rect::from_min_size(pos2(head.min.x, center_y(head, 18.0)), Vec2::splat(18.0)), key, p.accent_icon);
            layout::text(ui, pos2(head.min.x + 26.0, center_y(head, line_height(ui, &tf))), title, tf.clone(), p.text_secondary, head.width() - 26.0);
            let row_r = Rect::from_min_size(pos2(inner.min.x, head.max.y + 7.0), vec2(inner.width(), 40.0));
            let selected_ref = match kind {
                TestKind::Image => &mut config.CustomImageUploaderSelected,
                TestKind::Text => &mut config.CustomTextUploaderSelected,
                TestKind::File => &mut config.CustomFileUploaderSelected,
                TestKind::UrlShortener => &mut config.CustomURLShortenerSelected,
                TestKind::UrlSharing => &mut config.CustomURLSharingServiceSelected,
            };
            let style = ComboBox { padding: Thickness::hv(7.0, 4.0), enabled };
            let current = get(*selected_ref);
            let shown: Vec<ComboItem> = current.map(|i| vec![names[i].clone()]).unwrap_or_default();
            let cw = style.content_width(ui, &shown).min(row_r.width() - 80.0);
            let combo = Rect::from_min_size(row_r.min, vec2(cw, 30.0));
            if let Some(v) = style.show(ui, combo, ui.id().with(("test-combo", i)), &names, current) {
                *selected_ref = v as i32;
            }
            let text = s::CustomUploaderSettingsWindow_Test();
            let bw = compact_width(ui, false, text);
            let b = Rect::from_min_size(pos2(row_r.max.x - bw, row_r.min.y), vec2(bw, 40.0));
            if compact_button(ui, b, ui.id().with(("test", i)), None, text, true, enabled) {
                let index = *selected_ref;
                if *kind == TestKind::Text {
                    // `TextUploadWindow.ShowAsync(TextUploadSample)` first.
                    let (tx, rx) = mpsc::channel();
                    out.push(Effect::AskText(s::CustomUploaderSettingsWindow_TextUploadSample().to_owned(), tx));
                    self.pending_text_test = Some(rx);
                } else {
                    self.start_test(config, *kind, index, None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_value_rows_keep_the_first_of_each_name() {
        let rows = vec![("a".to_owned(), "1".to_owned()), (String::new(), "x".into()), ("a".into(), "2".into()), ("b".into(), String::new())];
        assert_eq!(synchronize(&rows), vec![("a".to_owned(), Some("1".to_owned())), ("b".into(), Some(String::new()))]);
    }

    #[test]
    fn removals_fix_the_selections_as_sharex() {
        let mut c = UploadersConfig::default();
        c.CustomUploadersList.0 = vec![CustomUploaderItem::init(), CustomUploaderItem::init()];
        c.CustomImageUploaderSelected = 2;
        c.CustomTextUploaderSelected = 1;
        c.CustomFileUploaderSelected = 0;
        c.CustomUploadersList.0.remove(1);
        fix_selections_after_removal(&mut c, 1);
        assert_eq!((c.CustomImageUploaderSelected, c.CustomTextUploaderSelected, c.CustomFileUploaderSelected), (1, 0, 0));
    }

    #[test]
    fn the_syntax_test_parses_the_sample() {
        let st = SyntaxTest {
            response_info: ResponseInfo::default(),
            response_text: "{\"data\": {\"link\": \"https:\\/\\/example.com\\/image.png\"}}".into(),
            expression: "{json:data.link}".into(),
        };
        assert_eq!(st.result(), "https://example.com/image.png");
    }
}
