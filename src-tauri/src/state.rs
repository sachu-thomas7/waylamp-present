use std::path::PathBuf;
use std::sync::Arc;

use tauri::{Manager, Runtime};

use crate::bus::EventBus;
use crate::config::StreamDeckConfig;
use crate::slide::SlideDeck;
use crate::slide_state_machine::{self, StateMachineHandle};

/// Registers the managed state shared by the app and the mock-runtime test harness.
pub fn manage_core<R: Runtime, M: Manager<R>>(
    manager: &M,
    bus: Arc<EventBus>,
    data_dir: PathBuf,
    config: StreamDeckConfig,
) -> StateMachineHandle {
    let machine = slide_state_machine::new_handle(SlideDeck::stub(), data_dir);
    manager.manage(bus);
    manager.manage(Arc::clone(&machine));
    manager.manage(crate::scene::new_scene_handle());
    manager.manage(config);
    machine
}

// Commands generic over the runtime; the `wry_only` ones take a concrete `tauri::AppHandle`.
macro_rules! handler {
    ($($wry_only:tt)*) => {
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
            crate::commands::scene::scene_push_video,
            crate::commands::scene::scene_set_source,
            crate::commands::scene::scene_get_active_source,
            $($wry_only)*
        ]
    };
}

/// Real-app handler: every command. Used by `app.rs`.
pub fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    handler![
        crate::commands::presentation::presentation_show,
        crate::commands::presentation::presentation_hide,
        crate::commands::deck::deck_import,
        crate::commands::deck::deck_restore,
        crate::commands::deck::deck_clear,
        crate::commands::scene::scene_push_image,
        crate::commands::display::display_list,
        crate::commands::display::display_assign_presentation,
    ]
}

/// Mock-runtime handler: the runtime-generic subset (grows as commands are made generic).
pub fn mock_invoke_handler<R: Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    handler![]
}
