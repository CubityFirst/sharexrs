//! `ShareX/Presentation/Notification`: `NotificationWindow`, the toast a
//! finished task (`TaskManager`) and `TaskHelpers.ShowNotificationTip`
//! show. Borderless, transparent, topmost, never activated, out of the
//! taskbar, as big as its card plus a 14px margin for its shadow, placed in
//! a corner of the work area (`ToastWindowPlacement`, `ToastWindowMonitor`),
//! dark whatever the app's theme. It stays `Duration`, then fades over
//! `FadeDuration`; the pointer over the card holds it and shows the image's
//! caption and the action buttons.

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, CornerRadius, Id, Rect, Sense, Vec2, pos2, vec2};
use image::RgbaImage;
use sharex_settings::{Described, ImageContentAlignment, TaskSettingsGeneral, ToastClickAction, ToastWindowMonitor};
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::wrap_lines_at_hyphens as wrap_lines;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{font, galley, line_height};
use sharex_ui::icons;

use super::{Cx, Dialog, Effect, FileService, Spec, UploadRequest};

/// `ShadowMargin`, `MinimumTextWidth`, `MaximumTextWidth`.
pub const SHADOW_MARGIN: f32 = 14.0;
const MIN_TEXT_WIDTH: f32 = 240.0;
const MAX_TEXT_WIDTH: f32 = 520.0;
/// The caption and action bar's `Background="#C01B1B1B"`.
const OVERLAY: Color32 = Color32::from_rgba_premultiplied(0x14, 0x14, 0x14, 0xC0);
/// `ToolTip.SetShowDelay(button, 400)`.
const TOOLTIP_DELAY: Duration = Duration::from_millis(400);

/// `NotificationActionButton`.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionButton {
    pub action: ToastClickAction,
    pub label: Option<String>,
    pub icon: Option<String>,
    pub dismiss: bool,
}

impl ActionButton {
    /// `ToastWindowButtons` as ShareX writes them.
    pub fn list_from_json(v: &serde_json::Value) -> Vec<Self> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|b| {
                        let action = serde_json::from_value::<ToastClickAction>(b.get("Action")?.clone()).ok()?;
                        let s = |k: &str| b.get(k).and_then(serde_json::Value::as_str).map(str::to_owned);
                        Some(Self { action, label: s("Label"), icon: s("Icon"), dismiss: b.get("DismissNotification").and_then(serde_json::Value::as_bool).unwrap_or(true) })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `NotificationActionButton.GetDefaultIcon`.
    pub fn default_icon(action: &ToastClickAction) -> &'static str {
        use ToastClickAction as A;
        match action {
            A::AnnotateImage => icon::Annotate,
            A::CopyImageToClipboard => icon::Copy,
            A::CopyFile => icon::Files,
            A::CopyFilePath => icon::Clipboard,
            A::CopyUrl => icon::Link,
            A::OpenFile | A::OpenUrl => icon::ExternalLink,
            A::OpenFolder => icon::FolderOpen,
            A::Upload => icon::Upload,
            A::PinToScreen => icon::Pin,
            A::DeleteFile => icon::Trash,
            _ => icon::Close,
        }
    }

    fn label(&self) -> String {
        self.label.clone().filter(|l| !l.trim().is_empty()).unwrap_or_else(|| self.action.localized_description())
    }

    /// A saved icon that isn't an icon key falls back to the default.
    fn icon(&self) -> String {
        match &self.icon {
            Some(i) if icons::exists(i) => i.clone(),
            _ => Self::default_icon(&self.action).to_owned(),
        }
    }
}

/// `NotificationWindowConfig`.
#[derive(Clone, Debug)]
pub struct NotificationConfig {
    pub duration: Duration,
    pub fade_duration: Duration,
    pub placement: ImageContentAlignment,
    pub monitor: ToastWindowMonitor,
    /// `ToastWindowMonitorDeviceName` (`SpecificMonitor`).
    pub monitor_device_name: String,
    /// `Offset` (5).
    pub offset: i32,
    /// `Size`: the image's bounds, the text card's width.
    pub size: (i32, i32),
    /// `BackgroundColor`, `TitleColor`, `TextColor` of the text card.
    pub background: Color32,
    pub title_color: Color32,
    pub text_color: Color32,
    pub image: Option<Arc<RgbaImage>>,
    pub title: String,
    pub text: String,
    pub file_path: String,
    pub url: String,
    pub left_click: ToastClickAction,
    pub right_click: ToastClickAction,
    pub middle_click: ToastClickAction,
    pub button_size: i32,
    pub buttons: Vec<ActionButton>,
}

