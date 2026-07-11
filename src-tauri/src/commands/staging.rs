//! Comandos de staging: el único camino para tocar un archivo real de
//! `~/.claude` es `stage_change` seguido de `apply_staged` — no hay ningún
//! comando de "escribir directo". Son `async fn` + `blocking(...)`: el I/O
//! (leer/escribir borradores, backups, historial) no corre en el hilo principal.
//!
//! Cada comando `#[tauri::command]` es un wrapper delgado (resuelve
//! `claude_dir`/`app_data_dir` desde el `AppHandle`) sobre una función `_impl`
//! que hace el trabajo real con paths explícitos. La separación no es
//! cosmética: `AppHandle` solo se puede construir con una ventana real de
//! Tauri, así que sin este corte los tests de integración no podrían ejercitar
//! el código real de `apply`/`revert` con I/O de disco de verdad. Ver
//! `src-tauri/tests/staging_integration.rs`.

use std::path::{Path, PathBuf};

use cf_core::staging::{AppliedChange, StagedChange, StagingStore};

use crate::commands::blocking;
use crate::error::AppError;
use crate::paths::{app_data_dir, claude_dir, ensure_within_claude_dir};

fn store_at(app_data_dir: &Path, claude_dir: &Path) -> StagingStore {
    // Boundary cableado a claude_dir: cf-core revalida cada escritura (apply /
    // revert) contra este root, no solo el chequeo de abajo al crear el
    // borrador. Defensa en profundidad ante drafts/history adulterados.
    StagingStore::new(app_data_dir).with_boundary(claude_dir)
}

#[tauri::command]
pub async fn stage_change(
    app: tauri::AppHandle,
    target_path: String,
    draft_content: String,
) -> Result<StagedChange, AppError> {
    stage_change_impl(
        claude_dir(&app)?,
        app_data_dir(&app)?,
        target_path,
        draft_content,
    )
    .await
}

pub async fn stage_change_impl(
    claude_dir: PathBuf,
    app_data_dir: PathBuf,
    target_path: String,
    draft_content: String,
) -> Result<StagedChange, AppError> {
    let target = PathBuf::from(target_path);
    ensure_within_claude_dir(&claude_dir, &target)?;
    let store = store_at(&app_data_dir, &claude_dir);
    let target_log = target.display().to_string();
    log_result(
        "stage_change",
        &target_log,
        blocking(move || Ok(store.stage(&target, draft_content)?)).await,
    )
}

#[tauri::command]
pub async fn list_staged(app: tauri::AppHandle) -> Result<Vec<StagedChange>, AppError> {
    list_staged_impl(claude_dir(&app)?, app_data_dir(&app)?).await
}

pub async fn list_staged_impl(
    claude_dir: PathBuf,
    app_data_dir: PathBuf,
) -> Result<Vec<StagedChange>, AppError> {
    let store = store_at(&app_data_dir, &claude_dir);
    blocking(move || Ok(store.list()?)).await
}

#[tauri::command]
pub async fn diff_staged(app: tauri::AppHandle, id: String) -> Result<String, AppError> {
    diff_staged_impl(claude_dir(&app)?, app_data_dir(&app)?, id).await
}

pub async fn diff_staged_impl(
    claude_dir: PathBuf,
    app_data_dir: PathBuf,
    id: String,
) -> Result<String, AppError> {
    let store = store_at(&app_data_dir, &claude_dir);
    blocking(move || Ok(store.diff(&id)?)).await
}

#[tauri::command]
pub async fn discard_staged(app: tauri::AppHandle, id: String) -> Result<(), AppError> {
    discard_staged_impl(claude_dir(&app)?, app_data_dir(&app)?, id).await
}

pub async fn discard_staged_impl(
    claude_dir: PathBuf,
    app_data_dir: PathBuf,
    id: String,
) -> Result<(), AppError> {
    let store = store_at(&app_data_dir, &claude_dir);
    let id_log = id.clone();
    log_result(
        "discard_staged",
        &id_log,
        blocking(move || Ok(store.discard(&id)?)).await,
    )
}

#[tauri::command]
pub async fn apply_staged(app: tauri::AppHandle, id: String) -> Result<AppliedChange, AppError> {
    apply_staged_impl(claude_dir(&app)?, app_data_dir(&app)?, id).await
}

pub async fn apply_staged_impl(
    claude_dir: PathBuf,
    app_data_dir: PathBuf,
    id: String,
) -> Result<AppliedChange, AppError> {
    let store = store_at(&app_data_dir, &claude_dir);
    let id_log = id.clone();
    log_result(
        "apply_staged",
        &id_log,
        blocking(move || Ok(store.apply(&id)?)).await,
    )
}

#[tauri::command]
pub async fn list_history(app: tauri::AppHandle) -> Result<Vec<AppliedChange>, AppError> {
    list_history_impl(claude_dir(&app)?, app_data_dir(&app)?).await
}

pub async fn list_history_impl(
    claude_dir: PathBuf,
    app_data_dir: PathBuf,
) -> Result<Vec<AppliedChange>, AppError> {
    let store = store_at(&app_data_dir, &claude_dir);
    blocking(move || Ok(store.history()?)).await
}

#[tauri::command]
pub async fn revert_applied(app: tauri::AppHandle, id: String) -> Result<AppliedChange, AppError> {
    revert_applied_impl(claude_dir(&app)?, app_data_dir(&app)?, id).await
}

pub async fn revert_applied_impl(
    claude_dir: PathBuf,
    app_data_dir: PathBuf,
    id: String,
) -> Result<AppliedChange, AppError> {
    let store = store_at(&app_data_dir, &claude_dir);
    let id_log = id.clone();
    log_result(
        "revert_applied",
        &id_log,
        blocking(move || Ok(store.revert(&id)?)).await,
    )
}

/// Loguea el resultado de un comando de mutación (éxito con INFO, fallo con
/// ERROR incluyendo el AppError) y devuelve el resultado sin tocarlo. Es el
/// único punto donde se deja rastro de qué se intentó tocar en ~/.claude.
fn log_result<T>(cmd: &str, subject: &str, result: Result<T, AppError>) -> Result<T, AppError> {
    match &result {
        Ok(_) => log::info!("{cmd} ok: {subject}"),
        Err(e) => log::error!("{cmd} falló ({subject}): {e}"),
    }
    result
}
