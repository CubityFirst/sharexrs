//! Task settings' variable menus (`TaskSettingsPageBuilder`'s
//! `NamePatternText`, `PixelInfoText`, `UploadInfoText` and `CodeMenuText`):
//! a text box whose menu of variables opens to its right when it gets the
//! focus or a pointer is released over it; an item goes in at the caret.
//!
//! Application settings' sub folder and clipboard format menus are
//! `app_settings::filename_pattern_menu` and `clipboard_format_menu`.

use egui::{Id, Rect, Ui};
use sharex_helpers::code_menu;
use sharex_strings::sharex as s;
use sharex_ui::menu::{Flyout, MenuEntry, MenuStyle, Placement};

use crate::app_settings::CLIPBOARD_RESULT_TOKENS;

/// What a box's menu lists.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VariableKind {
    /// `NamePatternText(value, ignoredEntries)`: the name pattern variables
    /// by category, without these (`CodeMenuEntryFilename` values).
    NamePattern(&'static [&'static str]),
    /// `PixelInfoText`: the pixel info variables, no categories.
    PixelInfo,
    /// `UploadInfoText`: Upload result's tokens, then the name pattern menu.
    UploadInfo,
}

/// `CodeMenuText`'s item: "`%y` - Current year", inserting `%y`.
fn item(pattern: String, description: &str) -> MenuEntry<String> {
    MenuEntry::item(format!("{pattern} - {description}"), "", pattern)
}

/// `CodeMenuText<CodeMenuEntryFilename>(value, useCategories: true, ignored)`:
/// a submenu per category (`%n` has none and sits at the end).
pub fn name_pattern_menu(ignored: &[&str]) -> Vec<MenuEntry<String>> {
    let entries = code_menu::ALL.into_iter().filter(|e| !ignored.contains(&e.value));
    let mut out = Vec::new();
    for (category, group) in code_menu::group_by_category(entries) {
        let items: Vec<_> = group.iter().map(|e| item(e.to_prefix_string(), e.description())).collect();
        match category {
            Some(c) if !c.trim().is_empty() => out.push(MenuEntry::parent(c, "", items)),
            _ => out.extend(items),
        }
    }
    out
}

/// `CodeMenuText<CodeMenuEntryPixelInfo>(value, useCategories: false)`.
pub fn pixel_info_menu() -> Vec<MenuEntry<String>> {
    code_menu::PIXEL_INFO.iter().map(|e| item(e.to_prefix_string(), e.description())).collect()
}

/// `UploadInfoText`: "Upload result" with `UploadInfoTokens` (the token
/// alone as the header), then the name pattern menu with nothing left out.
pub fn upload_info_menu() -> Vec<MenuEntry<String>> {
    let tokens = CLIPBOARD_RESULT_TOKENS.iter().map(|t| MenuEntry::item(*t, "", t.to_string())).collect();
    let mut out = vec![MenuEntry::parent(s::ApplicationSettingsWindow_UploadResult(), "", tokens)];
    out.extend(name_pattern_menu(&[]));
    out
}

pub fn entries(kind: VariableKind) -> Vec<MenuEntry<String>> {
    match kind {
        VariableKind::NamePattern(ignored) => name_pattern_menu(ignored),
        VariableKind::PixelInfo => pixel_info_menu(),
        VariableKind::UploadInfo => upload_info_menu(),
    }
}

/// The open menu (`_activeCodeMenu`): which box it belongs to.
pub struct VariableMenu {
    pub key: String,
    pub flyout: Flyout<String>,
    /// Opened this frame: the press of a click that came in one frame
    /// isn't a press outside it (SET-04a; Avalonia closes on the press and
    /// reopens on the release).
    just_opened: bool,
}

/// The menus of one window: at most one open; a chosen variable waits for
/// its box to take it (the box applies it the next time it's drawn).
#[derive(Default)]
pub struct VariableMenus {
    pub open: Option<VariableMenu>,
    /// A box focused again after an insertion doesn't open its menu again
    /// (ShareX's menu never takes the focus, so `GotFocus` doesn't fire).
    refocused: Option<Id>,
    /// The chosen variable and the box it goes into.
    pending: Option<(String, String)>,
    /// A render opens this box's menu as if it had been focused.
    pub open_on_draw: Option<String>,
}

