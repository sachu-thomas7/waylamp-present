use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum SceneSource {
    SlideDeck,
    SceneMedia,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SceneMediaPayload {
    pub media_type: SceneMediaType,
    pub path: String,
    pub display_name: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum SceneMediaType {
    Image,
    Video,
}

pub struct SceneState {
    pub active_source: SceneSource,
    pub current_media: Option<SceneMediaPayload>,
}

pub type SceneStateHandle = Arc<RwLock<SceneState>>;

pub fn new_scene_handle() -> SceneStateHandle {
    Arc::new(RwLock::new(SceneState {
        active_source: SceneSource::SlideDeck,
        current_media: None,
    }))
}
