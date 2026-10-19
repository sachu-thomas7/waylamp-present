use std::path::PathBuf;
use std::sync::Arc;

use tauri::{Manager, Runtime};

use crate::bus::EventBus;
use crate::config::StreamDeckConfig;
use crate::display::{DisplayProviderHandle, DisplayStateHandle};
use crate::error::AppError;
use crate::events::AppEvent;
use crate::slide::{SlideDeck, SlidePayload};
use crate::slide_state_machine::{self, StateMachineHandle};

/// The app data directory, managed so commands (and tests) never re-resolve it from the OS.
pub struct AppDataDir(pub PathBuf);

/// Path of `streamdeck.toml`, managed so commands (and tests) never re-resolve it from the OS.
pub struct ConfigPath(pub PathBuf);

pub struct TimerAbortHandle(pub tokio::task::AbortHandle);

/// Registers the managed state shared by the app and the mock-runtime test harness.
pub fn manage_core<R: Runtime, M: Manager<R>>(
    manager: &M,
    bus: Arc<EventBus>,
    data_dir: PathBuf,
    config_path: PathBuf,
    config: StreamDeckConfig,
) -> StateMachineHandle {
    let machine = slide_state_machine::new_handle(SlideDeck::stub(), data_dir.clone());
    manager.manage(AppDataDir(data_dir));
    manager.manage(ConfigPath(config_path));
    manager.manage(bus);
    manager.manage(Arc::clone(&machine));
    manager.manage(crate::scene::new_scene_handle());
    manager.manage(config);
    machine
}

/// Creates and manages the display state (seeded with the persisted assignment) and provider.
pub fn manage_display<R: Runtime, M: Manager<R>>(
    manager: &M,
    config: &StreamDeckConfig,
    provider: DisplayProviderHandle,
) -> DisplayStateHandle {
    manager.manage(provider);
    let handle = crate::display::new_display_handle(config.presentation.display_id.clone());
    manager.manage(Arc::clone(&handle));
    handle
}

/// Loads the deck named by `decks/active_deck_id.txt`; `None` when nothing is persisted.
pub async fn restore_deck(
    machine: &StateMachineHandle,
    data_dir: &std::path::Path,
) -> Result<Option<SlidePayload>, AppError> {
    let decks_dir = data_dir.join("decks");
    let Ok(raw) = std::fs::read_to_string(decks_dir.join("active_deck_id.txt")) else {
        return Ok(None);
    };
    let deck_json_path = decks_dir.join(raw.trim()).join("deck.json");
    if !deck_json_path.exists() {
        return Ok(None);
    }
    let deck = SlideDeck::load_from_json(&deck_json_path)?;
    let mut m = machine.write().await;
    m.load_deck(deck);
    Ok(Some(m.current_payload()))
}

/// Startup restore: the persisted deck first, then the saved slide index/timer. Best-effort.
pub async fn restore_on_startup(
    machine: &StateMachineHandle,
    bus: &Arc<EventBus>,
    data_dir: &std::path::Path,
) {
    if let Err(e) = restore_deck(machine, data_dir).await {
        eprintln!("Deck restore failed: {e}");
    }
    slide_state_machine::restore_state(machine, data_dir).await;
    let payload = machine.read().await.current_payload();
    bus.publish(AppEvent::SlideUpdate(payload));
}

/// Spawns the 1s timer task and keeps its abort handle in managed state. Needs a Tokio context.
pub fn spawn_timer<R: Runtime, M: Manager<R>>(
    manager: &M,
    machine: StateMachineHandle,
    bus: Arc<EventBus>,
) {
    let abort = slide_state_machine::spawn_timer_task(machine, bus);
    manager.manage(TimerAbortHandle(abort));
}

// Every command is generic over the runtime, so the mock and real handlers share one list.
macro_rules! handler {
    () => {
        tauri::generate_handler![
            crate::commands::info::get_build_info,
            crate::commands::system::ping,
            crate::commands::config::get_config,
            crate::commands::slide::slide_advance,
            crate::commands::slide::slide_rewind,
            crate::commands::slide::slide_jump_to,
            crate::commands::slide::slide_get_current,
            crate::commands::slide::slide_timer_reset,
            crate::commands::deck::deck_get_all_slides,
            crate::commands::deck::deck_import,
            crate::commands::deck::deck_restore,
            crate::commands::deck::deck_clear,
            crate::commands::scene::scene_push_image,
            crate::commands::scene::scene_push_video,
            crate::commands::scene::scene_set_source,
            crate::commands::scene::scene_get_active_source,
            crate::commands::presentation::presentation_show,
            crate::commands::presentation::presentation_hide,
            crate::commands::display::display_list,
            crate::commands::display::display_assign_presentation,
            crate::commands::pipeline::pipeline_start_test,
            crate::commands::pipeline::pipeline_pause,
            crate::commands::pipeline::pipeline_resume,
            crate::commands::pipeline::pipeline_stop,
            crate::commands::pipeline::pipeline_get_state,
            crate::commands::ndi::ndi_list_sources,
            crate::commands::ndi::ndi_add_source,
            crate::commands::ndi::ndi_remove_source,
        ]
    };
}

/// Real-app handler: every command. Used by `app.rs`.
pub fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    handler!()
}

/// Mock-runtime handler: the same command list on any runtime.
pub fn mock_invoke_handler<R: Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    handler!()
}

/// Spawns the pipeline actor with `backend` and manages its handle.
pub fn manage_pipeline<R: Runtime, M: Manager<R>>(
    manager: &M,
    backend: Box<dyn crate::pipeline::PipelineBackend>,
    bus: Arc<EventBus>,
) -> crate::pipeline::PipelineHandle {
    let handle = crate::pipeline::PipelineService::spawn(backend, bus);
    manager.manage(Arc::clone(&handle));
    handle
}

/// Manages the NDI discovery provider and the (initially empty) source state.
pub fn manage_ndi<R: Runtime, M: Manager<R>>(
    manager: &M,
    provider: crate::ndi::NdiDiscoveryHandle,
) -> crate::ndi::NdiStateHandle {
    manager.manage(provider);
    let handle = crate::ndi::new_ndi_handle();
    manager.manage(Arc::clone(&handle));
    handle
}

/// The real GStreamer backend when compiled in and initialisable, else one that reports why not.
#[cfg(feature = "gstreamer")]
pub fn production_pipeline_backend(
    bus: Arc<EventBus>,
) -> Box<dyn crate::pipeline::PipelineBackend> {
    match crate::pipeline::gst_backend::GstBackend::new(bus) {
        Ok(b) => Box::new(b),
        Err(e) => {
            eprintln!("{e}");
            Box::new(crate::pipeline::UnavailableBackend(e.to_string()))
        }
    }
}

#[cfg(not(feature = "gstreamer"))]
pub fn production_pipeline_backend(
    _bus: Arc<EventBus>,
) -> Box<dyn crate::pipeline::PipelineBackend> {
    Box::new(crate::pipeline::UnavailableBackend(
        "GStreamer support not compiled in (build with --features gstreamer)".into(),
    ))
}
