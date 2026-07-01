//! Comandos de staging: el único camino para tocar un archivo real de
//! `~/.claude` es `stage_change` seguido de `apply_staged` — no hay ningún
//! comando de "escribir directo". Son `async fn` + `blocking(...)`: el I/O
//! (leer/escribir borradores, backups, historial) no corre en el hilo principal.

use cf_core::staging::{AppliedChange, StagedChange, StagingStore};

use crate::commands::blocking;
use crate::error::AppError;
use crate::paths::{app_data_dir, claude_dir, ensure_within_claude_dir};

fn store(app: &tauri::AppHandle) -> Result<StagingStore, AppError> {
    // Boundary cableado a ~/.claude: cf-core revalida cada escritura (apply /
    // revert) contra este root, no solo el chequeo de abajo al crear el
    // borrador. Defensa en profundidad ante drafts/history adulterados.
    Ok(StagingStore::new(&app_data_dir(app)?).with_boundary(&claude_dir(app)?))
}

#[tauri::command]
pub async fn stage_change(
    app: tauri::AppHandle,
    target_path: String,
    draft_content: String,
) -> Result<StagedChange, AppError> {
    let dir = claude_dir(&app)?;
    let target = std::path::PathBuf::from(target_path);
    ensure_within_claude_dir(&dir, &target)?;
    let store = store(&app)?;
    blocking(move || Ok(store.stage(&target, draft_content)?)).await
}

#[tauri::command]
pub async fn list_staged(app: tauri::AppHandle) -> Result<Vec<StagedChange>, AppError> {
    let store = store(&app)?;
    blocking(move || Ok(store.list()?)).await
}

#[tauri::command]
pub async fn diff_staged(app: tauri::AppHandle, id: String) -> Result<String, AppError> {
    let store = store(&app)?;
    blocking(move || Ok(store.diff(&id)?)).await
}

#[tauri::command]
pub async fn discard_staged(app: tauri::AppHandle, id: String) -> Result<(), AppError> {
    let store = store(&app)?;
    blocking(move || Ok(store.discard(&id)?)).await
}

#[tauri::command]
pub async fn apply_staged(app: tauri::AppHandle, id: String) -> Result<AppliedChange, AppError> {
    let store = store(&app)?;
    blocking(move || Ok(store.apply(&id)?)).await
}

#[tauri::command]
pub async fn list_history(app: tauri::AppHandle) -> Result<Vec<AppliedChange>, AppError> {
    let store = store(&app)?;
    blocking(move || Ok(store.history()?)).await
}

#[tauri::command]
pub async fn revert_applied(app: tauri::AppHandle, id: String) -> Result<AppliedChange, AppError> {
    let store = store(&app)?;
    blocking(move || Ok(store.revert(&id)?)).await
}
