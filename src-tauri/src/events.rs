use crate::display::DisplayInfo;
use crate::pipeline::PipelineState;
use crate::scene::SceneMediaPayload;
use crate::scene::SceneSource;
use crate::slide::SlidePayload;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload")]
#[non_exhaustive]
pub enum AppEvent {
    BusLag {
        receiver_id: String,
        dropped: u64,
    },
    Ping {
        message: String,
    },
    ConfigReloaded,
    SlideUpdate(SlidePayload),
    PresentationVisibility {
        visible: bool,
    },
    SlideAdvance,
    SlideRewind,
    DeckEnd,
    DeckStart,
    TimerTick {
        elapsed_secs: u64,
    },
    TimerReset,
    DeckImportProgress {
        slide: u32,
        total: u32,
    },
    DeckImportComplete {
        deck_id: String,
    },
    DeckImportFailed {
        slide: u32,
        reason: String,
    },
    SceneMediaUpdate(SceneMediaPayload),
    SceneSourceChanged {
        source: SceneSource,
    },
    DisplaysChanged(Vec<DisplayInfo>),
    DisplayAssigned {
        display_id: String,
    },
    DisplayLost,
    PipelineStateChanged {
        state: PipelineState,
    },
    PipelineError {
        element: String,
        message: String,
    },
    PipelineWarning {
        element: String,
        message: String,
    },
    PipelineLatency {
        element: String,
    },
    PipelineQos {
        element: String,
        processed: u64,
        dropped: u64,
    },
}