impl NotificationConfig {
    /// The defaults with a task's (or the default task's) toast settings.
    pub fn new(g: &TaskSettingsGeneral) -> Self {
        Self {
            duration: Duration::from_secs_f32((g.ToastWindowDuration * 1000.0).trunc().max(0.0) / 1000.0),
            fade_duration: Duration::from_secs_f32((g.ToastWindowFadeDuration * 1000.0).trunc().max(0.0) / 1000.0),
            placement: g.ToastWindowPlacement.clone(),
            monitor: g.ToastWindowMonitor.clone(),
            monitor_device_name: g.ToastWindowMonitorDeviceName.clone(),
            offset: 5,
            size: (g.ToastWindowSize.width, g.ToastWindowSize.height),
            background: Color32::from_rgb(50, 50, 50),
            title_color: Color32::from_rgb(240, 240, 240),
            text_color: Color32::from_rgb(210, 210, 210),
            image: None,
            title: String::new(),
            text: String::new(),
            file_path: String::new(),
            url: String::new(),
            left_click: ToastClickAction::CloseNotification,
            right_click: ToastClickAction::CloseNotification,
            middle_click: ToastClickAction::CloseNotification,
            button_size: g.ToastWindowButtonSize,
            buttons: ActionButton::list_from_json(&g.ToastWindowButtons),
        }
    }

    /// `TaskHelpers.ShowNotificationTip(text, title, duration)` with the
    /// default task's settings (`duration < 0`: theirs).
    pub fn tip(g: &TaskSettingsGeneral, text: &str, title: &str, duration_ms: i32) -> Self {
        let mut c = Self::new(g);
        if duration_ms >= 0 {
            c.duration = Duration::from_millis(duration_ms as u64);
        }
        c.title = title.to_owned();
        c.text = text.to_owned();
        c
    }

    /// The task's click actions (`TaskManager`'s toast).
    pub fn with_clicks(mut self, g: &TaskSettingsGeneral) -> Self {
        self.left_click = g.ToastWindowLeftClickAction.clone();
        self.right_click = g.ToastWindowRightClickAction.clone();
        self.middle_click = g.ToastWindowMiddleClickAction.clone();
        self
    }

    /// `IsValid`.
    pub fn is_valid(&self) -> bool {
        (!self.duration.is_zero() || !self.fade_duration.is_zero()) && self.size.0 > 0 && self.size.1 > 0
    }

    fn has_file(&self) -> bool {
        !self.file_path.trim().is_empty()
    }

    /// `CanExecute`.
    pub fn can_execute(&self, action: &ToastClickAction) -> bool {
        use ToastClickAction as A;
        let has_file = self.has_file();
        let has_image_file = has_file && sharex_helpers::file_helpers::is_image_file(&self.file_path);
        let has_target = has_file || !self.url.trim().is_empty();
        let has_image_in_memory = !has_file && self.image.is_some();
        match action {
            A::AnnotateImage | A::CopyImageToClipboard | A::PinToScreen => has_image_file || has_image_in_memory,
            A::Upload => has_file || has_image_in_memory,
            A::CopyFile | A::CopyFilePath | A::OpenFile | A::OpenFolder | A::DeleteFile => has_file,
            A::CopyUrl | A::OpenUrl => has_target,
            A::CloseNotification => true,
            A::Unknown(_) => false,
        }
    }

    /// `PrepareAction` + `ExecuteAction`: what the action does.
    pub fn execute(&self, action: &ToastClickAction) -> Vec<Effect> {
        use ToastClickAction as A;
        if *action == A::CloseNotification || !self.can_execute(action) {
            return Vec::new();
        }
        let path = std::path::PathBuf::from(&self.file_path);
        // `UsesImageInMemory`: an image that was never saved.
        let memory = (!self.has_file()).then(|| self.image.as_ref().map(|i| (**i).clone())).flatten();
        let e = match action {
            A::AnnotateImage => match memory {
                Some(image) => Effect::AnnotateImage(image),
                None => Effect::FileService(FileService::EditImage, path),
            },
            A::CopyImageToClipboard => match memory {
                Some(image) => Effect::CopyImage(image),
                None => Effect::CopyImageFile(path),
            },
            A::CopyFile => Effect::CopyFiles(vec![path]),
            A::CopyFilePath => Effect::CopyText(self.file_path.clone()),
            A::CopyUrl => Effect::CopyText(if self.url.is_empty() { self.file_path.clone() } else { self.url.clone() }),
            A::OpenFile => Effect::OpenFile(path),
            A::OpenFolder => Effect::OpenFolderWithFile(path),
            A::OpenUrl if !self.url.is_empty() => Effect::OpenUrl(self.url.clone()),
            A::OpenUrl => Effect::OpenFile(path),
            A::Upload => match memory {
                Some(image) => Effect::Upload(Box::new(UploadRequest::QrCodeImage(image))),
                None => Effect::FileService(FileService::UploadFile, path),
            },
            A::PinToScreen => match memory {
                Some(image) => Effect::PinImage(image, Box::default()),
                None => Effect::FileService(FileService::PinToScreen, path),
            },
            A::DeleteFile => Effect::DeleteFile(path),
            _ => return Vec::new(),
        };
        vec![e]
    }
}

