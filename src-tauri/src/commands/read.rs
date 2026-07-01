//! Comandos de solo lectura: delgados a propósito, delegan todo a `cf-core`.
//! Son `async fn` + `blocking(...)` para que el I/O de filesystem no corra en
//! el hilo principal y no congele la ventana al cargar cada pestaña.

use cf_core::model::{AgentDef, MemoryEntry, SettingsSummary, Skill, SlashCommand, Workflow};

use crate::commands::blocking;
use crate::error::AppError;
use crate::paths::{claude_dir, ensure_within_claude_dir};

#[tauri::command]
pub async fn list_memories(app: tauri::AppHandle) -> Result<Vec<MemoryEntry>, AppError> {
    let dir = claude_dir(&app)?;
    blocking(move || Ok(cf_core::scan::list_memories(&dir)?)).await
}

#[tauri::command]
pub async fn list_skills(app: tauri::AppHandle) -> Result<Vec<Skill>, AppError> {
    let dir = claude_dir(&app)?;
    blocking(move || Ok(cf_core::scan::list_skills(&dir)?)).await
}

#[tauri::command]
pub async fn list_agents(app: tauri::AppHandle) -> Result<Vec<AgentDef>, AppError> {
    let dir = claude_dir(&app)?;
    blocking(move || Ok(cf_core::scan::list_agents(&dir)?)).await
}

#[tauri::command]
pub async fn list_scheduled_tasks(app: tauri::AppHandle) -> Result<Vec<Skill>, AppError> {
    let dir = claude_dir(&app)?;
    blocking(move || Ok(cf_core::scan::list_scheduled_tasks(&dir)?)).await
}

#[tauri::command]
pub async fn read_claude_md(app: tauri::AppHandle) -> Result<String, AppError> {
    let dir = claude_dir(&app)?;
    blocking(move || Ok(cf_core::scan::read_claude_md(&dir)?)).await
}

#[tauri::command]
pub async fn list_commands(app: tauri::AppHandle) -> Result<Vec<SlashCommand>, AppError> {
    let dir = claude_dir(&app)?;
    blocking(move || Ok(cf_core::scan::list_commands(&dir)?)).await
}

#[tauri::command]
pub async fn list_workflows(app: tauri::AppHandle) -> Result<Vec<Workflow>, AppError> {
    let dir = claude_dir(&app)?;
    blocking(move || Ok(cf_core::scan::list_workflows(&dir)?)).await
}

#[tauri::command]
pub async fn read_settings_summary(app: tauri::AppHandle) -> Result<SettingsSummary, AppError> {
    let dir = claude_dir(&app)?;
    blocking(move || Ok(cf_core::scan::read_settings_summary(&dir)?)).await
}

#[tauri::command]
pub async fn read_settings_raw(app: tauri::AppHandle) -> Result<String, AppError> {
    let dir = claude_dir(&app)?;
    blocking(move || Ok(cf_core::scan::read_settings_raw(&dir)?)).await
}

/// Path absoluto de `settings.json`, para que el editor genérico del frontend
/// pueda abrirlo/stagearlo igual que cualquier otro archivo.
#[tauri::command]
pub fn settings_path(app: tauri::AppHandle) -> Result<String, AppError> {
    Ok(claude_dir(&app)?
        .join("settings.json")
        .display()
        .to_string())
}

/// Lee el contenido completo de un archivo listado por cualquiera de los
/// comandos de arriba (para abrirlo en el editor). `path` viene de ese mismo
/// listado, pero igual se valida: nunca confiar en un path que cruza el IPC.
#[tauri::command]
pub async fn read_file_content(app: tauri::AppHandle, path: String) -> Result<String, AppError> {
    let dir = claude_dir(&app)?;
    let target = std::path::PathBuf::from(path);
    ensure_within_claude_dir(&dir, &target)?;
    blocking(move || {
        std::fs::read_to_string(&target).map_err(|source| AppError::Io {
            path: target,
            source,
        })
    })
    .await
}
