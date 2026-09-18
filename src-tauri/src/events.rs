use crate::slide::SlidePayload;
use crate::scene::SceneMediaPayload;
use crate::scene::SceneSource;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload")]
#[non_exhaustive]
pub enum AppEvent {
    BusLag { receiver_id: String, dropped: u64 },
    Ping { message: String },
    ConfigReloaded,
    SlideUpdate(SlidePayload),
    PresentationVisibility { visible: bool },
    SlideAdvance,
    SlideRewind,
    DeckEnd,
    DeckStart,
    TimerTick { elapsed_secs: u64 },
    TimerReset,
    DeckImportProgress { slide: u32, total: u32 },
    DeckImportComplete { deck_id: String },
    DeckImportFailed { slide: u32, reason: String },
    SceneMediaUpdate(SceneMediaPayload),
    SceneSourceChanged { source: SceneSource },
}