pub struct NotificationWindow {
    pub config: NotificationConfig,
    texture: Option<egui::TextureHandle>,
    /// `PreviewImage`'s size (points).
    preview: Option<Vec2>,
    /// When it was shown, and when the fade started.
    shown: Option<Instant>,
    fade_from: Option<Instant>,
    duration_ended: bool,
    /// The pointer is over the card (`_pointerInside`).
    pub hovered: bool,
    /// Pressed on the card, not on a button.
    card_pressed: bool,
    /// Over an action button since, for its tooltip.
    tip: Option<(usize, Instant)>,
    /// For `--render`: the card drawn hovered, a button hovered.
    pub force_hover: Option<Option<usize>>,
    /// For `--render`: the hovered button's tooltip drawn (ShareX's is a
    /// window of its own, outside its shots).
    pub force_tooltip: bool,
    /// Closing (an action dismissed it, or the fade ended).
    closed: bool,
    /// A left press on the card that may become a drag of the file
    /// (`_isDragCandidate`, `_dragStart`), and the drag (`_fileDragStarted`).
    drag_start: Option<egui::Pos2>,
    file_dragged: bool,
    /// The button tooltip shown in its popup window (`Effect::Tooltip`).
    popup_tip: Option<(String, Rect)>,
}

impl NotificationWindow {
    /// `NotificationWindow.Show(config)`: `None` when there's nothing to
    /// show (invalid, or neither an image nor text).
    pub fn new(mut config: NotificationConfig) -> Option<Self> {
        if !config.is_valid() {
            return None;
        }
        if config.image.is_none() && config.has_file() {
            config.image = crate::thumbnail::load(std::path::Path::new(&config.file_path)).map(Arc::new);
        }
        if config.image.is_none() && config.text.is_empty() {
            return None;
        }
        // `LoadPreview`: the image within `Size`, never upscaled.
        let preview = config.image.as_ref().map(|img| {
            let (mw, mh) = (config.size.0.max(1) as f32, config.size.1.max(1) as f32);
            let scale = (mw / img.width() as f32).min(mh / img.height() as f32).min(1.0);
            vec2((img.width() as f32 * scale).round().max(1.0), (img.height() as f32 * scale).round().max(1.0))
        });
        Some(Self {
            config,
            texture: None,
            preview,
            shown: None,
            fade_from: None,
            duration_ended: false,
            hovered: false,
            card_pressed: false,
            tip: None,
            force_hover: None,
            force_tooltip: false,
            closed: false,
            drag_start: None,
            file_dragged: false,
            popup_tip: None,
        })
    }

    fn buttons(&self) -> Vec<ActionButton> {
        self.config.buttons.iter().filter(|b| self.config.can_execute(&b.action)).cloned().collect()
    }

    fn button_size(&self) -> f32 {
        self.config.button_size.clamp(16, 128) as f32
    }

    /// The card's width (`NotificationCard.Width`).
    fn card_width(&self) -> f32 {
        match self.preview {
            Some(p) => p.x + 2.0,
            None => (self.config.size.0 as f32).clamp(MIN_TEXT_WIDTH, MAX_TEXT_WIDTH),
        }
    }

    fn title_font() -> egui::FontId {
        font(14.0, 600)
    }

    fn body_font() -> egui::FontId {
        font(13.0, 600)
    }

    /// `TextContent.Margin`: 18,16, with room for the action bar.
    fn text_margins(&self) -> (f32, f32, f32, f32) {
        if self.buttons().is_empty() { (18.0, 16.0, 18.0, 16.0) } else { (18.0, 16.0, 18.0, self.button_size() + 20.0) }
    }

    fn text_width(&self) -> f32 {
        let (l, _, r, _) = self.text_margins();
        self.card_width() - 2.0 - l - r - 34.0 - 12.0
    }

    /// The body: `LineHeight="19"`, wrapped, at most six lines.
    fn body_lines(&self, ui: &egui::Ui) -> Vec<String> {
        let mut lines = wrap_lines(ui, &self.config.text, &Self::body_font(), self.text_width());
        if lines.len() > 6 {
            lines.truncate(6);
            if let Some(last) = lines.last_mut() {
                // `TextTrimming="WordEllipsis"` on the last line.
                last.push('…');
            }
        }
        lines
    }

