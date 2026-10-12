use crate::bus::EventBus;
use crate::error::AppError;
use crate::slide::SlidePayload;
use crate::slide_state_machine::{self, StateMachineHandle};
use std::sync::Arc;

#[tauri::command]
pub async fn slide_advance(
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
) -> Result<(), AppError> {
    slide_state_machine::advance(&machine, &bus).await
}

#[tauri::command]
pub async fn slide_rewind(
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
) -> Result<(), AppError> {
    slide_state_machine::rewind(&machine, &bus).await
}

#[tauri::command]
pub async fn slide_jump_to(
    index: usize,
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
) -> Result<(), AppError> {
    slide_state_machine::jump_to(index, &machine, &bus).await
}

#[tauri::command]
pub async fn slide_get_current(
    machine: tauri::State<'_, StateMachineHandle>,
) -> Result<SlidePayload, AppError> {
    Ok(machine.read().await.current_payload())
}

#[tauri::command]
pub async fn slide_timer_reset(
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
) -> Result<(), AppError> {
    slide_state_machine::reset_timer(&machine, &bus).await
}
