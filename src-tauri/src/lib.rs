mod commands;
mod error;
mod paths;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            // Log a stdout (dev) y a un archivo en el dir de logs de la app —
            // una app que reescribe ~/.claude no puede soportarse a ciegas.
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_memories,
            commands::list_skills,
            commands::list_agents,
            commands::list_scheduled_tasks,
            commands::list_commands,
            commands::list_workflows,
            commands::read_settings_summary,
            commands::read_settings_raw,
            commands::read_claude_md,
            commands::read_file_content,
            commands::settings_path,
            commands::stage_change,
            commands::list_staged,
            commands::diff_staged,
            commands::discard_staged,
            commands::apply_staged,
            commands::list_history,
            commands::revert_applied,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar claude-flow-studio");
}
