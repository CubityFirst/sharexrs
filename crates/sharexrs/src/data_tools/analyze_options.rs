//! `ShareX.Tools/Tools/AnalyzeImage/AnalyzeImageOptionsWindow`
//! (`AnalyzeImageOptionsViewModel`): the provider and its key page, each
//! provider's key, model (an editable combo box; OpenAI's with Load
//! models), OpenAI's custom URL, reasoning effort and verbosity, the
//! behaviour switches, Test connection, Save and Cancel. It edits a copy
//! (`target.Clone()`); Save hands the new options back (keeping the
//! prompt, `updated.Input = _target.Input`) and closes. Load models and
//! Test connection run `sharex_tools::analyze_image` off the UI thread.

use std::sync::mpsc;

use egui::{Id, Rect, Ui, pos2, vec2};
use sharex_strings::tools as s;
use sharex_tools::analyze_image::{self as ai, AIOptions, AIProvider};
use sharex_ui::controls::{ComboBox, ComboItem, TextBox, ToggleSwitch};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::center_y;

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools;

pub const WIDTH: f32 = 650.0;
pub const HEIGHT: f32 = 590.0;
/// The provider row's label column (`ColumnDefinitions="100,*,Auto"`).
const PROVIDER_LABEL_W: f32 = 100.0;
/// The provider panels' label columns (`"120,*"`).
const LABEL_W: f32 = 120.0;
const TOGGLE_ROW: f32 = 32.0;

/// `LocalizedDisplayConverter` for a provider.
pub fn provider_text(p: AIProvider) -> &'static str {
    match p {
        AIProvider::OpenAI => s::LocalizedDisplayConverter_OpenAI(),
        AIProvider::Gemini => s::LocalizedDisplayConverter_Gemini(),
        AIProvider::OpenRouter => s::LocalizedDisplayConverter_OpenRouter(),
        AIProvider::OpenAILegacy => s::LocalizedDisplayConverter_OpenAI_legacy(),
    }
}

/// `LocalizedDisplayConverter` for the reasoning and verbosity values.
pub fn level_text(v: &str) -> String {
    match v {
        "minimal" => s::LocalizedDisplayConverter_Minimal().to_owned(),
        "low" => s::LocalizedDisplayConverter_Low().to_owned(),
        "medium" => s::LocalizedDisplayConverter_Medium().to_owned(),
        "high" => s::LocalizedDisplayConverter_High().to_owned(),
        other => other.to_owned(),
    }
}

pub fn provider_items() -> Vec<ComboItem> {
    AIProvider::ALL.iter().map(|p| ComboItem::text(provider_text(*p))).collect()
}

pub fn level_items(levels: &[&str]) -> Vec<ComboItem> {
    levels.iter().map(|l| ComboItem::text(level_text(l))).collect()
}

/// `AddCurrentModel`: the model list with the current model added.
pub fn model_list(defaults: &[&str], current: &str) -> Vec<String> {
    let mut list: Vec<String> = defaults.iter().map(|m| m.to_string()).collect();
    if !current.trim().is_empty() && !list.iter().any(|m| m == current) {
        list.push(current.to_owned());
    }
    list
}

/// A key box: the secret's text, and whether it was edited (an untouched
/// key is written back exactly as read).
struct KeyBox {
    text: String,
    original: String,
}

impl KeyBox {
    fn new(secret: &sharex_uploaders::dpapi::Secret) -> Self {
        let text = secret.get().unwrap_or("").to_owned();
        Self { original: text.clone(), text }
    }

    fn apply(&self, secret: &mut sharex_uploaders::dpapi::Secret) {
        if self.text != self.original {
            secret.set(Some(self.text.clone()));
        }
    }
}

enum Work {
    Test(Result<ai::ConnectionResult, String>),
    Models(Result<Vec<String>, String>),
}

pub struct AnalyzeImageOptionsWindow {
    /// The options it opened with (`_target`).
    target: AIOptions,
    pub provider: AIProvider,
    openai_key: KeyBox,
    pub openai_model: String,
    pub openai_custom_url: String,
    pub openai_reasoning_effort: String,
    pub openai_verbosity: String,
    gemini_key: KeyBox,
    pub gemini_model: String,
    openrouter_key: KeyBox,
    pub openrouter_model: String,
    pub auto_start_region: bool,
    pub auto_start_analyze: bool,
    pub auto_copy_result: bool,
    pub openai_models: Vec<String>,
    gemini_models: Vec<String>,
    openrouter_models: Vec<String>,
    pub is_busy: bool,
    pub status_text: String,
    pub status_success: bool,
    rx: Option<mpsc::Receiver<Work>>,
    /// Where Save sends the options (`Close(true)`).
    reply: Option<mpsc::Sender<AIOptions>>,
    scroll: f32,
}

