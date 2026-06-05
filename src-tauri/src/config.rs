use serde::Serialize;
use serde::Deserialize;

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamDeckConfig {
    pub version: u32,
    pub app: AppConfig,
}

impl Default for StreamDeckConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            app: AppConfig::default(),
        }
    }
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
    }
}