    fn text_stack_height(&self, ui: &egui::Ui) -> f32 {
        let has_title = !self.config.title.trim().is_empty();
        let has_text = !self.config.text.trim().is_empty();
        let title = if has_title { line_height(ui, &Self::title_font()) } else { 0.0 };
        let body = if has_text { 19.0 * self.body_lines(ui).len() as f32 } else { 0.0 };
        title + body + if has_title && has_text { 3.0 } else { 0.0 }
    }

    fn card_height(&self, ui: &egui::Ui) -> f32 {
        match self.preview {
            Some(p) => p.y + 2.0,
            None => {
                let (_, t, _, b) = self.text_margins();
                2.0 + t + self.text_stack_height(ui).max(34.0) + b
            }
        }
    }

    /// `StartFade` / `OnFadeTick`: the window's opacity now.
    fn fade_opacity(&self, now: Instant) -> f32 {
        match self.fade_from {
            Some(from) if !self.config.fade_duration.is_zero() => {
                1.0 - (now.duration_since(from).as_secs_f32() / self.config.fade_duration.as_secs_f32()).min(1.0)
            }
            _ => 1.0,
        }
    }

    /// One action button (`Button.notification-action`).
    fn action_button(&self, ui: &egui::Ui, rect: Rect, id: Id, key: &str, hovered: bool) -> egui::Response {
        let p = palette(ui.ctx());
        let r = ui.interact(rect, id, Sense::click());
        let over = hovered || r.hovered();
        // `:pressed`: `scale(0.97)`.
        let shown = if r.is_pointer_button_down_on() { Rect::from_center_size(rect.center(), rect.size() * 0.97) } else { rect };
        let radius = (rect.width() / 4.0).clamp(4.0, 16.0);
        let (fill, stroke, fg) = if over {
            (Brush::accent(ui.ctx()), p.accent_border, p.accent_foreground)
        } else {
            (Brush::Solid(Color32::from_rgba_unmultiplied(0x38, 0x38, 0x38, 0xCC)), Color32::from_rgba_unmultiplied(255, 255, 255, 0x33), Color32::from_rgba_unmultiplied(255, 255, 255, 0xF2))
        };
        border::border(ui.painter(), shown, radius, 1.0, Some(fill), Some(stroke));
        let size = (rect.width() / 2.0).round();
        icons::paint(ui.painter(), Rect::from_center_size(shown.center(), Vec2::splat(size)), key, fg);
        r
    }
}

impl Dialog for NotificationWindow {
    fn spec(&self) -> Spec {
        let width = self.card_width() + 2.0 * SHADOW_MARGIN;
        Spec {
            topmost: true,
            skip_taskbar: true,
            maximizable: false,
            activate: false,
            borderless: true,
            ..Spec::fit_height("", width, width)
        }
    }

