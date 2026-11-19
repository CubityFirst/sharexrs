//! `ShareX.Tools/Tools/RemoteStorageBrowser` (`RemoteStorageBrowserWindow`,
//! `RemoteStorageBrowserViewModel`): the files of an Amazon S3 bucket or an
//! FTP/FTPS/SFTP account, from Tools › Remote storage browser and the
//! Destination settings' button (`TaskHelpers.OpenRemoteStorageBrowser`).
//!
//! The providers are `sharex_uploaders::remote_storage`'s. Every call they
//! make blocks, so each operation runs on a worker thread, one at a time
//! (`RunOperationAsync`: `IsBusy` disables the controls, the overlay shows
//! the status); the window polls for the result each frame.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use egui::{Color32, CornerRadius, Id, Rect, Ui, Vec2, pos2, vec2};
use sharex_strings::tools as t;
use sharex_ui::controls::{self, Button, ComboBox, ComboItem, TextBox};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, MenuEntry, MenuStyle, Placement};
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};
use sharex_ui::border::{self, Brush};
use sharex_uploaders::remote_storage::{Capabilities, CancelToken, RemoteStorageError, RemoteStorageItem, RemoteStorageProvider};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::message_box::{DialogResult, MessageBox, MessageBoxIcon};
use crate::upload::UrlOpener;

/// The toolbar's `Padding="12"`, `RowSpacing="10"`, `ColumnSpacing="8"`.
const PAD: f32 = 12.0;
const ROW_SPACING: f32 = 10.0;
const GAP: f32 = 8.0;
/// `Button`'s `Height="40"`; the `TextBox` and `ComboBox` `MinHeight="40"`.
const CONTROL_H: f32 = 40.0;
/// `Button.toolbar-button`: `MinWidth="38"`, `Padding="9,5"`; its
/// `StackPanel Spacing="7"`; the icon takes the button's 13px.
const TOOL_MIN_W: f32 = 38.0;
const TOOL_PAD: Thickness = Thickness { left: 9.0, top: 5.0, right: 9.0, bottom: 5.0 };
const TOOL_ICON: f32 = 13.0;
/// The search box: `Width="250"`, its icon 16px 10 in, `Padding="32,0,8,0"`.
const SEARCH_W: f32 = 250.0;
/// The list: `Margin="12"`, the header row 38, the columns `34,*,140,190`
/// 8 apart in `Padding="10,0"` (rows: `Padding="10,8"`), the icon 17px.
const HEADER_H: f32 = 38.0;
const COLUMNS: [f32; 3] = [34.0, 140.0, 190.0];
const ROW_PAD: (f32, f32) = (10.0, 8.0);
const ITEM_ICON: f32 = 17.0;

fn normal() -> egui::FontId {
    font(13.0, 400)
}
fn semibold() -> egui::FontId {
    font(13.0, 600)
}

/// `RemoteStorageSortColumn`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortColumn {
    Name,
    Size,
    Modified,
}

/// A provider and what the window reads of it without waiting for an
/// operation (its names and capabilities don't change).
struct Slot {
    display_name: String,
    root_path: String,
    capabilities: Capabilities,
    provider: Arc<Mutex<Box<dyn RemoteStorageProvider>>>,
}

/// What a finished operation hands back.
enum Done {
    Listed { slot: usize, path: String, items: Vec<RemoteStorageItem>, location: String },
    Ok,
    HasChildren(bool),
    /// A file read for View.
    Bytes(Vec<u8>),
}

/// What to do once an operation succeeds.
#[derive(Clone)]
enum Then {
    Nothing,
    Browse(String),
    DeleteAfterCheck(RemoteStorageItem),
    View(String),
}

struct Running {
    rx: mpsc::Receiver<Result<Done, RemoteStorageError>>,
    cancel: CancelToken,
    success: Option<String>,
    then: Then,
}

/// A window the browser asked for and waits on.
enum Pending {
    CreateFolder(mpsc::Receiver<Option<String>>),
    Rename(RemoteStorageItem, mpsc::Receiver<Option<String>>),
    Delete(RemoteStorageItem, mpsc::Receiver<DialogResult>),
    DeleteNonEmpty(RemoteStorageItem, mpsc::Receiver<DialogResult>),
}

/// The context menu's commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cmd {
    Download,
    Upload,
    CreateFolder,
    Rename,
    Delete,
    View,
    OpenUrl,
    CopyUrl,
}

pub struct RemoteStorageBrowserWindow {
    slots: Vec<Slot>,
    selected_slot: usize,
    current_path: String,
    location: String,
    items: Vec<RemoteStorageItem>,
    /// `Items`: the shown ones (filtered, sorted), as indexes of `items`.
    shown: Vec<usize>,
    /// `SelectedItem`, an index of `items`.
    selected: Option<usize>,
    pub search: String,
    sort: SortColumn,
    descending: bool,
    pub status: String,
    running: Option<Running>,
    pending: Option<Pending>,
    busy_since: Instant,
    pub drag_over: bool,
    menu: Option<Flyout<Cmd>>,
    scroll: f32,
    opened: bool,
    disposed: bool,
    /// Runs operations on the window's thread (`--render`).
    pub sync: bool,
    opener: Arc<dyn UrlOpener>,
    effects: Vec<Effect>,
}

impl RemoteStorageBrowserWindow {
    /// `new RemoteStorageBrowserWindow(providers, services)`: the first
    /// provider at its root.
    pub fn new(providers: Vec<Box<dyn RemoteStorageProvider>>, opener: Arc<dyn UrlOpener>) -> Self {
        assert!(!providers.is_empty(), "At least one remote storage provider is required.");
        let slots: Vec<Slot> = providers
            .into_iter()
            .map(|p| Slot { display_name: p.display_name(), root_path: p.root_path(), capabilities: p.capabilities(), provider: Arc::new(Mutex::new(p)) })
            .collect();
        let root = slots[0].root_path.clone();
        let location = slots[0].provider.lock().map(|p| p.get_display_path(&root)).unwrap_or_default();
        Self {
            slots,
            selected_slot: 0,
            current_path: root,
            location,
            items: Vec::new(),
            shown: Vec::new(),
            selected: None,
            search: String::new(),
            sort: SortColumn::Name,
            descending: false,
            status: String::new(),
            running: None,
            pending: None,
            busy_since: Instant::now(),
            drag_over: false,
            menu: None,
            scroll: 0.0,
            opened: false,
            disposed: false,
            sync: false,
            opener,
            effects: Vec::new(),
        }
    }

    fn slot(&self) -> &Slot {
        &self.slots[self.selected_slot]
    }

    /// `IsBusy`.
    pub fn is_busy(&self) -> bool {
        self.running.is_some()
    }

    fn has(&self, c: Capabilities) -> bool {
        self.slot().capabilities.contains(c)
    }

    fn selected_item(&self) -> Option<&RemoteStorageItem> {
        self.selected.and_then(|i| self.items.get(i))
    }

    fn parent_path(&self) -> Option<String> {
        self.slot().provider.try_lock().ok().and_then(|p| p.get_parent_path(&self.current_path))
    }

    fn url_of(&self, item: &RemoteStorageItem) -> Option<String> {
        self.slot().provider.try_lock().ok().and_then(|p| p.get_url(item)).filter(|u| !u.trim().is_empty())
    }

