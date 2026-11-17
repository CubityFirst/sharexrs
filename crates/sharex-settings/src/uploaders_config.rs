//! `ShareX.UploadersLib/UploadersConfig.cs` (`UploadersConfig.json`): every
//! destination's account settings. Not ported yet: this keeps the whole file
//! as it is (in `extra`, in its order), so it can be loaded and saved
//! without losing anything once ShareXrs needs it. ShareX encrypts
//! `[JsonEncrypt]` properties with DPAPI (`SupportDPAPIEncryption`); kept as
//! written here, they stay readable by ShareX.
//!
//! ShareXrs doesn't load or save this file yet; the uploaders port will
//! type its properties (and the machine-specific copy,
//! `SettingManager.GetUploadersConfigFileName`).

use serde_json::Value;

use crate::settings;
use crate::settings_base::SettingsFile;

settings! {
    /// `ShareX.UploadersLib.UploadersConfig`, untyped for now.
    pub struct UploadersConfig {}
}

impl SettingsFile for UploadersConfig {
    fn application_version(&self) -> Option<&str> {
        self.extra.get("ApplicationVersion").and_then(Value::as_str)
    }

    fn set_application_version(&mut self, version: String) {
        self.extra.insert("ApplicationVersion".into(), Value::String(version));
    }
}