impl AnalyzeImageOptionsWindow {
    /// `new AnalyzeImageOptionsViewModel(target, service)`.
    pub fn new(target: AIOptions, reply: Option<mpsc::Sender<AIOptions>>) -> Self {
        let d = target.clone();
        let openai_model = d.openai_model.clone().unwrap_or_default();
        let gemini_model = d.gemini_model.clone().unwrap_or_default();
        let openrouter_model = d.openrouter_model.clone().unwrap_or_default();
        Self {
            provider: d.provider.unwrap_or(AIProvider::OpenAI),
            openai_key: KeyBox::new(&d.openai_api_key),
            openai_models: model_list(&ai::OPENAI_MODELS, &openai_model),
            gemini_models: model_list(&ai::GEMINI_MODELS, &gemini_model),
            openrouter_models: model_list(&ai::OPENROUTER_MODELS, &openrouter_model),
            openai_model,
            openai_custom_url: d.openai_custom_url.clone().unwrap_or_default(),
            openai_reasoning_effort: d.openai_reasoning_effort.clone().unwrap_or_default(),
            openai_verbosity: d.openai_verbosity.clone().unwrap_or_default(),
            gemini_key: KeyBox::new(&d.gemini_api_key),
            gemini_model,
            openrouter_key: KeyBox::new(&d.openrouter_api_key),
            openrouter_model,
            auto_start_region: d.auto_start_region,
            auto_start_analyze: d.auto_start_analyze,
            auto_copy_result: d.auto_copy_result,
            is_busy: false,
            status_text: String::new(),
            status_success: false,
            rx: None,
            reply,
            scroll: 0.0,
            target,
        }
    }

    pub fn is_openai(&self) -> bool {
        matches!(self.provider, AIProvider::OpenAI | AIProvider::OpenAILegacy)
    }

    /// `CreateOptions`.
    pub fn create_options(&self) -> AIOptions {
        let mut o = self.target.clone();
        o.provider = Some(self.provider);
        self.openai_key.apply(&mut o.openai_api_key);
        o.openai_model = Some(self.openai_model.clone());
        o.openai_custom_url = (!self.openai_custom_url.is_empty() || self.target.openai_custom_url.is_some()).then(|| self.openai_custom_url.clone());
        o.openai_reasoning_effort = Some(if self.openai_reasoning_effort.is_empty() { "minimal".into() } else { self.openai_reasoning_effort.clone() });
        o.openai_verbosity = Some(if self.openai_verbosity.is_empty() { "medium".into() } else { self.openai_verbosity.clone() });
        self.gemini_key.apply(&mut o.gemini_api_key);
        o.gemini_model = Some(self.gemini_model.clone());
        self.openrouter_key.apply(&mut o.openrouter_api_key);
        o.openrouter_model = Some(self.openrouter_model.clone());
        o.auto_start_region = self.auto_start_region;
        o.auto_start_analyze = self.auto_start_analyze;
        o.auto_copy_result = self.auto_copy_result;
        o
    }

    /// `TestConnectionAsync` (`sync`: on this thread).
    fn test_connection(&mut self, ctx: &egui::Context) {
        self.is_busy = true;
        self.status_success = false;
        self.status_text = s::AnalyzeImageOptionsViewModel_Testing().to_owned();
        let options = self.create_options();
        self.spawn(ctx, move || Work::Test(ai::test_connection(&options)));
    }

    /// `LoadModelsAsync`.
    fn load_models(&mut self, ctx: &egui::Context) {
        self.is_busy = true;
        self.status_success = false;
        self.status_text = s::AnalyzeImageOptionsViewModel_Loading_models().to_owned();
        let options = self.create_options();
        self.spawn(ctx, move || Work::Models(ai::load_models(&options)));
    }