    // `Can*`.
    fn can_go_up(&self) -> bool {
        !self.is_busy() && self.parent_path().is_some()
    }
    fn can_upload(&self) -> bool {
        !self.is_busy() && self.has(Capabilities::UPLOAD)
    }
    fn can_create_folder(&self) -> bool {
        !self.is_busy() && self.has(Capabilities::CREATE_FOLDER)
    }
    fn can_download(&self) -> bool {
        !self.is_busy() && self.selected_item().is_some_and(|i| !i.is_folder) && self.has(Capabilities::DOWNLOAD)
    }
    fn can_rename(&self) -> bool {
        !self.is_busy() && self.selected.is_some() && self.has(Capabilities::RENAME)
    }
    fn can_delete(&self) -> bool {
        !self.is_busy() && self.selected.is_some() && self.has(Capabilities::DELETE)
    }
    fn can_view(&self) -> bool {
        self.can_download() && self.selected_item().is_some_and(is_image)
    }
    fn can_use_url(&self) -> bool {
        !self.is_busy()
            && self.has(Capabilities::URL)
            && self.selected_item().filter(|i| !i.is_folder).is_some_and(|i| self.url_of(i).is_some())
    }

    /// `EmptyMessage`.
    fn empty_message(&self) -> &'static str {
        if self.search.trim().is_empty() { t::RemoteStorageBrowser_EmptyFolder() } else { t::RemoteStorageBrowser_NoMatchingItems() }
    }

    // --- Operations ---------------------------------------------------------

    /// `RunOperationAsync`: one at a time; the success text in the status
    /// line; a failure says so and shows the error.
    fn run(&mut self, success: Option<String>, then: Then, op: impl FnOnce(&mut dyn RemoteStorageProvider, &CancelToken) -> Result<Done, RemoteStorageError> + Send + 'static) {
        if self.is_busy() || self.disposed {
            return;
        }
        let cancel = CancelToken::new();
        let provider = self.slot().provider.clone();
        let (tx, rx) = mpsc::channel();
        let token = cancel.clone();
        let work = move || {
            let r = match provider.lock() {
                Ok(mut p) => op(p.as_mut(), &token),
                Err(_) => Err(RemoteStorageError::Failed("The provider is unavailable.".into())),
            };
            let _ = tx.send(r);
        };
        self.busy_since = Instant::now();
        self.running = Some(Running { rx, cancel, success, then });
        if self.sync {
            work();
            self.poll();
        } else if let Err(e) = std::thread::Builder::new().name("remote-storage".into()).spawn(work) {
            self.running = None;
            eprintln!("Remote storage browser: {e}");
        }
    }

    /// `BrowseAsync(path)`: "Loading...", the listing, the counts.
    fn browse(&mut self, path: String) {
        if self.is_busy() || self.disposed {
            return;
        }
        self.status = t::RemoteStorageBrowser_Loading().into();
        let slot = self.selected_slot;
        self.run(None, Then::Nothing, move |p, cancel| {
            let items = p.get_items(&path, cancel)?;
            cancel.check()?;
            let location = p.get_display_path(&path);
            Ok(Done::Listed { slot, path, items, location })
        });
    }

    /// The operation's result, once it's there.
    fn poll(&mut self) {
        let Some(running) = &self.running else { return };
        let result = match running.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(RemoteStorageError::Failed("The operation stopped.".into())),
        };
        let running = self.running.take().expect("running");
        match result {
            Ok(done) => {
                if let Some(s) = running.success.filter(|s| !s.trim().is_empty()) {
                    self.status = s;
                }
                let mut viewed = None;
                match done {
                    Done::Listed { slot, path, items, location } => self.listed(slot, path, items, location),
                    Done::HasChildren(children) => {
                        if let Then::DeleteAfterCheck(item) = running.then.clone() {
                            if children {
                                let (tx, rx) = mpsc::channel();
                                let warning = t::RemoteStorageBrowser_DeleteNonEmptyFolderConfirmation().replace("{0}", &item.name);
                                let mut m = MessageBox::yes_no(delete_title(), warning, MessageBoxIcon::Warning);
                                m.default_button = 1;
                                self.effects.push(Effect::OpenDialog(Box::new(crate::dialogs::message_dialog::MessageDialog::new(m, Some(tx)))));
                                self.pending = Some(Pending::DeleteNonEmpty(item, rx));
                            } else {
                                self.delete(item);
                            }
                        }
                        return;
                    }
                    Done::Ok => {}
                    Done::Bytes(data) => viewed = Some(data),
                }
                match running.then {
                    Then::Browse(path) => self.browse(path),
                    Then::View(name) => {
                        // `ImageViewerWindowIntegration.ShowImage(data, name)`:
                        // nothing for no data; the viewer decodes the rest.
                        if let Some(data) = viewed.filter(|d| !d.is_empty()) {
                            match image::load_from_memory(&data) {
                                Ok(img) => self.effects.push(Effect::ShowImageNamed(img.to_rgba8(), name)),
                                Err(e) => eprintln!("Image viewer: can't decode {name}: {e}"),
                            }
                        }
                    }
                    Then::Nothing | Then::DeleteAfterCheck(_) => {}
                }
            }
            Err(RemoteStorageError::Cancelled) => {}
            Err(e) => {
                eprintln!("Remote storage operation failed. {e}");
                self.status = t::RemoteStorageBrowser_OperationFailed().into();
                self.show_error(&e.to_string());
            }
        }
    }

    fn listed(&mut self, slot: usize, path: String, items: Vec<RemoteStorageItem>, location: String) {
        if slot != self.selected_slot {
            return;
        }
        let path_changed = path != self.current_path;
        self.items = items;
        self.current_path = path;
        if path_changed && !self.search.is_empty() {
            self.search.clear();
        }
        self.apply_filter();
        self.location = location;
        let files = self.items.iter().filter(|i| !i.is_folder).count();
        let folders = self.items.len() - files;
        let total: i64 = self.items.iter().filter(|i| !i.is_folder).map(|i| i.size.unwrap_or(0)).sum();
        self.status = t::RemoteStorageBrowser_ItemCount()
            .replace("{0}", &files.to_string())
            .replace("{1}", &folders.to_string())
            .replace("{2}", &crate::tasks::size_string(total.max(0) as u64, false));
    }

    /// `ApplyFilter`: the search (case-insensitive), the sort; nothing
    /// selected.
    pub fn apply_filter(&mut self) {
        let search = self.search.trim().to_owned();
        let mut shown: Vec<usize> =
            (0..self.items.len()).filter(|&i| search.is_empty() || sharex_helpers::culture::invariant_ignore_case_contains(&self.items[i].name, &search)).collect();
        let items = &self.items;
        let name_cmp = |a: usize, b: usize| sharex_helpers::culture::current_culture_ignore_case_cmp(&items[a].name, &items[b].name);
        let (sort, desc) = (self.sort, self.descending);
        shown.sort_by(|&a, &b| {
            let (x, y) = (&items[a], &items[b]);
            let folders = y.is_folder.cmp(&x.is_folder);
            let by = match sort {
                SortColumn::Name => {
                    let o = name_cmp(a, b);
                    return folders.then(if desc { o.reverse() } else { o });
                }
                SortColumn::Size => {
                    let o = x.size.cmp(&y.size);
                    x.size.is_none().cmp(&y.size.is_none()).then(if desc { o.reverse() } else { o })
                }
                SortColumn::Modified => {
                    let o = x.modified.cmp(&y.modified);
                    x.modified.is_none().cmp(&y.modified.is_none()).then(if desc { o.reverse() } else { o })
                }
            };
            folders.then(by).then(name_cmp(a, b))
        });
        self.shown = shown;
        self.selected = None;
    }

    /// `SortBy(column)`: the same column again flips the order.
    pub fn sort_by(&mut self, column: SortColumn) {
        if self.sort == column {
            self.descending = !self.descending;
        } else {
            self.sort = column;
            self.descending = false;
        }
        self.apply_filter();
    }

    fn change_provider(&mut self, index: usize) {
        if index == self.selected_slot || index >= self.slots.len() || self.is_busy() {
            return;
        }
        self.selected_slot = index;
        self.items.clear();
        self.shown.clear();
        self.search.clear();
        self.selected = None;
        self.current_path = self.slot().root_path.clone();
        self.location = self.slot().provider.try_lock().map(|p| p.get_display_path(&self.current_path)).unwrap_or_default();
        self.browse(self.current_path.clone());
    }

    fn open_folder(&mut self, item: &RemoteStorageItem) {
        if item.is_folder {
            self.browse(item.path.clone());
        }
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.parent_path() {
            self.browse(parent);
        }
    }

    /// `UploadAsync(sources)`: each file into the current folder, then the
    /// folder listed again.
    pub fn upload(&mut self, files: Vec<PathBuf>) {
        if files.is_empty() {
            return;
        }
        let target = self.current_path.clone();
        let success = t::RemoteStorageBrowser_UploadedFiles().replace("{0}", &files.len().to_string());
        self.run(Some(success), Then::Browse(target.clone()), move |p, cancel| {
            for f in &files {
                cancel.check()?;
                let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let mut file = std::fs::File::open(f).map_err(|e| RemoteStorageError::Failed(e.to_string()))?;
                p.upload(&target, &name, &mut file, cancel)?;
            }
            Ok(Done::Ok)
        });
    }

    /// `CreateFolderAsync(name)`.
    pub fn create_folder(&mut self, name: &str) {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return;
        }
        if self.items.iter().any(|i| i.name == name) {
            self.show_error(&t::RemoteStorageBrowser_NameAlreadyExists().replace("{0}", &name));
            return;
        }
        let target = self.current_path.clone();
        let success = t::RemoteStorageBrowser_CreatedFolder().replace("{0}", &name);
        self.run(Some(success), Then::Browse(target.clone()), move |p, cancel| p.create_folder(&target, &name, cancel).map(|_| Done::Ok));
    }

    /// `RenameAsync(item, newName)`.
    pub fn rename(&mut self, item: RemoteStorageItem, new_name: &str) {
        let new_name = new_name.trim().to_owned();
        if new_name.is_empty() || new_name == item.name {
            return;
        }
        if self.items.iter().any(|i| i.path != item.path && i.name == new_name) {
            self.show_error(&t::RemoteStorageBrowser_NameAlreadyExists().replace("{0}", &new_name));
            return;
        }
        let target = self.current_path.clone();
        let success = t::RemoteStorageBrowser_Renamed().replace("{0}", &item.name).replace("{1}", &new_name);
        self.run(Some(success), Then::Browse(target), move |p, cancel| p.rename(&item, &new_name, cancel).map(|_| Done::Ok));
    }

    /// `DeleteAsync(item)` (asked first: [`Self::ask_delete`]).
    fn delete(&mut self, item: RemoteStorageItem) {
        let target = self.current_path.clone();
        let success = t::RemoteStorageBrowser_Deleted().replace("{0}", &item.name);
        self.run(Some(success), Then::Browse(target), move |p, cancel| p.delete(&item, cancel).map(|_| Done::Ok));
    }

    /// `DownloadAsync(item, file)`: a failed download's file is deleted.
    pub fn download_to(&mut self, item: RemoteStorageItem, file: PathBuf) {
        if item.is_folder {
            return;
        }
        let success = t::RemoteStorageBrowser_Downloaded().replace("{0}", &item.name);
        self.run(Some(success), Then::Nothing, move |p, cancel| {
            let mut out = std::fs::File::create(&file).map_err(|e| RemoteStorageError::Failed(e.to_string()))?;
            let r = p.download(&item, &mut out, cancel);
            drop(out);
            if r.is_err() {
                let _ = std::fs::remove_file(&file);
            }
            r.map(|_| Done::Ok)
        });
    }

    // --- The window's commands ----------------------------------------------

    fn show_error(&mut self, message: &str) {
        let m = MessageBox::ok(crate::rs_title(t::RemoteStorageBrowser_Title()), message, MessageBoxIcon::Error);
        self.effects.push(Effect::OpenDialog(Box::new(crate::dialogs::message_dialog::MessageDialog::new(m, None))));
    }

    fn ask_text(&mut self, title: &str, input: &str) -> mpsc::Receiver<Option<String>> {
        let (tx, rx) = mpsc::channel();
        let w = crate::dialogs::input_box::InputBoxWindow::new(title, Some(input), Some(title), Some(t::RemoteStorageBrowser_Cancel()), Some(tx));
        self.effects.push(Effect::OpenDialog(Box::new(w)));
        rx
    }

    fn command(&mut self, cmd: Cmd) {
        if self.pending.is_some() {
            return;
        }
        match cmd {
            Cmd::Upload => {
                if !self.can_upload() {
                    return;
                }
                let files = rfd::FileDialog::new().set_title(t::RemoteStorageBrowser_SelectFilesToUpload()).pick_files().unwrap_or_default();
                self.upload(files);
            }
            Cmd::CreateFolder => {
                if !self.can_create_folder() {
                    return;
                }
                let title = trim_dots(t::RemoteStorageBrowser_CreateFolder());
                let rx = self.ask_text(&title, t::RemoteStorageBrowser_NewFolder());
                self.pending = Some(Pending::CreateFolder(rx));
            }
            Cmd::Rename => {
                let Some(item) = self.selected_item().cloned().filter(|_| self.can_rename()) else { return };
                let rx = self.ask_text(&trim_dots(t::RemoteStorageBrowser_Rename()), &item.name.clone());
                self.pending = Some(Pending::Rename(item, rx));
            }
            Cmd::Delete => {
                let Some(item) = self.selected_item().cloned().filter(|_| self.can_delete()) else { return };
                self.ask_delete(item);
            }
            Cmd::Download => {
                let Some(item) = self.selected_item().cloned().filter(|_| self.can_download()) else { return };
                let mut d = rfd::FileDialog::new().set_title(t::RemoteStorageBrowser_SaveDownloadedFile()).set_file_name(&item.name);
                if let Some(ext) = std::path::Path::new(&item.name).extension().map(|e| e.to_string_lossy().into_owned()).filter(|e| !e.is_empty()) {
                    d = d.add_filter(&ext, &[ext.as_str()]);
                }
                if let Some(file) = d.save_file() {
                    self.download_to(item, file);
                }
            }
            Cmd::View => {
                let Some(item) = self.selected_item().cloned().filter(|_| self.can_view()) else { return };
                let success = t::RemoteStorageBrowser_Downloaded().replace("{0}", &item.name);
                let name = item.name.clone();
                self.run(Some(success), Then::View(name), move |p, cancel| {
                    let mut data = Vec::new();
                    p.download(&item, &mut data, cancel)?;
                    Ok(Done::Bytes(data))
                });
            }
            Cmd::OpenUrl => {
                if let Some(url) = self.selected_item().filter(|i| !i.is_folder).and_then(|i| self.url_of(i)) {
                    self.opener.open(&url);
                }
            }
            Cmd::CopyUrl => {
                if let Some(url) = self.selected_item().filter(|i| !i.is_folder).and_then(|i| self.url_of(i)) {
                    self.effects.push(Effect::CopyText(url));
                }
            }
        }
    }

    /// `OnDeleteClick`: Yes/No first; a folder with something in it is
    /// asked about again (No by default).
    fn ask_delete(&mut self, item: RemoteStorageItem) {
        let prompt = if item.is_folder { t::RemoteStorageBrowser_DeleteFolderConfirmation() } else { t::RemoteStorageBrowser_DeleteFileConfirmation() };
        let (tx, rx) = mpsc::channel();
        let m = MessageBox::yes_no(delete_title(), prompt.replace("{0}", &item.name), MessageBoxIcon::Question);
        self.effects.push(Effect::OpenDialog(Box::new(crate::dialogs::message_dialog::MessageDialog::new(m, Some(tx)))));
        self.pending = Some(Pending::Delete(item, rx));
    }

    /// Answers of the windows the browser opened.
    fn poll_pending(&mut self) {
        let Some(pending) = self.pending.take() else { return };
        match pending {
            Pending::CreateFolder(rx) => match rx.try_recv() {
                Ok(Some(name)) => self.create_folder(&name),
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {}
                Err(mpsc::TryRecvError::Empty) => self.pending = Some(Pending::CreateFolder(rx)),
            },
            Pending::Rename(item, rx) => match rx.try_recv() {
                Ok(Some(name)) => self.rename(item, &name),
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {}
                Err(mpsc::TryRecvError::Empty) => self.pending = Some(Pending::Rename(item, rx)),
            },
            Pending::Delete(item, rx) => match rx.try_recv() {
                Ok(DialogResult::Yes) if item.is_folder => {
                    // `HasChildrenAsync`, then delete or ask again.
                    let folder = item.clone();
                    self.run(None, Then::DeleteAfterCheck(item), move |p, cancel| p.has_children(&folder, cancel).map(Done::HasChildren));
                }
                Ok(DialogResult::Yes) => self.delete(item),
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {}
                Err(mpsc::TryRecvError::Empty) => self.pending = Some(Pending::Delete(item, rx)),
            },
            Pending::DeleteNonEmpty(item, rx) => match rx.try_recv() {
                Ok(DialogResult::Yes) => self.delete(item),
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {}
                Err(mpsc::TryRecvError::Empty) => self.pending = Some(Pending::DeleteNonEmpty(item, rx)),
            },
        }
    }

    /// `OnContextMenuOpened`: what the selection allows.
    fn context_menu(&self) -> Vec<MenuEntry<Cmd>> {
        let (rename, delete) = (self.can_rename(), self.can_delete());
        let (view, url) = (self.can_view(), self.can_use_url());
        vec![
            MenuEntry::item(t::RemoteStorageBrowser_Download(), "", Cmd::Download).visible(self.can_download()),
            MenuEntry::item(t::RemoteStorageBrowser_Upload(), "", Cmd::Upload).visible(self.can_upload()),
            MenuEntry::item(t::RemoteStorageBrowser_CreateFolder(), "", Cmd::CreateFolder).visible(self.can_create_folder()),
            MenuEntry::separator().visible(rename || delete),
            MenuEntry::item(t::RemoteStorageBrowser_Rename(), "", Cmd::Rename).visible(rename),
            MenuEntry::item(t::RemoteStorageBrowser_Delete(), "", Cmd::Delete).visible(delete),
            MenuEntry::separator().visible(view || url),
            MenuEntry::item(t::RemoteStorageBrowser_View(), "", Cmd::View).visible(view),
            MenuEntry::item(t::RemoteStorageBrowser_OpenUrl(), "", Cmd::OpenUrl).visible(url),
            MenuEntry::item(t::RemoteStorageBrowser_CopyUrl(), "", Cmd::CopyUrl).visible(url),
        ]
    }

    /// Opens the context menu at `pos` (a right click; `--render`).
    pub fn open_menu(&mut self, pos: egui::Pos2) {
        let mut f = Flyout::menu(Id::new("remote-storage-menu"), self.context_menu(), Rect::from_min_size(pos, Vec2::ZERO), Placement::Pointer(pos));
        f.style = MenuStyle::Standard;
        self.menu = Some(f);
    }

    /// `Closed`: the operation cancelled, the providers' connections closed
    /// (off this thread, once any operation lets go of them).
    fn dispose(&mut self) {
        if std::mem::replace(&mut self.disposed, true) {
            return;
        }
        if let Some(r) = &self.running {
            r.cancel.cancel();
        }
        let providers: Vec<_> = self.slots.iter().map(|s| s.provider.clone()).collect();
        let _ = std::thread::Builder::new().name("remote-storage-dispose".into()).spawn(move || {
            for p in providers {
                if let Ok(mut p) = p.lock() {
                    p.dispose();
                }
            }
        });
    }

    // --- Drawing ----------------------------------------------------------

    fn toolbar_button_width(ui: &Ui, text: Option<&str>) -> f32 {
        let content = match text {
            Some(t) => TOOL_ICON + 7.0 + text_width(ui, t, normal()),
            None => TOOL_ICON,
        };
        (1.0 + TOOL_PAD.left + content + TOOL_PAD.right + 1.0).max(TOOL_MIN_W)
    }

    /// A `Button.toolbar-button`: the icon (and text), centred.
    fn toolbar_button(ui: &Ui, rect: Rect, id: Id, key: &str, text: Option<&str>, enabled: bool) -> egui::Response {
        let p = palette(ui.ctx());
        let r = Button::compact(TOOL_PAD).show(ui, rect, id, "", enabled);
        let opacity = if enabled { 1.0 } else { 0.35 };
        let inner = rect.shrink(1.0);
        let lh = line_height(ui, &normal());
        let content_w = text.map_or(TOOL_ICON, |t| TOOL_ICON + 7.0 + text_width(ui, t, normal()));
        let x = inner.min.x + ((inner.width() - content_w) / 2.0).round_ties_even();
        let icon_color = border::group_content(p.text, p.control_background, opacity, p.background_panel);
        match text {
            Some(t) => {
                let top = center_y(inner, lh);
                icons::paint(ui.painter(), Rect::from_min_size(pos2(x, top + (lh - TOOL_ICON) / 2.0), Vec2::splat(TOOL_ICON)), key, icon_color);
                let c = p.text_secondary.gamma_multiply(opacity);
                ui.painter().galley(pos2(x + TOOL_ICON + 7.0, top), galley(ui, t, normal(), c, f32::INFINITY), c);
            }
            None => icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(inner, TOOL_ICON)), Vec2::splat(TOOL_ICON)), key, icon_color),
        }
        r
    }

    fn toolbar(&mut self, ui: &mut Ui, full: Rect) -> f32 {
        let p = palette(ui.ctx());
        let height = PAD + CONTROL_H + ROW_SPACING + CONTROL_H + PAD + 1.0;
        let band = Rect::from_min_size(full.min, vec2(full.width(), height));
        ui.painter().rect_filled(band, 0.0, p.background_panel);
        ui.painter().rect_filled(Rect::from_min_max(pos2(band.min.x, band.max.y - 1.0), band.max), 0.0, p.border);
        let busy = self.is_busy();
        let (x0, x1) = (band.min.x + PAD, band.max.x - PAD);
        // Provider:, the providers, Create folder..., Upload...
        let y = band.min.y + PAD;
        let row = Rect::from_min_max(pos2(x0, y), pos2(x1, y + CONTROL_H));
        let label = t::RemoteStorageBrowser_Provider();
        let lw = text_width(ui, label, normal());
        controls::label(ui, Rect::from_min_max(row.min, pos2(x0 + lw, row.max.y)), label, normal(), p.text_secondary);
        let (create, upload) = (t::RemoteStorageBrowser_CreateFolder(), t::RemoteStorageBrowser_Upload());
        let uw = Self::toolbar_button_width(ui, Some(upload));
        let cw = Self::toolbar_button_width(ui, Some(create));
        let ur = Rect::from_min_max(pos2(x1 - uw, y), pos2(x1, y + CONTROL_H));
        let cr = Rect::from_min_size(pos2(ur.min.x - GAP - cw, y), vec2(cw, CONTROL_H));
        let combo_rect = Rect::from_min_max(pos2(x0 + lw + GAP, y), pos2(cr.min.x - GAP, y + CONTROL_H));
        let items: Vec<ComboItem> = self.slots.iter().map(|s| ComboItem::text(s.display_name.clone())).collect();
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: !busy };
        if let Some(i) = combo.show(ui, combo_rect, Id::new("remote-storage-provider"), &items, Some(self.selected_slot)) {
            self.change_provider(i);
        }
        if Self::toolbar_button(ui, cr, Id::new("remote-storage-create"), icon::FolderAdd, Some(create), self.can_create_folder()).clicked() {
            self.command(Cmd::CreateFolder);
        }
        if Self::toolbar_button(ui, ur, Id::new("remote-storage-upload"), icon::Upload, Some(upload), self.can_upload()).clicked() {
            self.command(Cmd::Upload);
        }
        // Location:, the path, Up, Refresh, the search.
        let y = y + CONTROL_H + ROW_SPACING;
        let label = t::RemoteStorageBrowser_Location();
        let lw = text_width(ui, label, normal());
        controls::label(ui, Rect::from_min_max(pos2(x0, y), pos2(x0 + lw, y + CONTROL_H)), label, normal(), p.text_secondary);
        let search = Rect::from_min_max(pos2(x1 - SEARCH_W, y), pos2(x1, y + CONTROL_H));
        let refresh = Rect::from_min_size(pos2(search.min.x - GAP - TOOL_MIN_W, y), vec2(TOOL_MIN_W, CONTROL_H));
        let up = Rect::from_min_size(pos2(refresh.min.x - GAP - TOOL_MIN_W, y), vec2(TOOL_MIN_W, CONTROL_H));
        let location = Rect::from_min_max(pos2(x0 + lw + GAP, y), pos2(up.min.x - GAP, y + CONTROL_H));
        let mut text = self.location.clone();
        TextBox { padding: Thickness::hv(8.0, 6.0), read_only: true, ..TextBox::default() }.show(ui, location, Id::new("remote-storage-location"), &mut text);
        // `ToolTip.Tip`s (a disabled control gets no pointer events, so no tip).
        let tip = |ui: &Ui, rect: Rect, key: &str, text: &str, enabled: bool| {
            if enabled {
                sharex_ui::tooltip::show(ui, rect, Id::new(("remote-storage-tip", key)), text);
            }
        };
        tip(ui, up, "up", t::RemoteStorageBrowser_Up(), self.can_go_up());
        tip(ui, refresh, "refresh", t::RemoteStorageBrowser_Refresh(), !busy);
        tip(ui, search, "search", t::RemoteStorageBrowser_SearchCurrentFolder(), !busy);
        if Self::toolbar_button(ui, up, Id::new("remote-storage-up"), icon::ArrowUp, None, self.can_go_up()).clicked() {
            self.go_up();
        }
        if Self::toolbar_button(ui, refresh, Id::new("remote-storage-refresh"), icon::Refresh, None, !busy).clicked() {
            self.browse(self.current_path.clone());
        }
        let before = self.search.clone();
        let tb = TextBox { padding: Thickness::new(32.0, 0.0, 8.0, 0.0), placeholder: t::RemoteStorageBrowser_SearchCurrentFolder(), enabled: !busy, ..TextBox::default() };
        tb.show(ui, search, Id::new("remote-storage-search"), &mut self.search);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(search.min.x + 10.0, center_y(search, 16.0)), Vec2::splat(16.0)), icon::Search, p.text);
        if self.search != before {
            self.apply_filter();
        }
        height
    }

    /// The column headings and the list.
    fn list(&mut self, ui: &mut Ui, panel: Rect) {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        border::border(ui.painter(), panel, 4.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
        let inner = panel.shrink(1.0);
        let busy = self.is_busy();
        // The heading row: panel background, a 1px line under it.
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), HEADER_H));
        ui.painter().rect_filled(Rect::from_min_max(pos2(header.min.x, header.max.y - 1.0), header.max), 0.0, p.border);
        let cols = columns(Rect::from_min_max(pos2(header.min.x + ROW_PAD.0, header.min.y), pos2(header.max.x - ROW_PAD.0, header.max.y - 1.0)));
        let headings = [(SortColumn::Name, t::RemoteStorageBrowser_Name()), (SortColumn::Size, t::RemoteStorageBrowser_Size()), (SortColumn::Modified, t::RemoteStorageBrowser_Modified())];
        for (k, (column, text)) in headings.into_iter().enumerate() {
            let cell = cols[k + 1];
            let resp = ui.interact(cell, Id::new(("remote-storage-heading", k)), if busy { egui::Sense::hover() } else { egui::Sense::click() });
            let lh = line_height(ui, &normal());
            let y = center_y(cell, lh);
            ui.painter().galley(pos2(cell.min.x, y), galley(ui, text, normal(), p.text_secondary, f32::INFINITY), p.text_secondary);
            if self.sort == column {
                let key = if self.descending { icon::ArrowDown } else { icon::ArrowUp };
                let x = cell.min.x + text_width(ui, text, normal()) + 6.0;
                icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(cell, 13.0)), Vec2::splat(13.0)), key, p.text_secondary);
            }
            if resp.clicked() {
                self.sort_by(column);
            }
        }
        // The rows (`ListBox.storage-list`, Fluent's background).
        let area = Rect::from_min_max(pos2(inner.min.x, header.max.y), inner.max);
        ui.painter().with_clip_rect(area.intersect(ui.clip_rect())).rect_filled(area, CornerRadius { sw: 3, se: 3, ..Default::default() }, f.chrome_medium_low);
        let row_h = ROW_PAD.1 + line_height(ui, &normal()).max(ITEM_ICON) + ROW_PAD.1;
        let content = row_h * self.shown.len() as f32;
        let max = (content - area.height()).max(0.0);
        if !busy && ui.rect_contains_pointer(area) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max);
        let painter = ui.painter().with_clip_rect(area.intersect(ui.clip_rect()));
        let mut clicked = None;
        let mut double = None;
        let mut context = None;
        for (n, &i) in self.shown.iter().enumerate() {
            let row = Rect::from_min_size(pos2(area.min.x, area.min.y + row_h * n as f32 - self.scroll), vec2(area.width(), row_h));
            if !row.intersects(area) {
                continue;
            }
            let item = &self.items[i];
            let selected = self.selected == Some(i);
            let r = ui.interact(row.intersect(area), Id::new(("remote-storage-row", n)), if busy { egui::Sense::hover() } else { egui::Sense::click() });
            controls::list_item_background(&painter, row, 0.0, r.hovered() && !busy, r.is_pointer_button_down_on(), selected);
            if r.clicked() {
                clicked = Some(i);
            }
            if r.double_clicked() {
                double = Some(i);
            }
            if r.secondary_clicked() {
                context = Some((i, r.interact_pointer_pos().unwrap_or(row.center())));
            }
            let fg = if selected { p.accent_foreground } else { p.text_secondary };
            let icon_fg = if selected { p.accent_foreground } else { p.text_secondary };
            let c = columns(Rect::from_min_max(pos2(row.min.x + ROW_PAD.0, row.min.y + ROW_PAD.1), pos2(row.max.x - ROW_PAD.0, row.max.y - ROW_PAD.1)));
            let key = if item.is_folder { icon::Folder } else if is_image(item) { icon::FileImage } else { icon::File };
            let ir = Rect::from_min_size(pos2(c[0].min.x + ((c[0].width() - ITEM_ICON) / 2.0).round_ties_even(), center_y(c[0], ITEM_ICON)), Vec2::splat(ITEM_ICON));
            icons::paint(&painter, ir, key, icon_fg);
            let lh = line_height(ui, &normal());
            let size = item.size.map(|s| crate::tasks::size_string(s.max(0) as u64, false)).unwrap_or_default();
            let modified = item.modified.map(|m| sharex_helpers::date_format::CultureFormat::current().format_g(m.with_timezone(&chrono::Local).naive_local())).unwrap_or_default();
            for (k, text) in [(1, item.name.clone()), (2, size), (3, modified)] {
                let s = crate::image_tools::ellipsize(ui, &text, &normal(), c[k].width());
                painter.galley(pos2(c[k].min.x, center_y(c[k], lh)), galley(ui, &s, normal(), fg, f32::INFINITY), fg);
            }
        }
        crate::dialogs::layout::scroll_bar(ui, area, content, self.scroll, 0.0);
        if let Some(i) = clicked {
            self.selected = Some(i);
        }
        if let Some((i, pos)) = context {
            // `OnItemPointerPressed`: a right click selects the row.
            self.selected = Some(i);
            self.open_menu(pos);
        }
        if let Some(i) = double {
            self.selected = Some(i);
            let item = self.items[i].clone();
            if item.is_folder {
                self.open_folder(&item);
            } else if self.can_view() {
                self.command(Cmd::View);
            }
        }
        // `OnFileListKeyDown`: Enter opens a folder, Delete deletes.
        if !busy && ui.memory(|m| m.focused().is_none()) {
            let (enter, del) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Delete)));
            if enter && let Some(item) = self.selected_item().filter(|i| i.is_folder).cloned() {
                self.open_folder(&item);
            } else if del && self.can_delete() {
                self.command(Cmd::Delete);
            }
        }
        // The empty state, the drop target, the busy overlay.
        if !busy && self.shown.is_empty() {
            crate::image_tools::empty_state(ui, area, icon::FolderOpen, 48.0, 10.0, &[(self.empty_message(), normal(), p.text_secondary)]);
        }
        if self.drag_over {
            let r = area.shrink(8.0);
            let fill = p.background_panel.gamma_multiply(0.96);
            border::border(ui.painter(), r, 8.0, 2.0, Some(Brush::Solid(fill)), Some(p.accent_border.gamma_multiply(0.96)));
            let text = t::RemoteStorageBrowser_DropFilesToUpload();
            let lh = line_height(ui, &semibold());
            let total = 42.0 + 10.0 + lh;
            let top = r.min.y + ((r.height() - total) / 2.0).round_ties_even();
            let ix = r.min.x + ((r.width() - 42.0) / 2.0).round_ties_even();
            icons::paint(ui.painter(), Rect::from_min_size(pos2(ix, top), Vec2::splat(42.0)), icon::Upload, p.accent_icon);
            let tw = text_width(ui, text, semibold());
            let tx = r.min.x + ((r.width() - tw) / 2.0).round_ties_even();
            ui.painter().galley(pos2(tx, top + 52.0), galley(ui, text, semibold(), p.text_secondary, f32::INFINITY), p.text_secondary);
        }
        if busy {
            ui.painter().with_clip_rect(area.intersect(ui.clip_rect())).rect_filled(area, CornerRadius { sw: 3, se: 3, ..Default::default() }, Color32::from_black_alpha(0x66));
            let lh = line_height(ui, &semibold());
            let total = 4.0 + 12.0 + lh;
            let top = area.min.y + ((area.height() - total) / 2.0).round_ties_even();
            let bar = Rect::from_min_size(pos2(area.min.x + ((area.width() - 220.0) / 2.0).round_ties_even(), top), vec2(220.0, 4.0));
            controls::indeterminate_progress(ui.painter(), bar, self.busy_since.elapsed().as_secs_f64());
            let tw = text_width(ui, &self.status, semibold());
            let tx = area.min.x + ((area.width() - tw) / 2.0).round_ties_even();
            ui.painter().galley(pos2(tx, top + 16.0), galley(ui, &self.status, semibold(), Color32::WHITE, f32::INFINITY), Color32::WHITE);
        }
    }

    fn status_bar(&self, ui: &Ui, full: Rect) -> f32 {
        let p = palette(ui.ctx());
        let lh = line_height(ui, &normal());
        let h = 1.0 + 7.0 + lh + 7.0;
        let bar = Rect::from_min_max(pos2(full.min.x, full.max.y - h), full.max);
        ui.painter().rect_filled(bar, 0.0, p.background_panel);
        ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(bar.width(), 1.0)), 0.0, p.border);
        ui.painter().galley(pos2(bar.min.x + 12.0, bar.min.y + 1.0 + 7.0), galley(ui, &self.status, normal(), p.text_secondary, bar.width() - 24.0), p.text_secondary);
        h
    }
}

