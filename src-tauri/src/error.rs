//! `AppError` es el único tipo de error que cruza el límite de IPC. Se
//! serializa como un string plano — el frontend lo recibe como el `catch` de
//! una promesa rechazada, no como un objeto estructurado (no hace falta más
//! para esta app: no hay lógica de UI que distinga entre variantes de error).

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Scan(#[from] cf_core::scan::ScanError),
    #[error(transparent)]
    Staging(#[from] cf_core::staging::StagingError),
    #[error("no se pudo leer/escribir {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("{0}")]
    Path(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
