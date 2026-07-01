//! Motor de staging genérico: nunca escribe directo a un archivo real. Toda
//! edición pasa por un borrador (guardado fuera de `~/.claude`, en el
//! directorio de datos propio de la app) que se puede diffear contra el
//! archivo real, y luego aplicar (con backup) o descartar.
//!
//! Genérico a propósito: memorias, skills, comandos, workflows y settings.json
//! son todos, para este motor, solo un `(target_path, contenido: String)`.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;

#[derive(Debug, thiserror::Error)]
pub enum StagingError {
    #[error("no se pudo leer/escribir {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("no se pudo (de)serializar: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("no existe el borrador/cambio con id {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StagedChange {
    pub id: String,
    pub target_path: PathBuf,
    pub draft_content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppliedChange {
    pub id: String,
    pub target_path: PathBuf,
    pub backup_path: PathBuf,
    pub applied_at: String,
}

pub struct StagingStore {
    staging_dir: PathBuf,
    backups_dir: PathBuf,
    history_path: PathBuf,
}

impl StagingStore {
    /// `app_data_dir` es el directorio de datos propio de la app (NO `~/.claude`)
    /// — típicamente lo resuelve Tauri (`app.path().app_data_dir()`).
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            staging_dir: app_data_dir.join("staging"),
            backups_dir: app_data_dir.join("backups"),
            history_path: app_data_dir.join("history.jsonl"),
        }
    }

    fn ensure_dirs(&self) -> Result<(), StagingError> {
        fs::create_dir_all(&self.staging_dir).map_err(|source| StagingError::Io {
            path: self.staging_dir.clone(),
            source,
        })?;
        fs::create_dir_all(&self.backups_dir).map_err(|source| StagingError::Io {
            path: self.backups_dir.clone(),
            source,
        })
    }

    fn change_path(&self, id: &str) -> PathBuf {
        self.staging_dir.join(format!("{id}.json"))
    }

    /// Guarda un borrador. No toca `target_path` para nada todavía.
    pub fn stage(
        &self,
        target_path: &Path,
        draft_content: String,
    ) -> Result<StagedChange, StagingError> {
        self.ensure_dirs()?;
        let change = StagedChange {
            id: make_id(target_path),
            target_path: target_path.to_path_buf(),
            draft_content,
            created_at: Utc::now().to_rfc3339(),
        };
        let path = self.change_path(&change.id);
        fs::write(&path, serde_json::to_string_pretty(&change)?)
            .map_err(|source| StagingError::Io { path, source })?;
        Ok(change)
    }

    pub fn list(&self) -> Result<Vec<StagedChange>, StagingError> {
        if !self.staging_dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.staging_dir).map_err(|source| StagingError::Io {
            path: self.staging_dir.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| StagingError::Io {
                path: self.staging_dir.clone(),
                source,
            })?;
            let raw = fs::read_to_string(entry.path()).map_err(|source| StagingError::Io {
                path: entry.path(),
                source,
            })?;
            out.push(serde_json::from_str(&raw)?);
        }
        out.sort_by(|a: &StagedChange, b: &StagedChange| a.created_at.cmp(&b.created_at));
        Ok(out)
    }

    pub fn get(&self, id: &str) -> Result<StagedChange, StagingError> {
        let raw = fs::read_to_string(self.change_path(id))
            .map_err(|_| StagingError::NotFound(id.to_string()))?;
        Ok(serde_json::from_str(&raw)?)
    }

    /// Diff unificado entre lo que hay HOY en el archivo real y el borrador —
    /// el archivo real puede haber cambiado desde que se creó el borrador.
    pub fn diff(&self, id: &str) -> Result<String, StagingError> {
        let change = self.get(id)?;
        let current = fs::read_to_string(&change.target_path).unwrap_or_default();
        Ok(unified_diff(&current, &change.draft_content))
    }

    pub fn discard(&self, id: &str) -> Result<(), StagingError> {
        let path = self.change_path(id);
        if path.exists() {
            fs::remove_file(&path).map_err(|source| StagingError::Io { path, source })?;
        }
        Ok(())
    }

    /// Aplica el borrador: backup del archivo real (si existía) -> escribe el
    /// contenido nuevo -> historial -> borra el borrador.
    pub fn apply(&self, id: &str) -> Result<AppliedChange, StagingError> {
        self.ensure_dirs()?;
        let change = self.get(id)?;
        let backup_path = self.backup(&change.target_path)?;
        if let Some(parent) = change.target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| StagingError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&change.target_path, &change.draft_content).map_err(|source| {
            StagingError::Io {
                path: change.target_path.clone(),
                source,
            }
        })?;
        let applied = AppliedChange {
            id: change.id.clone(),
            target_path: change.target_path.clone(),
            backup_path,
            applied_at: Utc::now().to_rfc3339(),
        };
        self.append_history(&applied)?;
        self.discard(id)?;
        Ok(applied)
    }

    fn backup(&self, target_path: &Path) -> Result<PathBuf, StagingError> {
        self.ensure_dirs()?;
        let stamp = Utc::now().format("%Y%m%dT%H%M%S%.9f");
        let filename = target_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("archivo");
        let backup_path = self.backups_dir.join(format!("{stamp}-{filename}"));
        if target_path.is_file() {
            fs::copy(target_path, &backup_path).map_err(|source| StagingError::Io {
                path: backup_path.clone(),
                source,
            })?;
        } else {
            // No existía (p.ej. una memoria nueva): backup vacío, para que
            // revertir sepa que "el estado anterior" era "no existía".
            fs::write(&backup_path, "").map_err(|source| StagingError::Io {
                path: backup_path.clone(),
                source,
            })?;
        }
        Ok(backup_path)
    }

    fn append_history(&self, applied: &AppliedChange) -> Result<(), StagingError> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_path)
            .map_err(|source| StagingError::Io {
                path: self.history_path.clone(),
                source,
            })?;
        writeln!(file, "{}", serde_json::to_string(applied)?).map_err(|source| {
            StagingError::Io {
                path: self.history_path.clone(),
                source,
            }
        })?;
        Ok(())
    }

    /// Historial de cambios aplicados, más reciente primero.
    pub fn history(&self) -> Result<Vec<AppliedChange>, StagingError> {
        if !self.history_path.is_file() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.history_path).map_err(|source| StagingError::Io {
            path: self.history_path.clone(),
            source,
        })?;
        let mut out = Vec::new();
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            out.push(serde_json::from_str(line)?);
        }
        out.reverse();
        Ok(out)
    }

    /// Revierte un cambio ya aplicado: copia su backup de vuelta al archivo
    /// real. Registra el estado actual (pre-revert) como un nuevo backup, así
    /// revertir nunca es un callejón sin salida.
    pub fn revert(&self, applied_id: &str) -> Result<AppliedChange, StagingError> {
        let entry = self
            .history()?
            .into_iter()
            .find(|a| a.id == applied_id)
            .ok_or_else(|| StagingError::NotFound(applied_id.to_string()))?;
        let backup_of_current = self.backup(&entry.target_path)?;
        let restored =
            fs::read_to_string(&entry.backup_path).map_err(|source| StagingError::Io {
                path: entry.backup_path.clone(),
                source,
            })?;
        fs::write(&entry.target_path, restored).map_err(|source| StagingError::Io {
            path: entry.target_path.clone(),
            source,
        })?;
        let applied = AppliedChange {
            id: make_id(&entry.target_path),
            target_path: entry.target_path.clone(),
            backup_path: backup_of_current,
            applied_at: Utc::now().to_rfc3339(),
        };
        self.append_history(&applied)?;
        Ok(applied)
    }
}