    fn single(&self) -> Option<&'static str> {
        Some("notification")
    }

    fn theme(&self) -> Option<egui::Theme> {
        Some(egui::Theme::Dark)
    }

    fn opacity(&self) -> f32 {
        self.fade_opacity(Instant::now())
    }

    fn content_height(&mut self, ui: &mut egui::Ui) -> f32 {
        self.card_height(ui) + 2.0 * SHADOW_MARGIN
    }

    /// `PositionWindow`: a corner (or edge, or the centre) of the target
    /// monitor's work area, `Offset` in, less the shadow margin.
    fn position(&self, _work_area: sharex_capture::Rect, size: (u32, u32)) -> Option<(i32, i32)> {
        let screens = sharex_capture::screenshot::screens();
        // `GetTargetScreen`: a specific monitor by its device name, else the
        // pointer's, else (or when that monitor is gone) the primary.
        let screen = match self.config.monitor {
            ToastWindowMonitor::ActiveMonitor => sharex_capture::screenshot::cursor_position()
                .and_then(|(x, y)| screens.iter().find(|s| s.bounds.contains(x, y)))
                .or(screens.iter().find(|s| s.primary)),
            ToastWindowMonitor::SpecificMonitor => {
                crate::display_native::screen_by_device_name(&screens, &crate::display_native::monitor_names(), &self.config.monitor_device_name)
                    .or(screens.iter().find(|s| s.primary))
            }
            _ => screens.iter().find(|s| s.primary),
        }
        .or(screens.first())?;
        Some(placement(&self.config.placement, screen.work_area, size, self.config.offset, screen.scale))
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let now = Instant::now();
        let shown = *self.shown.get_or_insert(now);
        let mut out = Vec::new();
        if self.closed {
            return vec![Effect::Close];
        }
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        let card = Rect::from_min_size(full.min + Vec2::splat(SHADOW_MARGIN), vec2(self.card_width(), self.card_height(ui)));

        let mut wanted_tip: Option<(String, Rect)> = None;
        // The release that ends a drag of the file (OLE had the pointer):
        // `ResetPointerInteraction`, then the fade if it's due.
        if self.file_dragged && !ui.input(|i| i.pointer.primary_down()) {
            self.file_dragged = false;
            self.drag_start = None;
            self.card_pressed = false;
            if self.duration_ended && !ui.rect_contains_pointer(card) {
                self.fade_from = Some(now);
            }
        }
        // The pointer over the card holds it and shows the overlays.
        let hovered = match self.force_hover {
            Some(_) => true,
            None => ui.rect_contains_pointer(card),
        };
        if hovered != self.hovered {
            self.hovered = hovered;
            if hovered {
                self.fade_from = None;
            } else {
                self.card_pressed = false;
                self.tip = None;
                if self.duration_ended {
                    self.fade_from = Some(now);
                }
            }
        }
        if !self.duration_ended && now.duration_since(shown) >= self.config.duration {
            // `EndDuration`.
            self.duration_ended = true;
            if !self.hovered {
                if self.config.fade_duration.is_zero() {
                    return vec![Effect::Close];
                }
                self.fade_from = Some(now);
            }
        }
        if let Some(from) = self.fade_from
            && now.duration_since(from) >= self.config.fade_duration
        {
            return vec![Effect::Close];
        }

        // `BoxShadow="0 0 15 #80000000"`.
        let shadow = egui::epaint::Shadow { offset: [0, 0], blur: 15, spread: 0, color: Color32::from_black_alpha(0x80) };
        ui.painter().add(shadow.as_shape(card, CornerRadius::same(4)));
        let inner = card.shrink(1.0);
        let painter = ui.painter();
        match self.preview {
            Some(size) => {
                // The image, clipped to 3px corners.
                if self.texture.is_none()
                    && let Some(img) = &self.config.image
                {
                    let small = image::imageops::resize(&**img, size.x as u32, size.y as u32, image::imageops::FilterType::Triangle);
                    let ci = egui::ColorImage::from_rgba_unmultiplied([small.width() as usize, small.height() as usize], small.as_raw());
                    self.texture = Some(ui.ctx().load_texture("notification-preview", ci, egui::TextureOptions::LINEAR));
                }
                if let Some(t) = &self.texture {
                    let r = Rect::from_center_size(inner.center(), size);
                    painter.add(egui::epaint::RectShape::filled(r, CornerRadius::same(3), Color32::WHITE).with_texture(t.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))));
                }
            }
            None => {
                border::border(painter, card, 4.0, 0.0, Some(Brush::Solid(self.config.background)), None);
                let (l, t, r, _) = self.text_margins();
                let content = Rect::from_min_max(pos2(inner.min.x + l, inner.min.y + t), pos2(inner.max.x - r, inner.max.y));
                let stack = self.text_stack_height(ui);
                let row_h = stack.max(34.0);
                let tile = Rect::from_min_size(pos2(content.min.x, content.min.y + ((row_h - 34.0) / 2.0).round_ties_even()), Vec2::splat(34.0));
                border::border(painter, tile, 9.0, 0.0, Some(Brush::accent(ui.ctx())), None);
                icons::paint(painter, Rect::from_center_size(tile.center(), Vec2::splat(17.0)), icon::Image, p.accent_foreground);
                let x = tile.max.x + 12.0;
                let w = self.text_width();
                let mut y = content.min.y + ((row_h - stack) / 2.0).round_ties_even();
                if !self.config.title.trim().is_empty() {
                    let tf = Self::title_font();
                    painter.galley(pos2(x, y), galley(ui, &self.config.title, tf.clone(), self.config.title_color, w), self.config.title_color);
                    y += line_height(ui, &tf) + 3.0;
                }
                if !self.config.text.trim().is_empty() {
                    let bf = Self::body_font();
                    let natural = line_height(ui, &bf);
                    for line in self.body_lines(ui) {
                        let ty = y + ((19.0 - natural) / 2.0).round();
                        painter.galley(pos2(x, ty), galley(ui, &line, bf.clone(), self.config.text_color, f32::INFINITY), self.config.text_color);
                        y += 19.0;
                    }
                }
            }
        }
        // The card's border.
        border::border(painter, card, 4.0, 1.0, None, Some(p.accent_border));

        // The card's own clicks (`OnCardPointerPressed`/`Released`), sensed
        // before the action buttons: egui's hit test gives the pointer to
        // the last widget sensing clicks, and the buttons sit on the card
        // (registered after it, they get their hover, tooltip and click;
        // LIVE-CHECKS TOAST2-02, TOAST2-03).
        let card_r = ui.interact(card, ui.id().with("card"), Sense::click());

        let buttons = self.buttons();
        if self.hovered {
            // `ImageCaption`: the title and text over the image's top.
            if self.preview.is_some() && (!self.config.title.trim().is_empty() || !self.config.text.trim().is_empty()) {
                let w = inner.width() - 20.0;
                let tf = font(13.0, 600);
                let bf = font(12.0, 600);
                let mut title: Vec<String> = wrap_lines(ui, &self.config.title, &tf, w);
                title.truncate(2);
                let mut body: Vec<String> = if self.config.text.trim().is_empty() { Vec::new() } else { wrap_lines(ui, &self.config.text, &bf, w) };
                body.truncate(2);
                let (th, bh) = (line_height(ui, &tf), line_height(ui, &bf));
                let has_title = !self.config.title.trim().is_empty();
                let mut h = 14.0;
                if has_title {
                    h += th * title.len() as f32;
                }
                if !body.is_empty() {
                    h += bh * body.len() as f32 + if has_title { 2.0 } else { 0.0 };
                }
                let band = Rect::from_min_size(inner.min, vec2(inner.width(), h));
                painter.rect_filled(band, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, OVERLAY);
                let mut y = band.min.y + 7.0;
                if has_title {
                    for line in &title {
                        painter.galley(pos2(band.min.x + 10.0, y), galley(ui, line, tf.clone(), Color32::WHITE, w), Color32::WHITE);
                        y += th;
                    }
                    y += 2.0;
                }
                let c = Color32::from_rgba_unmultiplied(255, 255, 255, 0xD9);
                for line in &body {
                    painter.galley(pos2(band.min.x + 10.0, y), galley(ui, line, bf.clone(), c, w), c);
                    y += bh;
                }
            }
            // `ActionsPanel`: the buttons, centred, at the bottom.
            if !buttons.is_empty() {
                let size = self.button_size();
                let cell = size + 6.0;
                let per_row = (((inner.width() - 12.0) / cell).floor() as usize).max(1);
                let rows = buttons.len().div_ceil(per_row);
                let h = 12.0 + cell * rows as f32;
                let band = Rect::from_min_max(pos2(inner.min.x, inner.max.y - h), inner.max);
                painter.rect_filled(band, CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 }, OVERLAY);
                let mut tip_at = None;
                for (i, b) in buttons.iter().enumerate() {
                    let (row, col) = (i / per_row, i % per_row);
                    let in_row = (buttons.len() - row * per_row).min(per_row);
                    let row_w = cell * in_row as f32;
                    let x = band.min.x + 6.0 + ((band.width() - 12.0 - row_w) / 2.0).round_ties_even() + cell * col as f32 + 3.0;
                    let y = band.min.y + 6.0 + cell * row as f32 + 3.0;
                    let rect = Rect::from_min_size(pos2(x, y), Vec2::splat(size));
                    let forced = self.force_hover == Some(Some(i));
                    let r = self.action_button(ui, rect, ui.id().with(("action", i)), &b.icon(), forced);
                    if r.hovered() || forced {
                        let since = match self.tip {
                            Some((j, at)) if j == i => at,
                            _ => {
                                self.tip = Some((i, now));
                                now
                            }
                        };
                        if (forced && self.force_tooltip) || (!forced && now.duration_since(since) >= TOOLTIP_DELAY) {
                            tip_at = Some((rect, b.label()));
                        } else {
                            ui.ctx().request_repaint_after(TOOLTIP_DELAY - now.duration_since(since));
                        }
                    }
                    if r.clicked() {
                        // `OnActionButtonClick`.
                        out.extend(self.config.execute(&b.action));
                        if b.dismiss {
                            self.closed = true;
                            out.push(Effect::Close);
                        }
                        return out;
                    }
                }
                if let Some((rect, label)) = tip_at {
                    if self.force_tooltip {
                        // `--render`: drawn in the window (ShareX's is a popup
                        // window of its own, outside its shots).
                        let s = sharex_capture::region::tooltip::size(ui, &label);
                        let at = pos2((rect.center().x - s.x / 2.0).round(), rect.min.y - 4.0 - s.y);
                        sharex_capture::region::tooltip::paint(ui, &p, at, full, &label, 1.0);
                    } else {
                        // `PlacementMode.Top`, `VerticalOffset="-4"`, in its popup.
                        wanted_tip = Some((label, rect));
                    }
                } else if !buttons.iter().enumerate().any(|(i, _)| self.tip.is_some_and(|(j, _)| j == i)) {
                    self.tip = None;
                }
            }
        }

        if wanted_tip != self.popup_tip {
            self.popup_tip = wanted_tip.clone();
            out.push(Effect::Tooltip(wanted_tip));
        }

        // A click on the card (`OnCardPointerReleased`).
        let over_button = self.tip.is_some();
        if card_r.is_pointer_button_down_on() && !over_button {
            if !self.card_pressed && ui.input(|i| i.pointer.primary_pressed()) {
                // `OnCardPointerPressed`: a left press may start a drag.
                self.drag_start = ui.input(|i| i.pointer.press_origin());
                self.file_dragged = false;
            }
            self.card_pressed = true;
        }
        // `OnCardPointerMoved`: 20 px away either way, the file is dragged
        // out (Copy | Move), the fade held while it is.
        if let (Some(start), Some(pos)) = (self.drag_start, ui.input(|i| i.pointer.latest_pos()))
            && !self.file_dragged
            && ui.input(|i| i.pointer.primary_down())
            && ((pos.x - start.x).abs() >= 20.0 || (pos.y - start.y).abs() >= 20.0)
        {
            self.drag_start = None;
            let path = std::path::PathBuf::from(&self.config.file_path);
            if !self.config.file_path.is_empty() && path.is_file() {
                self.file_dragged = true;
                self.fade_from = None;
                out.push(Effect::DragFilesMovable(vec![path]));
                return out;
            }
        }
        if self.file_dragged {
            return out;
        }
        let released = ui.input(|i| {
            [egui::PointerButton::Primary, egui::PointerButton::Secondary, egui::PointerButton::Middle]
                .into_iter()
                .find(|b| i.pointer.button_released(*b))
        });
        if let Some(button) = released
            && self.card_pressed
            && self.hovered
        {
            self.card_pressed = false;
            let action = match button {
                egui::PointerButton::Primary => self.config.left_click.clone(),
                egui::PointerButton::Secondary => self.config.right_click.clone(),
                _ => self.config.middle_click.clone(),
            };
            out.extend(self.config.execute(&action));
            self.closed = true;
            out.push(Effect::Close);
            return out;
        }

        // Keep the timers going.
        if self.fade_from.is_some() {
            ui.ctx().request_repaint();
        } else if !self.duration_ended {
            ui.ctx().request_repaint_after(self.config.duration.saturating_sub(now.duration_since(shown)));
        } else {
            ui.ctx().request_repaint_after(Duration::from_millis(40));
        }
        out
    }
}

