pub mod read;
pub mod staging;

pub use read::*;
pub use staging::*;

use crate::error::AppError;

/// Corre trabajo de I/O bloqueante fuera del hilo principal. Los comandos son
/// `async fn` para que Tauri los despache al runtime; adentro delegan a cf-core
/// (read_dir + read_to_string síncronos) vía `spawn_blocking`, así ni el hilo
/// de la UI ni un worker del runtime quedan bloqueados en disco.
pub(crate) async fn blocking<T, F>(f: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Path(format!("tarea de I/O interrumpida: {e}")))?
}
