//! `sharexrs --render <screen>` for the Data tool windows, in the states
//! of `tools-data` in the reference shot manifest (docs/REFERENCE-SHOTS.md):
//!
//! - `hash-checker` (`--files a[,b]`: the first file, and the second with
//!   Compare two files on; `--state empty|single-result|compare-empty|
//!   match|mismatch|running|algorithm-menu`; hashes made at once),
//! - `index-folder` (`--content txt|html|xml|json`: the output;
//!   `--image <folder>` indexed at once; `--state format-menu|custom-css`),
//! - `metadata` (`--image <file>`, `--text <search>`; `--state
//!   strip-confirm|stripped|unsupported`; `stripped` strips a copy in the
//!   temp folder, never the file given),
//! - `analyze-image` (`--image <file>`, `--text <prompt>`; `--state
//!   loaded-unconfigured|prompt-menu|analyzing|result|api-error`: the
//!   result and error from synthetic data, no request made),
//! - `analyze-options` (`--content openai|gemini|openrouter|openai-legacy`;
//!   `--state provider-menu|model-menu|reasoning-menu|verbosity-menu|
//!   loaded-models|connection-result|auto-start-options`, the last three
//!   from synthetic data).
//!
//! A `*-menu` state draws the open combo box's dropdown on its own, as
//! ShareX's popup window is captured.

use sharex_settings::TaskSettings;
use sharex_tools::analyze_image::{AIOptions, AIProvider};
use sharex_tools::indexer::IndexerOutput;

use super::{ComboPopupShot, analyze_image, analyze_options, hash_checker, index_folder, metadata};
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    matches!(name, "hash-checker" | "index-folder" | "metadata" | "analyze-image" | "analyze-options")
}

fn files(o: &Options) -> Vec<String> {
    o.files.iter().map(|f| f.to_string_lossy().into_owned()).collect()
}