/// `GetHorizontalPosition` / `GetVerticalPosition`: the window's top-left
/// for `placement` in `area` (physical pixels), `offset` in less the
/// shadow margin at `scale`.
pub fn placement(placement: &ImageContentAlignment, area: sharex_capture::Rect, size: (u32, u32), offset: i32, scale: f64) -> (i32, i32) {
    use ImageContentAlignment as A;
    let offset = offset.max(0) - (SHADOW_MARGIN as f64 * scale).round() as i32;
    let (w, h) = (size.0 as i32, size.1 as i32);
    let x = match placement {
        A::TopLeft | A::MiddleLeft | A::BottomLeft => area.x + offset,
        A::TopCenter | A::MiddleCenter | A::BottomCenter => area.x + (area.w - w) / 2,
        _ => area.x + area.w - w - offset,
    };
    let y = match placement {
        A::TopLeft | A::TopCenter | A::TopRight => area.y + offset,
        A::MiddleLeft | A::MiddleCenter | A::MiddleRight => area.y + (area.h - h) / 2,
        _ => area.y + area.h - h - offset,
    };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> NotificationConfig {
        NotificationConfig::new(&TaskSettingsGeneral::default())
    }

    #[test]
    fn placement_is_sharex_s() {
        let area = sharex_capture::Rect::new(0, 0, 1024, 720);
        // The minimized-to-tray tip in ShareX's shot: 428x103 at 605,626.
        assert_eq!(placement(&ImageContentAlignment::BottomRight, area, (428, 103), 5, 1.0), (605, 626));
        // The image toast: 430x330 at 603,399.
        assert_eq!(placement(&ImageContentAlignment::BottomRight, area, (430, 330), 5, 1.0), (603, 399));
        assert_eq!(placement(&ImageContentAlignment::TopLeft, area, (430, 330), 5, 1.0), (-9, -9));
        assert_eq!(placement(&ImageContentAlignment::MiddleCenter, area, (430, 330), 5, 1.0), (297, 195));
    }

    #[test]
    fn actions_need_something_to_act_on() {
        let mut c = config();
        c.text = "x".into();
        assert!(c.can_execute(&ToastClickAction::CloseNotification));
        assert!(!c.can_execute(&ToastClickAction::OpenUrl));
        assert!(!c.can_execute(&ToastClickAction::CopyImageToClipboard));
        c.image = Some(Arc::new(RgbaImage::new(2, 2)));
        assert!(c.can_execute(&ToastClickAction::CopyImageToClipboard));
        assert!(c.can_execute(&ToastClickAction::Upload));
        assert!(!c.can_execute(&ToastClickAction::DeleteFile));
        c.file_path = "C:\\x\\a.png".into();
        assert!(c.can_execute(&ToastClickAction::DeleteFile));
        assert!(c.can_execute(&ToastClickAction::AnnotateImage));
        c.file_path = "C:\\x\\a.txt".into();
        assert!(!c.can_execute(&ToastClickAction::AnnotateImage));
        assert!(matches!(c.execute(&ToastClickAction::CopyUrl)[..], [Effect::CopyText(ref t)] if t == "C:\\x\\a.txt"));
    }

    #[test]
    fn the_default_buttons_are_sharex_s() {
        let b = config().buttons;
        let actions: Vec<_> = b.iter().map(|b| b.action.clone()).collect();
        assert_eq!(actions, [ToastClickAction::CopyImageToClipboard, ToastClickAction::AnnotateImage, ToastClickAction::PinToScreen, ToastClickAction::Upload]);
        assert!(b.iter().all(|b| b.dismiss));
    }

    /// The action buttons sit on the card and get the pointer: resting on
    /// Copy image asks for its tooltip after 400 ms, and a click runs it
    /// (LIVE-CHECKS TOAST2-02, TOAST2-03: the card took both).
    #[test]
    fn the_action_buttons_get_their_tooltip_and_click() {
        let mut c = config();
        c.image = Some(Arc::new(RgbaImage::from_pixel(640, 480, image::Rgba([40, 90, 160, 255]))));
        c.duration = Duration::from_secs(60);
        let mut w = NotificationWindow::new(c).expect("an image toast");
        let ctx = egui::Context::default();
        sharex_ui::install(&ctx);
        let mut settings = sharex_settings::ApplicationConfig::default();
        let store = crate::upload::UploadersConfigStore::loaded(Default::default());
        let mut t = 0.0;
        let mut frame = |w: &mut NotificationWindow, events: Vec<egui::Event>| {
            t += 1.0 / 60.0;
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(500.0, 400.0))), time: Some(t), events, ..Default::default() };
            let mut effects = Vec::new();
            let out = ctx.run_ui(input, |ui| {
                let mut cx = Cx { settings: &mut settings, uploaders: &store };
                effects = w.ui(ui, &mut cx);
            });
            let mut out = out;
            out.textures_delta.clear();
            effects
        };
        let card = Rect::from_min_size(pos2(SHADOW_MARGIN, SHADOW_MARGIN), vec2(w.card_width(), 330.0));
        frame(&mut w, vec![egui::Event::PointerMoved(card.center())]);
        frame(&mut w, vec![egui::Event::PointerMoved(card.center())]);
        // Find Copy image (the first button) along the bottom band.
        let mut at = None;
        'scan: for y in (card.max.y as i32 - 60..card.max.y as i32).step_by(4) {
            for x in (card.min.x as i32..card.max.x as i32).step_by(4) {
                let p = pos2(x as f32, y as f32);
                frame(&mut w, vec![egui::Event::PointerMoved(p)]);
                frame(&mut w, vec![egui::Event::PointerMoved(p)]);
                if w.tip.is_some_and(|(i, _)| i == 0) {
                    at = Some(p);
                    break 'scan;
                }
            }
        }
        let at = at.expect("Copy image takes the pointer");
        std::thread::sleep(TOOLTIP_DELAY + Duration::from_millis(50));
        let effects = frame(&mut w, vec![egui::Event::PointerMoved(at)]);
        let label = w.buttons()[0].label();
        assert!(effects.iter().any(|e| matches!(e, Effect::Tooltip(Some((text, _))) if *text == label)), "no tooltip for {label}");
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(&mut w, vec![press(true)]);
        let effects = frame(&mut w, vec![press(false)]);
        assert!(effects.iter().any(|e| matches!(e, Effect::CopyImage(_))), "the click didn't copy the image");
    }

    #[test]
    fn nothing_to_show_is_no_window() {
        assert!(NotificationWindow::new(config()).is_none());
        let mut c = config();
        c.text = "x".into();
        c.duration = Duration::ZERO;
        c.fade_duration = Duration::ZERO;
        assert!(NotificationWindow::new(c).is_none());
    }
}