/// The four columns (`34,*,140,190`, 8 apart) across `r`.
fn columns(r: Rect) -> [Rect; 4] {
    let name_w = (r.width() - COLUMNS.iter().sum::<f32>() - 3.0 * GAP).max(0.0);
    let widths = [COLUMNS[0], name_w, COLUMNS[1], COLUMNS[2]];
    let mut x = r.min.x;
    widths.map(|w| {
        let c = Rect::from_min_size(pos2(x, r.min.y), vec2(w, r.height()));
        x += w + GAP;
        c
    })
}

/// `IsImage`: a file with an image extension.
fn is_image(item: &RemoteStorageItem) -> bool {
    !item.is_folder && sharex_helpers::file_helpers::is_image_file(&item.name)
}

/// A menu entry's text as a window title (`TrimEnd('.', '…')`).
fn trim_dots(s: &str) -> String {
    s.trim_end_matches(['.', '…']).to_owned()
}

fn delete_title() -> String {
    trim_dots(t::RemoteStorageBrowser_Delete())
}

impl Dialog for RemoteStorageBrowserWindow {
    /// `Width="1000" Height="650"`, `MinWidth="760" MinHeight="480"`.
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(t::RemoteStorageBrowser_Title()), 1000.0, 650.0, 760.0, 480.0)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        if !std::mem::replace(&mut self.opened, true) {
            // `OnOpened`: `InitializeAsync`, the root listed.
            self.browse(self.current_path.clone());
        }
        self.poll();
        self.poll_pending();
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        let top = self.toolbar(ui, full);
        let bottom = self.status_bar(ui, full);
        let panel = Rect::from_min_max(pos2(full.min.x + PAD, full.min.y + top + PAD), pos2(full.max.x - PAD, full.max.y - bottom - PAD));
        self.list(ui, panel);
        if let Some(menu) = &mut self.menu {
            let o = menu.show(ui, full);
            if o.closed {
                self.menu = None;
            }
            if let Some(cmd) = o.command {
                self.menu = None;
                self.command(cmd);
            }
        }
        if self.is_busy() || self.pending.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
        }
        std::mem::take(&mut self.effects)
    }

    /// `OnFilesDragEnter`/`DragLeave`: the drop target while uploading's allowed.
    fn drag(&mut self, hover: bool) {
        self.drag_over = hover && self.can_upload();
    }

    /// `OnFilesDrop`.
    fn dropped(&mut self, files: Vec<PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        self.drag_over = false;
        if self.can_upload() {
            self.upload(files);
        }
        Vec::new()
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        self.dispose();
        Vec::new()
    }
}