/// A copy of `path` in the temp folder (the renders never change inputs).
fn temp_copy(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("sharexrs-render-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let to = dir.join(path.file_name().ok_or("no file name")?);
    std::fs::copy(path, &to).map_err(|e| e.to_string())?;
    Ok(to)
}

pub fn render_dialog(name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let state = o.state.as_deref().unwrap_or("");
    let defaults = TaskSettings::default();
    let task = TaskSettings::default_task_settings(&defaults);
    Ok(match name {
        "hash-checker" => {
            if state == "algorithm-menu" {
                let w = hash_checker::HashCheckerWindow::new(None, &task);
                return Ok(Box::new(ComboPopupShot::new(hash_checker::HashCheckerWindow::algorithm_items(), w.selected_algorithm(), None)));
            }
            let f = files(o);
            let mut w = hash_checker::HashCheckerWindow::new(None, &task);
            w.sync = true;
            if let Some(a) = f.first() {
                w.first_file_path = a.clone();
            }
            if let Some(b) = f.get(1) {
                w.compare_two_files = true;
                w.second_file_path = b.clone();
            }
            match state {
                "" | "empty" | "single-result" | "match" | "mismatch" => {
                    if !f.is_empty() {
                        w.check(None);
                        w.poll(None);
                    }
                }
                // As the shot has it: the single file's result kept, Compare
                // two files turned on.
                "compare-empty" => {
                    if !f.is_empty() {
                        w.check(None);
                        w.poll(None);
                    }
                    w.compare_two_files = true;
                }
                "running" => w.set_running(42.0),
                other => return Err(format!("no hash-checker state {other}")),
            }
            Box::new(w)
        }
        "index-folder" => {
            let output = match o.content.as_deref().unwrap_or("txt") {
                "txt" | "text" => IndexerOutput::Txt,
                "html" => IndexerOutput::Html,
                "xml" => IndexerOutput::Xml,
                "json" => IndexerOutput::Json,
                other => return Err(format!("no index output {other}")),
            };
            if state == "format-menu" {
                let sel = index_folder::OUTPUTS.iter().position(|x| *x == output);
                return Ok(Box::new(ComboPopupShot::new(index_folder::output_items(), sel, None)));
            }
            let mut w = index_folder::DirectoryIndexerWindow::new(task, &defaults, false);
            w.set_sync();
            w.settings.output = output;
            match state {
                "" | "empty" | "generated" => {}
                "custom-css" => {
                    w.settings.output = IndexerOutput::Html;
                    w.settings.use_custom_css_file = true;
                    w.settings.custom_css_file_path = o.text.clone().unwrap_or_default();
                }
                other => return Err(format!("no index-folder state {other}")),
            }
            if let Some(folder) = &o.image {
                w.set_folder(&folder.to_string_lossy(), true);
            }
            Box::new(w)
        }
        "metadata" => {
            let file = match (&o.image, state) {
                (Some(p), "stripped") => Some(temp_copy(p)?.to_string_lossy().into_owned()),
                (Some(p), _) => Some(p.to_string_lossy().into_owned()),
                (None, _) => None,
            };
            let mut w = metadata::MetadataWindow::new(file);
            w.set_sync();
            if let Some(t) = &o.text {
                w.search_text = t.clone();
            }
            match state {
                "" | "empty" | "grouped" | "filtered" | "no-match" | "unsupported" => {}
                "strip-confirm" => w.confirming_strip = true,
                "stripped" => w.confirm_strip(),
                other => return Err(format!("no metadata state {other}")),
            }
            Box::new(w)
        }
        "analyze-image" => {
            if state == "prompt-menu" {
                let mut shot = ComboPopupShot::new(analyze_image::AnalyzeImageWindow::preset_items(), None, None);
                shot.placeholder = Some(sharex_strings::tools::AnalyzeImageWindow_Choose_a_preset_prompt());
                return Ok(Box::new(shot));
            }
            // As the reference profile has it: no auto start, no keys.
            let mut task = task;
            let mut options = AIOptions::from_json(&task.ToolsSettings.clone().unwrap_or_default().AIOptions);
            options.auto_start_region = false;
            options.auto_start_analyze = false;
            if let Some(t) = &o.text {
                options.input = Some(t.clone());
            }
            if matches!(state, "analyzing" | "result" | "api-error") {
                options.openai_api_key.set(Some("synthetic-key".into()));
            }
            task.ToolsSettings.get_or_insert_default().AIOptions = options.to_json();
            let mut w = analyze_image::AnalyzeImageWindow::new(task, &defaults, o.image.as_ref().map(|p| p.to_string_lossy().into_owned()));
            w.set_offline(Box::new(|_, _, _| Err("no requests in a render".into())));
            match state {
                "" | "empty" => {}
                // `SelectImageAsync` → `AnalyzeAsync` without a key (the
                // custom prompt typed after it).
                "loaded-unconfigured" | "custom-prompt" => w.analyze(&egui::Context::default()),
                "analyzing" => w.set_busy(),
                "result" => w.set_result(Some(SYNTHETIC_RESULT), 2468),
                "api-error" => w.set_error(SYNTHETIC_ERROR),
                other => return Err(format!("no analyze-image state {other}")),
            }
            Box::new(w)
        }
        "analyze-options" => {
            let mut options = AIOptions::default();
            options.auto_start_region = false;
            options.auto_start_analyze = false;
            options.provider = Some(match o.content.as_deref().unwrap_or("openai") {
                "openai" => AIProvider::OpenAI,
                "gemini" => AIProvider::Gemini,
                "openrouter" => AIProvider::OpenRouter,
                "openai-legacy" => AIProvider::OpenAILegacy,
                other => return Err(format!("no provider {other}")),
            });
            let mut w = analyze_options::AnalyzeImageOptionsWindow::new(options.clone(), None);
            let combo_popup = |items, sel| -> Box<dyn Dialog> { Box::new(ComboPopupShot::new(items, sel, None)) };
            match state {
                "" | "openai" | "gemini" | "openrouter" | "openai-legacy" => {}
                "provider-menu" => return Ok(combo_popup(analyze_options::provider_items(), AIProvider::ALL.iter().position(|p| Some(*p) == options.provider))),
                "model-menu" => {
                    let items = w.openai_models.iter().map(|m| sharex_ui::controls::ComboItem::text(m.clone())).collect();
                    let sel = w.openai_models.iter().position(|m| *m == w.openai_model);
                    let mut shot = ComboPopupShot::new(items, sel, None);
                    // The editable combo box's width (its text box's text).
                    shot.editable_text = Some(w.openai_model.clone());
                    return Ok(Box::new(shot));
                }
                "reasoning-menu" => {
                    let levels = &sharex_tools::analyze_image::REASONING_EFFORTS;
                    return Ok(combo_popup(analyze_options::level_items(levels), levels.iter().position(|l| *l == w.openai_reasoning_effort)));
                }
                "verbosity-menu" => {
                    let levels = &sharex_tools::analyze_image::VERBOSITY_LEVELS;
                    return Ok(combo_popup(analyze_options::level_items(levels), levels.iter().position(|l| *l == w.openai_verbosity)));
                }
                "loaded-models" => w.set_models(vec!["gpt-5.2".into(), "gpt-5.1".into(), "gpt-5-mini".into(), "gpt-5-nano".into(), "gpt-4.1".into()]),
                "connection-result" => w.set_connection_result(true, sharex_strings::tools::AnalyzeImageService_Connection_OK()),
                "auto-start-options" => {
                    w.auto_start_region = true;
                    w.auto_start_analyze = true;
                    w.auto_copy_result = true;
                }
                other => return Err(format!("no analyze-options state {other}")),
            }
            Box::new(w)
        }
        other => return Err(format!("no data tool window {other}")),
    })
}

/// The `result` state's answer (synthetic: no provider is asked).
const SYNTHETIC_RESULT: &str = "The image shows a colourful gradient background, from dark green at the bottom left to pink at the top right, with a yellow circle on the left and a blue square with a white border on the right.\n\nThere is no text in the image.";

/// The `api-error` state's message, as `HttpClient` words a 401.
const SYNTHETIC_ERROR: &str = "Response status code does not indicate success: 401 (Unauthorized).";