fn make_id(target_path: &Path) -> String {
    let stamp = Utc::now().format("%Y%m%dT%H%M%S%.9f");
    let filename = target_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("archivo");
    let safe: String = filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{stamp}-{safe}")
}

fn unified_diff(before: &str, after: &str) -> String {
    similar::TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(3)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, tempfile::TempDir, StagingStore) {
        let app_data = tempdir().unwrap();
        let claude_home = tempdir().unwrap();
        let store = StagingStore::new(app_data.path());
        (app_data, claude_home, store)
    }

    #[test]
    fn stage_does_not_touch_the_real_file() {
        let (_app_data, claude_home, store) = setup();
        let target = claude_home.path().join("memory.md");
        fs::write(&target, "original\n").unwrap();

        store
            .stage(&target, "borrador nuevo\n".to_string())
            .unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "original\n");
    }

    #[test]
    fn discard_removes_the_draft_and_leaves_real_file_untouched() {
        let (_app_data, claude_home, store) = setup();
        let target = claude_home.path().join("memory.md");
        fs::write(&target, "original\n").unwrap();

        let change = store.stage(&target, "borrador\n".to_string()).unwrap();
        store.discard(&change.id).unwrap();

        assert!(store.get(&change.id).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "original\n");
    }

    #[test]
    fn apply_writes_real_file_backs_up_original_and_clears_draft() {
        let (_app_data, claude_home, store) = setup();
        let target = claude_home.path().join("memory.md");
        fs::write(&target, "original\n").unwrap();

        let change = store
            .stage(&target, "nuevo contenido\n".to_string())
            .unwrap();
        let applied = store.apply(&change.id).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "nuevo contenido\n");
        assert_eq!(
            fs::read_to_string(&applied.backup_path).unwrap(),
            "original\n"
        );
        assert!(
            store.get(&change.id).is_err(),
            "el borrador debería haberse limpiado"
        );
    }

    #[test]
    fn apply_on_a_new_file_backs_up_as_empty() {
        let (_app_data, claude_home, store) = setup();
        let target = claude_home.path().join("nueva-memoria.md");

        let change = store
            .stage(&target, "contenido nuevo\n".to_string())
            .unwrap();
        let applied = store.apply(&change.id).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "contenido nuevo\n");
        assert_eq!(fs::read_to_string(&applied.backup_path).unwrap(), "");
    }

    #[test]
    fn revert_restores_the_backup_and_logs_a_new_history_entry() {
        let (_app_data, claude_home, store) = setup();
        let target = claude_home.path().join("memory.md");
        fs::write(&target, "v1\n").unwrap();

        let change = store.stage(&target, "v2\n".to_string()).unwrap();
        let applied = store.apply(&change.id).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "v2\n");

        let reverted = store.revert(&applied.id).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "v1\n");

        let history = store.history().unwrap();
        assert_eq!(
            history.len(),
            2,
            "aplicar + revertir son dos entradas de historial"
        );
        assert_eq!(history[0].id, reverted.id, "más reciente primero");
    }

    #[test]
    fn diff_reflects_current_disk_state_not_just_the_draft() {
        let (_app_data, claude_home, store) = setup();
        let target = claude_home.path().join("memory.md");
        fs::write(&target, "linea original\n").unwrap();

        let change = store.stage(&target, "linea nueva\n".to_string()).unwrap();

        // El archivo real cambia por fuera mientras el borrador seguía pendiente.
        fs::write(&target, "linea cambiada por fuera\n").unwrap();

        let diff = store.diff(&change.id).unwrap();
        assert!(diff.contains("linea cambiada por fuera"));
        assert!(diff.contains("linea nueva"));
    }

    #[test]
    fn list_returns_staged_changes_oldest_first() {
        let (_app_data, claude_home, store) = setup();
        let a = claude_home.path().join("a.md");
        let b = claude_home.path().join("b.md");
        fs::write(&a, "a\n").unwrap();
        fs::write(&b, "b\n").unwrap();

        store.stage(&a, "a2\n".to_string()).unwrap();
        store.stage(&b, "b2\n".to_string()).unwrap();

        assert_eq!(store.list().unwrap().len(), 2);
    }
}