impl Drop for RemoteStorageBrowserWindow {
    fn drop(&mut self) {
        self.dispose();
    }
}

// ---------------------------------------------------------------------------
// `--render remote-storage --state <state>`: an in-memory provider with
// made-up files (nothing connects).

/// A provider of made-up files, for `--render` and tests.
pub struct MemoryProvider {
    pub name: String,
    pub entries: Vec<RemoteStorageItem>,
    pub capabilities: Capabilities,
    pub fail: Option<String>,
}

impl MemoryProvider {
    pub fn sample() -> Self {
        let at = |d: u32, h: u32| chrono::FixedOffset::east_opt(0).and_then(|z| chrono::TimeZone::with_ymd_and_hms(&z, 2026, 11, d, h, 30, 0).single());
        let file = |path: &str, size: i64, d: u32| RemoteStorageItem { modified: at(d, 9 + d % 8), ..RemoteStorageItem::new(path.rsplit('/').next().unwrap_or(path), path, false, Some(size)) };
        let folder = |path: &str| RemoteStorageItem::new(path.rsplit('/').next().unwrap_or(path), path, true, None);
        Self {
            name: "ftp.example.com (FTP)".into(),
            entries: vec![
                folder("Screenshots"),
                folder("Recordings"),
                folder("Archive"),
                file("readme.txt", 1_234, 3),
                file("Screenshot_2026-11-18.png", 482_113, 18),
                file("logo.svg", 5_870, 7),
                file("Recording_01.mp4", 18_742_651, 12),
                file("notes.md", 911, 16),
                file("Screenshots/a.png", 120_000, 18),
            ],
            capabilities: Capabilities(1 | 2 | 4 | 8 | 16 | 32),
            fail: None,
        }
    }
}

