//! Comandos de staging: el único camino para tocar un archivo real de
//! `~/.claude` es `stage_change` seguido de `apply_staged` — no hay ningún
//! comando de "escribir directo".

use cf_core::staging::{AppliedChange, StagedChange, StagingStore};

use crate::error::AppError;
use crate::paths::{app_data_dir, claude_dir, ensure_within_claude_dir};

fn store(app: &tauri::AppHandle) -> Result<StagingStore, AppError> {
    Ok(StagingStore::new(&app_data_dir(app)?))
}

#[tauri::command]
pub fn stage_change(
    app: tauri::AppHandle,
    target_path: String,
    draft_content: String,
) -> Result<StagedChange, AppError> {
    let dir = claude_dir(&app)?;
    let target = std::path::PathBuf::from(target_path);
    ensure_within_claude_dir(&dir, &target)?;
    Ok(store(&app)?.stage(&target, draft_content)?)
}

#[tauri::command]
pub fn list_staged(app: tauri::AppHandle) -> Result<Vec<StagedChange>, AppError> {
    Ok(store(&app)?.list()?)
}

#[tauri::command]
pub fn diff_staged(app: tauri::AppHandle, id: String) -> Result<String, AppError> {
    Ok(store(&app)?.diff(&id)?)
}

#[tauri::command]
pub fn discard_staged(app: tauri::AppHandle, id: String) -> Result<(), AppError> {
    Ok(store(&app)?.discard(&id)?)
}

#[tauri::command]
pub fn apply_staged(app: tauri::AppHandle, id: String) -> Result<AppliedChange, AppError> {
    Ok(store(&app)?.apply(&id)?)
}

#[tauri::command]
pub fn list_history(app: tauri::AppHandle) -> Result<Vec<AppliedChange>, AppError> {
    Ok(store(&app)?.history()?)
}

#[tauri::command]
pub fn revert_applied(app: tauri::AppHandle, id: String) -> Result<AppliedChange, AppError> {
    Ok(store(&app)?.revert(&id)?)
}
