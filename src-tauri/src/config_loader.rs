// src-tauri/src/config_loader.rs — full file

use std::path::PathBuf;
use std::sync::Arc;

use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;

use crate::bus::EventBus;
use crate::config::StreamDeckConfig;
use crate::error::AppError;
use crate::events::AppEvent;

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load_or_default(path: &PathBuf) -> Result<StreamDeckConfig, AppError> {
        if !path.exists() {
            let default = StreamDeckConfig::default();
            let toml_str = toml::to_string_pretty(&default)
                .map_err(|e| AppError::Config(e.to_string()))?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| AppError::Config(e.to_string()))?;
            }
            std::fs::write(path, toml_str)
                .map_err(|e| AppError::Config(e.to_string()))?;
            return Ok(default);
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| AppError::Config(e.to_string()))?;
        toml::from_str::<StreamDeckConfig>(&content)
            .map_err(|e| AppError::Config(e.to_string()))
    }

    pub fn spawn_watcher(
        path: PathBuf,
        bus: Arc<EventBus>,
    ) -> Result<RecommendedWatcher, AppError> {
        let watch_path = path.clone();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_))
                    && event.paths.iter().any(|p| p == &watch_path)
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
        let path = dir.path().join("streamdeck.toml");
        let config = ConfigLoader::load_or_default(&path).unwrap();
        assert_eq!(config.version, crate::config::CONFIG_VERSION);
        assert!(path.exists());
    }

    #[test]
    fn valid_toml_parses_correctly() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("streamdeck.toml");
        fs::write(&path, "version = 1\n[app]\nlog_level = \"debug\"\n").unwrap();
        let config = ConfigLoader::load_or_default(&path).unwrap();
        assert_eq!(config.app.log_level, "debug");
    }

    #[test]
    fn invalid_toml_returns_config_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("streamdeck.toml");
        fs::write(&path, "not valid toml ][[[").unwrap();
        let result = ConfigLoader::load_or_default(&path);
        assert!(matches!(result, Err(AppError::Config(_))));
    }

    #[test]
    fn v0_config_missing_new_fields_applies_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("streamdeck.toml");
        fs::write(&path, "version = 0\n").unwrap();
        let config = ConfigLoader::load_or_default(&path).unwrap();
        assert_eq!(config.version, 0);
        assert_eq!(config.app.log_level, "info");
    }
}