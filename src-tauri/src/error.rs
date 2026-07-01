//! `AppError` es el único tipo de error que cruza el límite de IPC. Se
//! serializa como un objeto estructurado `{ kind, message, path? }` — así el
//! frontend puede distinguir un `outside_boundary` de un `invalid_json` o un
//! `io` y reaccionar distinto, en vez de parsear un string opaco.

use cf_core::staging::StagingError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Scan(#[from] cf_core::scan::ScanError),
    #[error(transparent)]
    Staging(#[from] StagingError),
    #[error("no se pudo leer/escribir {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("{0}")]
    Path(String),
}

impl AppError {
    /// Discriminante estable para el frontend + el path afectado cuando aplica.
    fn kind_and_path(&self) -> (&'static str, Option<String>) {
        let display = |p: &std::path::Path| p.display().to_string();
        match self {
            AppError::Scan(_) => ("scan", None),
            AppError::Staging(StagingError::OutsideBoundary { target }) => {
                ("outside_boundary", Some(display(target)))
            }
            AppError::Staging(StagingError::NotFound(_)) => ("not_found", None),
            AppError::Staging(StagingError::Serde(_)) => ("invalid_json", None),
            AppError::Staging(StagingError::Io { path, .. }) => ("io", Some(display(path))),
            AppError::Io { path, .. } => ("io", Some(display(path))),
            AppError::Path(_) => ("path", None),
        }
    }
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let (kind, path) = self.kind_and_path();
        let mut st = serializer.serialize_struct("AppError", 3)?;
        st.serialize_field("kind", kind)?;
        st.serialize_field("message", &self.to_string())?;
        st.serialize_field("path", &path)?;
        st.end()
    }
}
