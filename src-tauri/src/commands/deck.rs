use crate::bus::EventBus;
use crate::error::AppError;
use crate::events::AppEvent;
use crate::scene::{SceneSource, SceneStateHandle};
use crate::slide::{SlideDeck, SlidePayload};
use crate::slide_state_machine::StateMachineHandle;
use crate::state::AppDataDir;
use std::sync::Arc;

/// What the operator sees before a freshly parsed deck replaces the live one.
#[derive(Debug, serde::Serialize)]
pub struct ImportPreview {
    pub staging_id: String,
    pub slide_count: u32,
    pub skipped: u32,
    pub first_titles: Vec<String>,
}

/// A parsed but not yet activated deck; only one is held, a newer stage replaces it.
pub struct StagedImport {
    id: String,
    deck: SlideDeck,
}

#[derive(Default)]
pub struct StagedImports(pub std::sync::Mutex<Option<StagedImport>>);

fn take_staged(staged: &StagedImports, id: Option<&str>) -> Result<Option<StagedImport>, AppError> {
    let mut slot = staged
        .0
        .lock()
        .map_err(|_| AppError::Internal("staged import lock poisoned".into()))?;
    match (slot.as_ref(), id) {
        (Some(s), Some(want)) if s.id != want => Err(AppError::NotFound(
            "that import is no longer pending".into(),
        )),
        _ => Ok(slot.take()),
    }
}

async fn stage(
    path: &str,
    bus: &Arc<EventBus>,
    staged: &StagedImports,
    data_dir: &std::path::Path,
) -> Result<ImportPreview, AppError> {
    let deck_id = uuid::Uuid::new_v4().to_string();
    let dest_dir = data_dir.join("decks").join(&deck_id);
    let report = crate::deck_importer::import_deck_reporting(
        std::path::Path::new(path),
        &dest_dir,
        Arc::clone(bus),
    )
    .await
    .inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&dest_dir);
    })?;
    let preview = ImportPreview {
        staging_id: deck_id.clone(),
        slide_count: report.deck.len() as u32,
        skipped: report.skipped,
        first_titles: report
            .deck
            .slides
            .iter()
            .take(5)
            .map(|s| s.title.clone())
            .collect(),
    };
    if let Some(old) = take_staged(staged, None)? {
        let _ = std::fs::remove_dir_all(data_dir.join("decks").join(old.id));
    }
    *staged
        .0
        .lock()
        .map_err(|_| AppError::Internal("staged import lock poisoned".into()))? =
        Some(StagedImport {
            id: deck_id,
            deck: report.deck,
        });
    Ok(preview)
}

async fn activate(
    staged: StagedImport,
    bus: &Arc<EventBus>,
    machine: &StateMachineHandle,
    scene: &SceneStateHandle,
    data_dir: &std::path::Path,
) -> Result<String, AppError> {
    let deck_id = staged.id;
    let deck_json_path = data_dir.join("decks").join(&deck_id).join("deck.json");
    staged.deck.save_to_json(&deck_json_path)?;

    let active_id_path = data_dir.join("decks").join("active_deck_id.txt");
    crate::persist::atomic_write(&active_id_path, deck_id.as_bytes())?;

    let first_payload = {
        let mut m = machine.write().await;
        m.load_deck(staged.deck);
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

/// Parses a file into a staging slot without touching the live deck.
#[tauri::command]
pub async fn deck_import_stage(
    path: String,
    bus: tauri::State<'_, Arc<EventBus>>,
    staged: tauri::State<'_, StagedImports>,
    data_dir: tauri::State<'_, AppDataDir>,
) -> Result<ImportPreview, AppError> {
    stage(&path, &bus, &staged, &data_dir.0).await
}

/// Makes the staged deck live ("Use this deck").
#[tauri::command]
pub async fn deck_import_commit(
    staging_id: String,
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
    scene: tauri::State<'_, SceneStateHandle>,
    staged: tauri::State<'_, StagedImports>,
    data_dir: tauri::State<'_, AppDataDir>,
) -> Result<String, AppError> {
    let s = take_staged(&staged, Some(&staging_id))?
        .ok_or_else(|| AppError::NotFound("nothing is staged".into()))?;
    activate(s, &bus, &machine, &scene, &data_dir.0).await
}

#[tauri::command]
pub async fn deck_import_discard(
    staging_id: String,
    staged: tauri::State<'_, StagedImports>,
    data_dir: tauri::State<'_, AppDataDir>,
) -> Result<(), AppError> {
    if let Ok(Some(s)) = take_staged(&staged, Some(&staging_id)) {
        let _ = std::fs::remove_dir_all(data_dir.0.join("decks").join(s.id));
    }
    Ok(())
}

/// Stage and activate in one step, for automation and tests; the UI uses stage then commit.
#[tauri::command]
pub async fn deck_import(
    path: String,
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
    scene: tauri::State<'_, SceneStateHandle>,
    staged: tauri::State<'_, StagedImports>,
    data_dir: tauri::State<'_, AppDataDir>,
) -> Result<String, AppError> {
    let preview = stage(&path, &bus, &staged, &data_dir.0).await?;
    let s = take_staged(&staged, Some(&preview.staging_id))?
        .ok_or_else(|| AppError::Internal("staged deck vanished".into()))?;
    activate(s, &bus, &machine, &scene, &data_dir.0).await
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
