use serde::Deserialize;
use serde::Serialize;

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamDeckConfig {
    pub version: u32,
    pub app: AppConfig,
    pub presentation: PresentationConfig,
}

impl Default for StreamDeckConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            app: AppConfig::default(),
            presentation: PresentationConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PresentationConfig {
    // Derived from monitor name + position (Tauri exposes no stable hardware id) — see display.rs
    pub display_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_version_is_one() {
        assert_eq!(StreamDeckConfig::default().version, 1);
    }

    #[test]
    fn round_trip_serialise_deserialise() {
        let original = StreamDeckConfig::default();
        let serialised = toml::to_string(&original).unwrap();
        let restored: StreamDeckConfig = toml::from_str(&serialised).unwrap();
        assert_eq!(original.version, restored.version);
        assert_eq!(original.app.log_level, restored.app.log_level);
        assert_eq!(
            original.presentation.display_id,
            restored.presentation.display_id
        );
    }

    #[test]
    fn v0_config_without_presentation_section_defaults_to_none() {
        let restored: StreamDeckConfig = toml::from_str("version = 0\n").unwrap();
        assert_eq!(restored.presentation.display_id, None);
    }

    #[test]
    fn presentation_display_id_round_trips() {
        let mut original = StreamDeckConfig::default();
        original.presentation.display_id = Some("Built-in@0,0".into());
        let serialised = toml::to_string(&original).unwrap();
        let restored: StreamDeckConfig = toml::from_str(&serialised).unwrap();
        assert_eq!(
            restored.presentation.display_id,
            Some("Built-in@0,0".into())
        );
    }
}