impl RemoteStorageProvider for MemoryProvider {
    fn display_name(&self) -> String {
        self.name.clone()
    }
    fn root_display_name(&self) -> String {
        self.name.clone()
    }
    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }
    fn get_display_path(&self, path: &str) -> String {
        format!("ftp://ftp.example.com:21/{path}")
    }
    fn get_parent_path(&self, path: &str) -> Option<String> {
        if path.is_empty() { None } else { Some(path.rsplit_once('/').map(|(p, _)| p.to_owned()).unwrap_or_default()) }
    }
    fn get_items(&mut self, path: &str, _cancel: &CancelToken) -> Result<Vec<RemoteStorageItem>, RemoteStorageError> {
        if let Some(e) = &self.fail {
            return Err(RemoteStorageError::Failed(e.clone()));
        }
        let prefix = if path.is_empty() { String::new() } else { format!("{path}/") };
        Ok(self.entries.iter().filter(|e| e.path.strip_prefix(&prefix).is_some_and(|r| !r.contains('/'))).cloned().collect())
    }
    fn download(&mut self, _item: &RemoteStorageItem, destination: &mut dyn std::io::Write, _cancel: &CancelToken) -> Result<(), RemoteStorageError> {
        destination.write_all(b"made-up").map_err(|e| RemoteStorageError::Failed(e.to_string()))
    }
    fn upload(&mut self, dir: &str, name: &str, source: &mut dyn std::io::Read, _cancel: &CancelToken) -> Result<(), RemoteStorageError> {
        let mut data = Vec::new();
        source.read_to_end(&mut data).map_err(|e| RemoteStorageError::Failed(e.to_string()))?;
        let path = if dir.is_empty() { name.to_owned() } else { format!("{dir}/{name}") };
        self.entries.push(RemoteStorageItem::new(name, &path, false, Some(data.len() as i64)));
        Ok(())
    }
    fn create_folder(&mut self, dir: &str, name: &str, _cancel: &CancelToken) -> Result<(), RemoteStorageError> {
        let path = if dir.is_empty() { name.to_owned() } else { format!("{dir}/{name}") };
        self.entries.push(RemoteStorageItem::new(name, &path, true, None));
        Ok(())
    }
    fn rename(&mut self, item: &RemoteStorageItem, new_name: &str, _cancel: &CancelToken) -> Result<(), RemoteStorageError> {
        if let Some(e) = self.entries.iter_mut().find(|e| e.path == item.path) {
            e.path = format!("{}{new_name}", e.path.strip_suffix(&e.name).unwrap_or_default());
            e.name = new_name.to_owned();
        }
        Ok(())
    }
    fn has_children(&mut self, folder: &RemoteStorageItem, _cancel: &CancelToken) -> Result<bool, RemoteStorageError> {
        Ok(self.entries.iter().any(|e| e.path.starts_with(&format!("{}/", folder.path))))
    }
    fn delete(&mut self, item: &RemoteStorageItem, _cancel: &CancelToken) -> Result<(), RemoteStorageError> {
        let prefix = format!("{}/", item.path);
        self.entries.retain(|e| e.path != item.path && !e.path.starts_with(&prefix));
        Ok(())
    }
    fn get_url(&self, item: &RemoteStorageItem) -> Option<String> {
        Some(format!("https://example.com/{}", item.path))
    }
}

