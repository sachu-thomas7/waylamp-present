// src-tauri/src/config_loader.rs — full file

use std::path::PathBuf;
use std::sync::Arc;

use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;

use crate::bus::EventBus;
use crate::config::WaylampConfig;
use crate::error::AppError;
use crate::events::AppEvent;

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn resolve_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, AppError> {
        use tauri::Manager;
        Ok(app
            .path()
            .app_config_dir()
            .map_err(|e| AppError::Config(e.to_string()))?
            .join("waylamp-present.toml"))
    }

    pub fn load_or_default(path: &PathBuf) -> Result<WaylampConfig, AppError> {
        if !path.exists() {
            let default = WaylampConfig::default();
            let toml_str =
                toml::to_string_pretty(&default).map_err(|e| AppError::Config(e.to_string()))?;
            crate::persist::atomic_write(path, toml_str.as_bytes())?;
            return Ok(default);
        }

        let content = std::fs::read_to_string(path).map_err(|e| AppError::Config(e.to_string()))?;
        toml::from_str::<WaylampConfig>(&content).map_err(|e| AppError::Config(e.to_string()))
    }

    pub fn save(path: &std::path::Path, config: &WaylampConfig) -> Result<(), AppError> {
        let toml_str =
            toml::to_string_pretty(config).map_err(|e| AppError::Config(e.to_string()))?;
        crate::persist::atomic_write(path, toml_str.as_bytes())
    }

    /// Startup load that never fails on a damaged file: restores the `.bak` or falls back to defaults.
    pub fn load_or_recover(path: &std::path::Path) -> (WaylampConfig, crate::persist::Recovery) {
        let (cfg, recovery) =
            crate::persist::load_recovering(path, |s| toml::from_str::<WaylampConfig>(s).ok());
        match cfg {
            Some(c) => (c, recovery),
            None => {
                let default = WaylampConfig::default();
                if let Ok(toml_str) = toml::to_string_pretty(&default) {
                    let _ = crate::persist::atomic_write(path, toml_str.as_bytes());
                }
                (default, recovery)
            }
        }
    }

    pub fn spawn_watcher(
        path: PathBuf,
        bus: Arc<EventBus>,
    ) -> Result<RecommendedWatcher, AppError> {
        let watch_path = path.clone();
        // FSEvents reports real paths (/var -> /private/var), so also match the canonical form.
        let canonical_path = path
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .zip(path.file_name())
            .map(|(dir, name)| dir.join(name));

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_))
                    && event
                        .paths
                        .iter()
                        .any(|p| p == &watch_path || Some(p) == canonical_path.as_ref())
                {
                    bus.publish(AppEvent::ConfigReloaded);
                }
            }
        })
        .map_err(|e| AppError::Config(e.to_string()))?;

        let watch_dir = path
            .parent()
            .ok_or_else(|| AppError::Config("config path has no parent dir".into()))?;

        watcher
            .watch(watch_dir, RecursiveMode::NonRecursive)
            .map_err(|e| AppError::Config(e.to_string()))?;

        Ok(watcher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn missing_file_writes_default_and_returns_it() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("waylamp-present.toml");
        let config = ConfigLoader::load_or_default(&path).unwrap();
        assert_eq!(config.version, crate::config::CONFIG_VERSION);
        assert!(path.exists());
    }

    #[test]
    fn valid_toml_parses_correctly() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("waylamp-present.toml");
        fs::write(&path, "version = 1\n[app]\nlog_level = \"debug\"\n").unwrap();
        let config = ConfigLoader::load_or_default(&path).unwrap();
        assert_eq!(config.app.log_level, "debug");
    }

    #[test]
    fn invalid_toml_returns_config_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("waylamp-present.toml");
        fs::write(&path, "not valid toml ][[[").unwrap();
        let result = ConfigLoader::load_or_default(&path);
        assert!(matches!(result, Err(AppError::Config(_))));
    }

    #[test]
    fn save_then_load_round_trips_presentation_display_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("waylamp-present.toml");
        let mut config = WaylampConfig::default();
        config.presentation.display_id = Some("Built-in@0,0".into());
        ConfigLoader::save(&path, &config).unwrap();
        let restored = ConfigLoader::load_or_default(&path).unwrap();
        assert_eq!(
            restored.presentation.display_id,
            Some("Built-in@0,0".into())
        );
    }

    #[test]
    fn v0_config_missing_new_fields_applies_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("waylamp-present.toml");
        fs::write(&path, "version = 0\n").unwrap();
        let config = ConfigLoader::load_or_default(&path).unwrap();
        assert_eq!(config.version, 0);
        assert_eq!(config.app.log_level, "info");
    }

    #[test]
    fn corrupt_config_recovers_from_bak_instead_of_aborting_startup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("waylamp-present.toml");
        let mut c = WaylampConfig::default();
        c.app.log_level = "debug".into();
        ConfigLoader::save(&path, &c).unwrap();
        ConfigLoader::save(&path, &c).unwrap();
        fs::write(&path, "log_level = \"half writ").unwrap();
        let (config, recovery) = ConfigLoader::load_or_recover(&path);
        assert_eq!(config.app.log_level, "debug");
        assert!(matches!(
            recovery,
            crate::persist::Recovery::FromBackup { .. }
        ));
        assert!(
            ConfigLoader::load_or_default(&path).is_ok(),
            "primary file is healed"
        );
    }

    #[test]
    fn corrupt_config_without_bak_starts_with_defaults_and_keeps_the_broken_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("waylamp-present.toml");
        fs::write(&path, "not valid toml ][[[").unwrap();
        let (config, recovery) = ConfigLoader::load_or_recover(&path);
        assert_eq!(config.version, crate::config::CONFIG_VERSION);
        let crate::persist::Recovery::Defaults { broken } = recovery else {
            panic!("expected Defaults")
        };
        assert_eq!(fs::read_to_string(broken).unwrap(), "not valid toml ][[[");
        assert!(ConfigLoader::load_or_default(&path).is_ok());
    }
}