impl VariableMenus {
    /// `OpenMenu`: closes another box's menu and opens this one's, unless
    /// it's open.
    pub fn open_for(&mut self, key: &str, kind: VariableKind, anchor: Rect) {
        if self.open.as_ref().is_some_and(|m| m.key == key) {
            return;
        }
        let mut flyout = Flyout::menu(Id::new(("variable-menu", key)), entries(kind), anchor, Placement::RightEdgeAlignedTop);
        flyout.style = MenuStyle::Standard;
        self.open = Some(VariableMenu { key: key.to_string(), flyout, just_opened: true });
    }

    /// After a box is drawn: opens its menu when it gained the focus, was
    /// clicked or a drag ended on it (`GotFocus`, `PointerReleased`), and
    /// puts a chosen variable into `value`. Returns whether `value` changed.
    pub fn attach(&mut self, ctx: &egui::Context, key: &str, kind: VariableKind, tb: Option<(egui::Response, Rect, Id)>, value: &mut String) -> bool {
        let Some((resp, rect, id)) = tb else { return false };
        let mut changed = false;
        if self.pending.as_ref().is_some_and(|(k, _)| k == key)
            && let Some((_, token)) = self.pending.take()
        {
            crate::app_settings::insert_at_caret(ctx, id, value, &token, false);
            self.refocused = Some(id);
            changed = true;
        }
        if self.open_on_draw.as_deref() == Some(key) {
            self.open_on_draw = None;
            self.open_for(key, kind, rect);
            return changed;
        }
        if resp.gained_focus() && self.refocused == Some(id) {
            self.refocused = None;
            return changed;
        }
        if resp.enabled() && (resp.gained_focus() || resp.clicked() || resp.drag_stopped()) {
            self.open_for(key, kind, rect);
        }
        changed
    }

    /// The open menu over the window: an item is kept for its box; a click
    /// elsewhere or Escape closes it.
    pub fn show(&mut self, ui: &mut Ui, full: Rect) {
        let Some(menu) = self.open.as_mut() else { return };
        let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("variable-menu-layer"));
        let child = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
        let out = menu.flyout.show(&child, full);
        let just_opened = std::mem::take(&mut menu.just_opened);
        let pressed_outside = !just_opened && ui.input(|i| i.pointer.any_pressed()) && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !menu.flyout.contains(pos));
        if let Some(token) = out.command {
            self.pending = Some((menu.key.clone(), token));
            self.open = None;
            ui.ctx().request_repaint();
        } else if out.closed || pressed_outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open = None;
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(m: &[MenuEntry<String>]) -> Vec<String> {
        m.iter().map(|e| e.header.clone()).collect()
    }

    #[test]
    fn menus_as_the_page_builder_makes_them() {
        // The capture pattern leaves out %n, %t and %pn: no Window category.
        let capture = name_pattern_menu(&["n", "t", "pn"]);
        assert_eq!(headers(&capture), ["Date and time", "Incremental", "Random", "Image", "Computer"]);
        // The window pattern keeps %t and %pn.
        let window = name_pattern_menu(&["n"]);
        assert_eq!(window[0].header, "Window");
        assert_eq!(window[0].children.as_ref().unwrap()[0].header, "%t - Title of window");
        let pixel = pixel_info_menu();
        assert_eq!(pixel.len(), 22);
        assert_eq!(pixel[0].header, "$r255 - Red color (0-255)");
        assert_eq!(pixel[0].command.as_deref(), Some("$r255"));
        let upload = upload_info_menu();
        assert_eq!(upload[0].header, "Upload result");
        assert_eq!(upload[0].children.as_ref().unwrap()[1].header, "$url");
        assert_eq!(upload.last().unwrap().header, "%n - New line");
    }

    #[test]
    fn a_chosen_variable_goes_into_its_box() {
        let ctx = egui::Context::default();
        let mut menus = VariableMenus { pending: Some(("a".into(), "%y".into())), ..Default::default() };
        let mut value = "x".to_string();
        let mut changed = false;
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let id = Id::new("box");
            let resp = ui.interact(Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(10.0, 10.0)), id, egui::Sense::click());
            // Another box doesn't take it.
            assert!(!menus.attach(ui.ctx(), "b", VariableKind::PixelInfo, Some((resp.clone(), resp.rect, id)), &mut value));
            changed = menus.attach(ui.ctx(), "a", VariableKind::PixelInfo, Some((resp.clone(), resp.rect, id)), &mut value);
        });
        out.textures_delta.clear();
        assert!(changed);
        assert_eq!(value, "x%y");
        assert!(menus.pending.is_none());
    }
}