    fn spawn(&mut self, ctx: &egui::Context, work: impl FnOnce() -> Work + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(work());
            ctx.request_repaint();
        });
        self.rx = Some(rx);
    }

    /// What Test connection or Load models answered.
    fn finish(&mut self, work: Work) {
        self.is_busy = false;
        match work {
            Work::Test(Ok(r)) => {
                self.status_success = r.success;
                self.status_text = r.message;
            }
            Work::Models(Ok(models)) => {
                self.openai_models = models.clone();
                if let Some(first) = models.first() {
                    if !models.contains(&self.openai_model) {
                        self.openai_model = first.clone();
                    }
                    self.status_success = true;
                    self.status_text = super::format(s::AnalyzeImageOptionsViewModel_Models_loaded(), &[&models.len().to_string()]);
                } else {
                    self.status_text = s::AnalyzeImageOptionsViewModel_No_models_found().to_owned();
                }
            }
            Work::Test(Err(e)) | Work::Models(Err(e)) => self.status_text = e,
        }
    }

    /// A finished Test connection (`--render`, no request made).
    pub fn set_connection_result(&mut self, success: bool, message: &str) {
        self.finish(Work::Test(Ok(ai::ConnectionResult { success, message: message.to_owned() })));
    }

    /// A finished Load models (`--render`, no request made).
    pub fn set_models(&mut self, models: Vec<String>) {
        self.finish(Work::Models(Ok(models)));
    }

    fn panel_height(&self, ui: &Ui) -> f32 {
        let h = tools::heading_height(ui);
        26.0 + if self.is_openai() { tools::grid_height(&[h, 40.0, 40.0, 40.0, 40.0], 10.0) } else { tools::grid_height(&[h, 40.0, 40.0], 10.0) }
    }

    fn behavior_height(ui: &Ui) -> f32 {
        26.0 + tools::grid_height(&[tools::heading_height(ui), TOGGLE_ROW, TOGGLE_ROW, TOGGLE_ROW], 8.0)
    }

    fn combo() -> ComboBox {
        ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true }
    }

    /// A model row's editable combo box (`HorizontalAlignment="Left"`).
    fn model_combo(ui: &mut Ui, area: Rect, id: &str, models: &[String], text: &mut String) {
        let items: Vec<ComboItem> = models.iter().map(|m| ComboItem::text(m.clone())).collect();
        let combo = Self::combo();
        let w = combo.editable_width(ui, text).min(area.width());
        combo.show_editable(ui, Rect::from_min_size(area.min, vec2(w, area.height())), Id::new(("ai-model", id)), &items, text);
    }

    /// The selected provider's panel.
    fn provider_panel(&mut self, ui: &mut Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        tools::panel(ui, rect);
        let inner = rect.shrink(13.0);
        let hh = tools::heading_height(ui);
        let heading = match self.provider {
            AIProvider::OpenAI | AIProvider::OpenAILegacy => s::AnalyzeImageOptionsWindow_OpenAI(),
            AIProvider::Gemini => s::AnalyzeImageOptionsWindow_Google_Gemini(),
            AIProvider::OpenRouter => s::AnalyzeImageOptionsWindow_OpenRouter(),
        };
        tools::heading(ui, Rect::from_min_size(inner.min, vec2(inner.width(), hh)), heading);
        let row = |k: usize| Rect::from_min_size(pos2(inner.min.x, inner.min.y + hh + 10.0 + k as f32 * 50.0), vec2(inner.width(), 40.0));
        let key = match self.provider {
            AIProvider::OpenAI | AIProvider::OpenAILegacy => &mut self.openai_key,
            AIProvider::Gemini => &mut self.gemini_key,
            AIProvider::OpenRouter => &mut self.openrouter_key,
        };
        let control = tools::label_row(ui, row(0), LABEL_W, s::AnalyzeImageOptionsWindow_API_key());
        TextBox { password: true, ..TextBox::default() }.show(ui, control, Id::new(("ai-key", self.provider.name())), &mut key.text);
        let control = tools::label_row(ui, row(1), LABEL_W, s::AnalyzeImageOptionsWindow_Model());
        match self.provider {
            AIProvider::OpenAI | AIProvider::OpenAILegacy => {
                let text = s::AnalyzeImageOptionsWindow_Load_models();
                let bw = tools::icon_text_button_width(ui, text);
                let br = Rect::from_min_size(pos2(control.max.x - bw, control.min.y), vec2(bw, 40.0));
                let area = Rect::from_min_max(control.min, pos2(br.min.x - 10.0, control.max.y));
                Self::model_combo(ui, area, "openai", &self.openai_models.clone(), &mut self.openai_model);
                if tools::icon_text_button(ui, br, Id::new("ai-load-models"), icon::Refresh, text, !self.is_busy).clicked() {
                    self.load_models(&ctx);
                }
                let control = tools::label_row(ui, row(2), LABEL_W, s::AnalyzeImageOptionsWindow_Custom_base_URL());
                TextBox { placeholder: s::AnalyzeImageOptionsWindow_https_api_openai_com(), ..TextBox::default() }.show(ui, control, Id::new("ai-url"), &mut self.openai_custom_url);
                // `ColumnDefinitions="120,*,120,*" ColumnSpacing="10"`.
                let r = row(3);
                let star = (r.width() - LABEL_W * 2.0 - 30.0) / 2.0;
                let combo = Self::combo();
                tools::row_label(ui, Rect::from_min_size(r.min, vec2(LABEL_W, 40.0)), s::AnalyzeImageOptionsWindow_Reasoning());
                let items = level_items(&ai::REASONING_EFFORTS);
                let sel = ai::REASONING_EFFORTS.iter().position(|v| *v == self.openai_reasoning_effort);
                let x = r.min.x + LABEL_W + 10.0;
                let w = combo.auto_width(ui, &items, sel).min(star);
                if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(x, r.min.y), vec2(w, 40.0)), Id::new("ai-reasoning"), &items, sel) {
                    self.openai_reasoning_effort = ai::REASONING_EFFORTS[i].to_owned();
                }
                let x = x + star + 10.0;
                tools::row_label(ui, Rect::from_min_size(pos2(x, r.min.y), vec2(LABEL_W, 40.0)), s::AnalyzeImageOptionsWindow_Verbosity());
                let items = level_items(&ai::VERBOSITY_LEVELS);
                let sel = ai::VERBOSITY_LEVELS.iter().position(|v| *v == self.openai_verbosity);
                let x = x + LABEL_W + 10.0;
                let w = combo.auto_width(ui, &items, sel).min(star);
                if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(x, r.min.y), vec2(w, 40.0)), Id::new("ai-verbosity"), &items, sel) {
                    self.openai_verbosity = ai::VERBOSITY_LEVELS[i].to_owned();
                }
            }
            AIProvider::Gemini => Self::model_combo(ui, control, "gemini", &self.gemini_models.clone(), &mut self.gemini_model),
            AIProvider::OpenRouter => Self::model_combo(ui, control, "openrouter", &self.openrouter_models.clone(), &mut self.openrouter_model),
        }
    }

    fn behavior_panel(&mut self, ui: &mut Ui, rect: Rect) {
        let p = palette(ui.ctx());
        tools::panel(ui, rect);
        let inner = rect.shrink(13.0);
        let hh = tools::heading_height(ui);
        tools::heading(ui, Rect::from_min_size(inner.min, vec2(inner.width(), hh)), s::AnalyzeImageOptionsWindow_Behavior());
        let rows: [(&str, &mut bool); 3] = [
            (s::AnalyzeImageOptionsWindow_Capture_a_region_when_opened_without_an_image(), &mut self.auto_start_region),
            (s::AnalyzeImageOptionsWindow_Analyze_automatically_when_opened(), &mut self.auto_start_analyze),
            (s::AnalyzeImageOptionsWindow_Copy_result_automatically(), &mut self.auto_copy_result),
        ];
        for (k, (text, on)) in rows.into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(inner.min.x, inner.min.y + hh + 8.0 + k as f32 * (TOGGLE_ROW + 8.0)), vec2(inner.width(), TOGGLE_ROW));
            sharex_ui::controls::label(ui, Rect::from_min_max(r.min, pos2(r.max.x - ToggleSwitch::SIZE.x, r.max.y)), text, tools::normal_font(), p.text);
            ToggleSwitch { enabled: true }.show(ui, pos2(r.max.x - ToggleSwitch::SIZE.x, center_y(r, ToggleSwitch::SIZE.y)), Id::new(("ai-behavior", k)), on);
        }
    }
}

impl Dialog for AnalyzeImageOptionsWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::AnalyzeImageOptionsWindow_ShareX_Analyze_image_options()), WIDTH, HEIGHT, 580.0, 540.0)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let ctx = ui.ctx().clone();
        if let Some(rx) = &self.rx
            && let Ok(work) = rx.try_recv()
        {
            self.rx = None;
            self.finish(work);
        }
        let p = palette(ui.ctx());
        let mut out = Vec::new();
        let c = tools::background(ui);

        // Provider, API key page.
        let top = Rect::from_min_size(c.min, vec2(c.width(), 66.0));
        tools::panel(ui, top);
        let row = top.shrink(13.0);
        let page = s::AnalyzeImageOptionsWindow_API_key_page();
        let pw = tools::icon_text_button_width(ui, page);
        let pr = Rect::from_min_size(pos2(row.max.x - pw, row.min.y), vec2(pw, 40.0));
        tools::row_label(ui, Rect::from_min_size(row.min, vec2(PROVIDER_LABEL_W, 40.0)), s::AnalyzeImageOptionsWindow_Provider());
        let items = provider_items();
        let sel = AIProvider::ALL.iter().position(|x| *x == self.provider);
        let combo = Self::combo();
        let cw = combo.auto_width(ui, &items, sel);
        if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(row.min.x + PROVIDER_LABEL_W + 10.0, row.min.y), vec2(cw, 40.0)), Id::new("ai-provider"), &items, sel) {
            self.provider = AIProvider::ALL[i];
        }
        sharex_ui::tooltip::show(ui, pr, Id::new("ai-key-page-tip"), s::AnalyzeImageOptionsWindow_Get_API_key());
        if tools::icon_text_button(ui, pr, Id::new("ai-key-page"), icon::Key, page, true).clicked() {
            let mut o = self.create_options();
            o.provider = Some(self.provider);
            if let Some(url) = o.api_key_page() {
                out.push(Effect::OpenUrl(url.to_owned()));
            }
        }

        // Bottom: Test connection, the status, Save, Cancel.
        let bottom = Rect::from_min_max(pos2(c.min.x, c.max.y - 40.0), c.max);
        let test = s::AnalyzeImageOptionsWindow_Test_connection();
        let tw = tools::icon_text_button_width(ui, test);
        let tr = Rect::from_min_size(bottom.min, vec2(tw, 40.0));
        let cancel = s::AnalyzeImageOptionsWindow_Cancel();
        let cw = tools::text_button_width(ui, cancel);
        let cr = Rect::from_min_size(pos2(bottom.max.x - cw, bottom.min.y), vec2(cw, 40.0));
        let save = s::AnalyzeImageOptionsWindow_Save();
        let sw = tools::text_button_width(ui, save);
        let sr = Rect::from_min_size(pos2(cr.min.x - 8.0 - sw, bottom.min.y), vec2(sw, 40.0));
        if tools::icon_text_button(ui, tr, Id::new("ai-test"), icon::Plug, test, !self.is_busy).clicked() {
            self.test_connection(&ctx);
        }
        if !self.status_text.trim().is_empty() {
            let f = tools::normal_font();
            let area = Rect::from_min_max(pos2(tr.max.x + 8.0, bottom.min.y), pos2(sr.min.x - 8.0, bottom.max.y));
            let text = tools::ellipsize(ui, &self.status_text, &f, area.width());
            sharex_ui::controls::label(ui, area, &text, f, p.text_secondary);
        }
        if tools::text_button(ui, sr, Id::new("ai-save"), save, !self.is_busy).clicked() {
            // `Save`: the prompt kept, the options handed back.
            let mut updated = self.create_options();
            updated.input = self.target.input.clone();
            if let Some(tx) = &self.reply {
                let _ = tx.send(updated);
            }
            out.push(Effect::Close);
        }
        if tools::text_button(ui, cr, Id::new("ai-cancel"), cancel, !self.is_busy).clicked() {
            out.push(Effect::Close);
        }

        // The provider's panel and Behavior, scrolled.
        let area = Rect::from_min_max(pos2(c.min.x, top.max.y + 12.0), pos2(c.max.x, bottom.min.y - 12.0));
        let ph = self.panel_height(ui);
        let bh = Self::behavior_height(ui);
        let content = ph + 12.0 + bh;
        let max_scroll = (content - area.height()).max(0.0);
        if ui.rect_contains_pointer(area) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
        child.set_clip_rect(area.intersect(ui.clip_rect()));
        let y = area.min.y - self.scroll;
        self.provider_panel(&mut child, Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), ph)));
        self.behavior_panel(&mut child, Rect::from_min_size(pos2(area.min.x, y + ph + 12.0), vec2(area.width(), bh)));
        if max_scroll > 0.0 {
            crate::dialogs::layout::scroll_bar(ui, area, content, self.scroll, 0.0);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_keeps_the_prompt_and_untouched_keys() {
        let mut target = AIOptions::default();
        target.input = Some("my prompt".into());
        let (tx, rx) = mpsc::channel();
        let mut w = AnalyzeImageOptionsWindow::new(target.clone(), Some(tx));
        assert_eq!(w.openai_models.len(), 5);
        w.provider = AIProvider::Gemini;
        w.gemini_key.text = "key".into();
        let o = w.create_options();
        assert_eq!(o.provider, Some(AIProvider::Gemini));
        assert_eq!(o.gemini_api_key.get(), Some("key"));
        assert_eq!(o.openai_api_key, target.openai_api_key);
        assert!(o.has_api_key());
        w.set_models(vec!["a".into(), "b".into()]);
        assert_eq!(w.openai_model, "a");
        assert_eq!(w.status_text, super::super::format(s::AnalyzeImageOptionsViewModel_Models_loaded(), &["2"]));
        drop(w);
        assert!(rx.try_recv().is_err());
        assert_eq!(model_list(&ai::GEMINI_MODELS, "gemini-1.5-flash-latest").len(), 5);
    }
}
