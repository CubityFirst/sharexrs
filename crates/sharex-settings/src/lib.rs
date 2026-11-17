//! ShareX's settings, read and written as ShareX does, so ShareXrs and
//! ShareX can share a profile.
//!
//! - [`ApplicationConfig`] (`ApplicationConfig.json`), with the task
//!   settings ([`TaskSettings`]) every capture runs with,
//! - [`HotkeysConfig`] (`HotkeysConfig.json`): the hotkeys and their tasks,
//! - [`UploadersConfig`] (`UploadersConfig.json`): a placeholder until the
//!   uploaders are ported,
//! - [`settings_base`]: loading (with ShareX's fallbacks) and saving (with
//!   its backups), [`paths`]: where the files live (portable mode included),
//!   [`SettingManager`]: when ShareX loads and saves them.
//!
//! Property names, order, enum names and number formats are Newtonsoft's,
//! as ShareX writes them; properties this version doesn't know are kept.
//! Classes of tools that aren't ported yet are kept as JSON ([`defaults`]).

pub mod macros;

pub mod application_config;
pub mod defaults;
pub mod enums;
pub mod hotkeys;
pub mod manager;
pub mod paths;
pub mod region_capture;
pub mod settings_base;
pub mod task_settings;
pub mod types;
pub mod uploaders_config;

pub use application_config::{ApplicationConfig, ApplicationThemeOptions, QuickTaskInfo};
pub use enums::*;
pub use hotkeys::{HotkeyInfo, HotkeySettings, HotkeyStatus, HotkeysConfig, InputKey, default_hotkey_list};
pub use manager::SettingManager;
pub use paths::{AppPaths, SystemOptions};
pub use region_capture::RegionCaptureOptions;
pub use settings_base::{Loaded, SaveOptions, SettingsFile};
pub use task_settings::{
    TaskSettings, TaskSettingsAdvanced, TaskSettingsCapture, TaskSettingsGeneral, TaskSettingsImage, TaskSettingsTools,
    TaskSettingsUpload,
};
pub use uploaders_config::UploadersConfig;
