use crate::bus::EventBus;
use crate::error::AppError;
use crate::events::AppEvent;
use crate::scene::{SceneSource, SceneStateHandle};
use crate::slide::{SlideDeck, SlidePayload};
use crate::slide_state_machine::StateMachineHandle;
use crate::state::AppDataDir;
use std::sync::Arc;

#[tauri::command]
pub async fn deck_import(
    path: String,
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
    scene: tauri::State<'_, SceneStateHandle>,
    data_dir: tauri::State<'_, AppDataDir>,
) -> Result<String, AppError> {
    let deck_id = uuid::Uuid::new_v4().to_string();
    let data_dir = data_dir.0.clone();
    let dest_dir = data_dir.join("decks").join(&deck_id);

    let bus_arc = Arc::clone(&*bus);
    let deck =
        crate::deck_importer::import_deck(std::path::Path::new(&path), &dest_dir, bus_arc).await?;

    let deck_json_path = dest_dir.join("deck.json");
    deck.save_to_json(&deck_json_path)?;

    let active_id_path = data_dir.join("decks").join("active_deck_id.txt");
    crate::persist::atomic_write(&active_id_path, deck_id.as_bytes())?;

    let first_payload = {
        let mut m = machine.write().await;
        m.load_deck(deck);
        m.current_payload()
    };

    {
        let mut s = scene.write().await;
        s.active_source = SceneSource::SlideDeck;
    }

    bus.publish(AppEvent::SlideUpdate(first_payload));
    bus.publish(AppEvent::SceneSourceChanged {
        source: SceneSource::SlideDeck,
    });
    bus.publish(AppEvent::DeckImportComplete {
        deck_id: deck_id.clone(),
    });

    Ok(deck_id)
}

#[tauri::command]
pub async fn deck_clear(
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
    scene: tauri::State<'_, SceneStateHandle>,
    data_dir: tauri::State<'_, AppDataDir>,
) -> Result<(), AppError> {
    let active_id_path = data_dir.0.join("decks").join("active_deck_id.txt");
    // Ignore "not found" — clearing an already-empty deck is a no-op, not an error.
    let _ = std::fs::remove_file(&active_id_path);

    let payload = {
        let mut m = machine.write().await;
        m.load_deck(SlideDeck::empty());
        m.current_payload()
    };

    {
        let mut s = scene.write().await;
        s.active_source = SceneSource::SlideDeck;
    }

    bus.publish(AppEvent::SlideUpdate(payload));
    bus.publish(AppEvent::SceneSourceChanged {
        source: SceneSource::SlideDeck,
    });
    Ok(())
}

#[tauri::command]
pub async fn deck_get_all_slides(
    machine: tauri::State<'_, StateMachineHandle>,
) -> Result<Vec<SlidePayload>, AppError> {
    let m = machine.read().await;
    let total = m.deck_len() as u32;
    Ok(m.all_slides()
        .iter()
        .map(|e| SlidePayload::from_entry(e, total))
        .collect())
}

#[tauri::command]
pub async fn deck_restore(
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
    data_dir: tauri::State<'_, AppDataDir>,
) -> Result<(), AppError> {
    if let Some(first_payload) = crate::state::restore_deck(&machine, &data_dir.0).await? {
        bus.publish(AppEvent::SlideUpdate(first_payload));
    }
    Ok(())
}