pub fn is_render_screen(name: &str) -> bool {
    name == "remote-storage"
}

/// `remote-storage --state listed|empty|loading|filtered|no-match|selected|
/// sorted-size|sorted-modified-desc|context-menu|drag-over|error|folder|
/// two-providers`.
pub fn render_dialog(_name: &str, o: &crate::dialogs::render::Options) -> Result<Box<dyn Dialog>, String> {
    let mut sample = MemoryProvider::sample();
    let state = o.state.as_deref().unwrap_or("listed");
    if state == "empty" {
        sample.entries.clear();
    }
    let mut providers: Vec<Box<dyn RemoteStorageProvider>> = vec![Box::new(sample)];
    if state == "two-providers" {
        providers.insert(0, Box::new(MemoryProvider { name: "Amazon S3 (example-bucket)".into(), ..MemoryProvider::sample() }));
    }
    let mut w = RemoteStorageBrowserWindow::new(providers, Arc::new(crate::upload::SystemBrowser));
    w.sync = true;
    w.opened = true;
    if state != "loading" {
        w.browse(w.current_path.clone());
    }
    match state {
        "listed" | "empty" | "two-providers" => {}
        "loading" => {
            // Busy with a listing that never ends.
            let (_tx, rx) = mpsc::channel();
            std::mem::forget(_tx);
            w.status = t::RemoteStorageBrowser_Loading().into();
            w.running = Some(Running { rx, cancel: CancelToken::new(), success: None, then: Then::Nothing });
        }
        "filtered" | "no-match" => {
            w.search = if state == "filtered" { "screen".into() } else { "zzz".into() };
            w.apply_filter();
        }
        "selected" | "context-menu" => {
            w.selected = w.shown.get(4).copied();
            if state == "context-menu" {
                let pos = o.context.unwrap_or(egui::pos2(300.0, 330.0));
                w.open_menu(pos);
            }
        }
        "sorted-size" => w.sort_by(SortColumn::Size),
        "sorted-modified-desc" => {
            w.sort_by(SortColumn::Modified);
            w.sort_by(SortColumn::Modified);
        }
        "drag-over" => w.drag_over = true,
        "folder" => w.browse("Screenshots".into()),
        "error" => w.status = t::RemoteStorageBrowser_OperationFailed().into(),
        other => return Err(format!("no remote storage browser state {other}")),
    }
    Ok(Box::new(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<String>>);

    impl UrlOpener for Recorder {
        fn open(&self, url: &str) {
            self.0.lock().unwrap().push(url.to_owned());
        }
    }

    fn browser(p: MemoryProvider) -> RemoteStorageBrowserWindow {
        let mut w = RemoteStorageBrowserWindow::new(vec![Box::new(p)], Arc::new(Recorder::default()));
        w.sync = true;
        w.browse(String::new());
        w
    }

    fn names(w: &RemoteStorageBrowserWindow) -> Vec<String> {
        w.shown.iter().map(|&i| w.items[i].name.clone()).collect()
    }

    #[test]
    fn lists_folders_first_and_sorts() {
        let mut w = browser(MemoryProvider::sample());
        assert_eq!(names(&w), ["Archive", "Recordings", "Screenshots", "logo.svg", "notes.md", "readme.txt", "Recording_01.mp4", "Screenshot_2026-11-18.png"]);
        assert_eq!(w.status, "5 file(s), 3 folder(s) • 19.23 MB total");
        assert_eq!(w.location, "ftp://ftp.example.com:21/");
        w.sort_by(SortColumn::Size);
        assert_eq!(names(&w)[3..], ["notes.md", "readme.txt", "logo.svg", "Screenshot_2026-11-18.png", "Recording_01.mp4"]);
        w.sort_by(SortColumn::Size);
        assert_eq!(names(&w)[3], "Recording_01.mp4", "the same column again: descending");
        assert_eq!(names(&w)[..3], ["Archive", "Recordings", "Screenshots"], "folders stay first");
        w.search = "SCREEN".into();
        w.apply_filter();
        assert_eq!(names(&w), ["Screenshots", "Screenshot_2026-11-18.png"]);
        assert_eq!(w.empty_message(), t::RemoteStorageBrowser_NoMatchingItems());
    }

    #[test]
    fn folders_open_and_go_up_and_the_search_clears() {
        let mut w = browser(MemoryProvider::sample());
        w.search = "screen".into();
        w.apply_filter();
        let folder = w.items.iter().find(|i| i.name == "Screenshots").unwrap().clone();
        w.open_folder(&folder);
        assert_eq!(w.current_path, "Screenshots");
        assert!(w.search.is_empty(), "a new folder clears the search");
        assert_eq!(names(&w), ["a.png"]);
        assert!(w.can_go_up());
        w.go_up();
        assert_eq!(w.current_path, "");
        assert!(!w.can_go_up());
    }

    #[test]
    fn create_rename_delete_and_upload() {
        let mut w = browser(MemoryProvider::sample());
        w.create_folder("Archive");
        assert!(matches!(w.effects.as_slice(), [Effect::OpenDialog(_)]), "a name that's there is an error");
        w.effects.clear();
        w.create_folder("  New folder ");
        assert!(names(&w).contains(&"New folder".to_owned()));
        let item = w.items.iter().find(|i| i.name == "notes.md").unwrap().clone();
        w.rename(item, "todo.md");
        assert!(names(&w).contains(&"todo.md".to_owned()));
        // Deleting a folder with something in it asks twice.
        w.selected = w.items.iter().position(|i| i.name == "Screenshots");
        w.command(Cmd::Delete);
        let Some(Pending::Delete(..)) = &w.pending else { panic!("not asked") };
        let (tx, rx) = mpsc::channel();
        w.pending = Some(Pending::Delete(w.selected_item().unwrap().clone(), rx));
        tx.send(DialogResult::Yes).unwrap();
        w.poll_pending();
        let Some(Pending::DeleteNonEmpty(item, _)) = w.pending.take() else { panic!("not asked again") };
        w.delete(item);
        assert!(!names(&w).contains(&"Screenshots".to_owned()));
        assert!(w.status.starts_with("5 file(s), 3 folder(s)"), "{}", w.status);
        // Upload a local file.
        let dir = std::env::temp_dir().join(format!("sharexrs-rsb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("up.txt");
        std::fs::write(&f, b"hello").unwrap();
        w.upload(vec![f]);
        assert!(names(&w).contains(&"up.txt".to_owned()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_listing_says_so() {
        let w = browser(MemoryProvider { fail: Some("Connection refused".into()), ..MemoryProvider::sample() });
        assert_eq!(w.status, t::RemoteStorageBrowser_OperationFailed());
        assert!(matches!(w.effects.as_slice(), [Effect::OpenDialog(_)]));
        assert!(!w.is_busy());
    }

    #[test]
    fn menu_entries_follow_the_selection() {
        let mut w = browser(MemoryProvider::sample());
        let visible = |w: &RemoteStorageBrowserWindow| w.context_menu().iter().filter(|e| e.visible).count();
        assert_eq!(visible(&w), 2, "nothing selected: upload, create folder");
        w.selected = w.items.iter().position(|i| i.name.ends_with(".png"));
        assert!(w.can_view() && w.can_use_url());
        assert_eq!(visible(&w), 10);
        let url = w.url_of(w.selected_item().unwrap()).unwrap();
        assert_eq!(url, "https://example.com/Screenshot_2026-11-18.png");
    }

    #[test]
    fn browses_an_ftp_account_on_the_mock_server() {
        use sharex_uploaders::testing::ftp::{FtpMockConfig, FtpMockServer};
        let server = FtpMockServer::start(FtpMockConfig {
            dirs: vec!["/shots".into()],
            files: vec![("/shots/a.png".into(), b"png".to_vec()), ("/readme.txt".into(), b"hi".to_vec())],
            ..Default::default()
        })
        .unwrap();
        let account = serde_json::json!({"Name": "mock", "Protocol": "FTP", "Host": "127.0.0.1", "Port": server.addr.port(), "Username": "user", "Password": "pass"});
        let config = sharex_uploaders::UploadersConfig { FTPAccountList: serde_json::json!([account]), ..Default::default() };
        let providers = sharex_uploaders::remote_storage::providers(&config, true);
        assert_eq!(providers.len(), 1);
        let mut w = RemoteStorageBrowserWindow::new(providers, Arc::new(Recorder::default()));
        // On its worker thread, as the window runs it.
        w.browse(String::new());
        let start = Instant::now();
        while w.is_busy() {
            assert!(start.elapsed() < std::time::Duration::from_secs(20));
            std::thread::sleep(std::time::Duration::from_millis(10));
            w.poll();
        }
        assert_eq!(names(&w), ["shots", "readme.txt"], "{}", w.status);
        w.create_folder("new");
        while w.is_busy() {
            std::thread::sleep(std::time::Duration::from_millis(10));
            w.poll();
        }
        assert!(server.dirs().iter().any(|d| d == "/new"), "{:?}", server.dirs());
    }
}
