//! `ApplicationState.UploadersConfig` with `SettingManager`'s handling of
//! it: loaded in the background at startup (`LoadInitialSettings`),
//! waited for before an upload (`WaitUploadersConfig`), saved with the
//! other settings (`SaveAllSettings`, `SaveUploadersConfigAsync`).

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use sharex_settings::settings_base::{self, SettingsFile};
use sharex_uploaders::UploadersConfig;

/// The shared `UploadersConfig`: the UI thread and every task's thread use
/// the same one, as ShareX's uploaders share `ApplicationState`'s.
#[derive(Clone, Default)]
pub struct UploadersConfigStore(Arc<(Mutex<Option<UploadersConfig>>, Condvar)>);

impl UploadersConfigStore {
    /// A store with `config` already loaded.
    #[cfg(test)]
    pub fn loaded(config: UploadersConfig) -> Self {
        Self(Arc::new((Mutex::new(Some(config)), Condvar::new())))
    }

    /// `LoadUploadersConfig` on a worker thread, as `LoadInitialSettings`
    /// does: `path` (`None` in sandbox mode: new settings), with ShareX's
    /// fallbacks to the backups in `backup_folder`, then the backward
    /// compatibility tasks.
    pub fn load_in_background(path: Option<PathBuf>, backup_folder: PathBuf) -> Self {
        let store = Self::default();
        let s = store.clone();
        std::thread::Builder::new()
            .name("uploaders-config".into())
            .spawn(move || {
                let loaded = UploadersConfig::load(path.as_deref(), Some(&backup_folder), true);
                match &loaded.source {
                    Some(p) => eprintln!("UploadersConfig loaded: {}", p.display()),
                    None => eprintln!("UploadersConfig: new settings"),
                }
                s.set(loaded.settings);
            })
            .expect("uploaders config thread");
        store
    }

    fn set(&self, config: UploadersConfig) {
        let (lock, loaded) = &*self.0;
        *lock.lock().expect("uploaders config") = Some(config);
        loaded.notify_all();
    }

    /// `WaitUploadersConfig`, then the config, locked.
    pub fn wait(&self) -> MutexGuard<'_, Option<UploadersConfig>> {
        let (lock, loaded) = &*self.0;
        let guard = lock.lock().expect("uploaders config");
        loaded.wait_while(guard, |c| c.is_none()).expect("uploaders config")
    }

    /// Runs `f` on the config once it's loaded.
    pub fn with<R>(&self, f: impl FnOnce(&mut UploadersConfig) -> R) -> R {
        let mut guard = self.wait();
        f(guard.as_mut().expect("loaded"))
    }

    /// The config as `UploadersConfig.json`, with the current version, if
    /// it's loaded (`if (UploadersConfig != null) Save`).
    pub fn to_json(&self) -> Option<String> {
        let (lock, _) = &*self.0;
        let mut guard = lock.lock().expect("uploaders config");
        let config = guard.as_mut()?;
        config.set_application_version(settings_base::application_version().to_owned());
        Some(settings_base::to_json(config))
    }
}
