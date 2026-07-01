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
use serde_json::Value;

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
    #[error("fuera del directorio permitido, rechazado: {target}")]
    OutsideBoundary { target: PathBuf },
}

/// Valida que `target` (exista o no) caiga dentro de `root` una vez resueltos
/// ambos. Canonicaliza en vez de comparar strings: eso neutraliza escapes vía
/// `..` en cualquier posición y symlinks que apunten afuera. Fail-closed: si no
/// se puede resolver `root`, o si `target` cae fuera, devuelve error.
///
/// Es la misma lógica que el guardrail de la capa Tauri, pero acá en el dominio
/// (sin dependencias de Tauri) para poder usarla como defensa en profundidad en
/// TODO camino de escritura de [`StagingStore`], no solo al crear el borrador.
pub fn ensure_within(root: &Path, target: &Path) -> Result<(), StagingError> {
    let canon_root = root.canonicalize().map_err(|source| StagingError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    let mut probe = target.to_path_buf();
    let canon_target = loop {
        match probe.canonicalize() {
            Ok(c) => break c,
            Err(_) if probe.pop() => continue,
            Err(source) => {
                return Err(StagingError::Io {
                    path: target.to_path_buf(),
                    source,
                })
            }
        }
    };
    if canon_target.starts_with(&canon_root) {
        Ok(())
    } else {
        Err(StagingError::OutsideBoundary {
            target: target.to_path_buf(),
        })
    }
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
    /// Si está seteado, TODA escritura a un archivo real (apply/revert) y todo
    /// borrador nuevo (stage) se valida contra este root — defensa en
    /// profundidad, no solo el chequeo de la capa Tauri al crear el borrador.
    boundary: Option<PathBuf>,
}

impl StagingStore {
    /// `app_data_dir` es el directorio de datos propio de la app (NO `~/.claude`)
    /// — típicamente lo resuelve Tauri (`app.path().app_data_dir()`).
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            staging_dir: app_data_dir.join("staging"),
            backups_dir: app_data_dir.join("backups"),
            history_path: app_data_dir.join("history.jsonl"),
            boundary: None,
        }
    }

    /// Restringe todo camino de escritura a archivos dentro de `root`. La capa
    /// Tauri lo cablea con `~/.claude`: así aunque un borrador o el
    /// `history.jsonl` (que viven fuera del scoping de la capability) fueran
    /// adulterados, apply/revert rechazan cualquier `target_path` fuera del
    /// boundary antes de escribir. Sin esto, el guardrail vivía solo en stage.
    pub fn with_boundary(mut self, root: &Path) -> Self {
        self.boundary = Some(root.to_path_buf());
        self
    }

    fn check_boundary(&self, target: &Path) -> Result<(), StagingError> {
        match &self.boundary {
            Some(root) => ensure_within(root, target),
            None => Ok(()),
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

    /// Guarda un borrador. No toca `target_path` para nada todavía. Deduplica
    /// por `target_path`: descarta cualquier borrador previo del mismo archivo
    /// antes de escribir el nuevo, así nunca se acumulan versiones y el editor
    /// no puede reabrir/aplicar una vieja por accidente.
    pub fn stage(
        &self,
        target_path: &Path,
        draft_content: String,
    ) -> Result<StagedChange, StagingError> {
        self.ensure_dirs()?;
        self.check_boundary(target_path)?;
        self.discard_for_target(target_path)?;
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

    /// Borra cualquier borrador pendiente cuyo `target_path` sea `target` — el
    /// paso de deduplicación de [`Self::stage`].
    fn discard_for_target(&self, target: &Path) -> Result<(), StagingError> {
        for existing in self.list()? {
            if existing.target_path == target {
                self.discard(&existing.id)?;
            }
        }
        Ok(())
    }

    /// El contenido EXACTO que `apply` escribiría para este cambio, y contra el
    /// que se calcula el diff — así lo que se revisa es lo que se aplica.
    ///
    /// Para archivos `.json` (settings.json) no se pisa el archivo entero: se
    /// parsea el borrador (fail-closed ante JSON inválido) y se mergea sobre el
    /// contenido real actual, preservando recursivamente las claves que el
    /// borrador no menciona (las que la app quizá ni conoce). El resto de los
    /// archivos se escriben tal cual.
    fn resolved_content(&self, change: &StagedChange) -> Result<String, StagingError> {
        if is_json_target(&change.target_path) {
            let draft: Value = serde_json::from_str(&change.draft_content)?;
            let current = read_json_or_empty(&change.target_path)?;
            let merged = merge_json(current, draft);
            Ok(format!("{}\n", serde_json::to_string_pretty(&merged)?))
        } else {
            Ok(change.draft_content.clone())
        }
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
        Ok(unified_diff(&current, &self.resolved_content(&change)?))
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
        // Defensa en profundidad: el borrador vive fuera del scoping de la
        // capability, así que se revalida el boundary ANTES de escribir — y se
        // resuelve el contenido (fail-closed ante settings.json malformado)
        // antes de tocar backup o archivo real.
        self.check_boundary(&change.target_path)?;
        let content = self.resolved_content(&change)?;
        let backup_path = self.backup(&change.target_path)?;
        if let Some(parent) = change.target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| StagingError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&change.target_path, &content).map_err(|source| StagingError::Io {
            path: change.target_path.clone(),
            source,
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
        // Mismo motivo que apply: el history.jsonl vive fuera del scoping, así
        // que revalidamos el boundary antes de escribir el archivo real.
        self.check_boundary(&entry.target_path)?;
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

fn is_json_target(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("json")
}

/// Lee `path` como JSON. Si no existe o está vacío, devuelve un objeto vacío
/// (base neutra para el merge). Si existe pero es JSON inválido, propaga el
/// error — no queremos mergear a ciegas sobre un archivo corrupto.
fn read_json_or_empty(path: &Path) -> Result<Value, StagingError> {
    match fs::read_to_string(path) {
        Ok(raw) if raw.trim().is_empty() => Ok(Value::Object(Default::default())),
        Ok(raw) => Ok(serde_json::from_str(&raw)?),
        Err(_) => Ok(Value::Object(Default::default())),
    }
}

/// Mergea `overlay` sobre `base`: los objetos se combinan clave por clave
/// recursivamente (así sobrevive una clave anidada que el overlay no menciona),
/// y para escalares/arrays gana `overlay`. Es lo que hace que editar
/// settings.json nunca pierda campos que el borrador no tocó.
fn merge_json(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Object(mut base_map), Value::Object(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                let merged = match base_map.remove(&key) {
                    Some(base_val) => merge_json(base_val, overlay_val),
                    None => overlay_val,
                };
                base_map.insert(key, merged);
            }
            Value::Object(base_map)
        }
        (_, overlay) => overlay,
    }
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

        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 2);
        // El test viejo solo miraba len() y no probaba el orden que su nombre
        // promete — ahora sí verifica created_at ascendente.
        assert!(
            listed[0].created_at <= listed[1].created_at,
            "list() debe devolver los borradores más viejos primero"
        );
    }

    #[test]
    fn stage_twice_on_same_file_keeps_only_the_newest_draft() {
        let (_app_data, claude_home, store) = setup();
        let target = claude_home.path().join("memory.md");
        fs::write(&target, "original\n").unwrap();

        store
            .stage(&target, "borrador viejo\n".to_string())
            .unwrap();
        let nuevo = store
            .stage(&target, "borrador nuevo\n".to_string())
            .unwrap();

        let listed = store.list().unwrap();
        assert_eq!(
            listed.len(),
            1,
            "no deben acumularse borradores del mismo archivo"
        );
        assert_eq!(listed[0].id, nuevo.id);
        assert_eq!(listed[0].draft_content, "borrador nuevo\n");
    }

    #[test]
    fn apply_rejects_a_target_outside_the_boundary() {
        let app_data = tempdir().unwrap();
        let root = tempdir().unwrap();
        let claude = root.path().join(".claude");
        fs::create_dir_all(&claude).unwrap();
        let outside = root.path().join("otro-lado.md");

        // Store SIN boundary para poder crear un borrador adulterado apuntando
        // afuera (simula un draft manipulado en app_data_dir).
        let sin_guard = StagingStore::new(app_data.path());
        let change = sin_guard.stage(&outside, "payload\n".to_string()).unwrap();

        // El store real SÍ tiene boundary: apply debe rechazar y no escribir.
        let store = StagingStore::new(app_data.path()).with_boundary(&claude);
        let err = store.apply(&change.id).unwrap_err();
        assert!(matches!(err, StagingError::OutsideBoundary { .. }));
        assert!(
            !outside.exists(),
            "no debió escribir el archivo fuera del boundary"
        );
    }

    #[test]
    fn revert_rejects_a_target_outside_the_boundary() {
        let app_data = tempdir().unwrap();
        let root = tempdir().unwrap();
        let claude = root.path().join(".claude");
        fs::create_dir_all(&claude).unwrap();
        let inside = claude.join("memory.md");
        fs::write(&inside, "v1\n").unwrap();

        let store = StagingStore::new(app_data.path()).with_boundary(&claude);
        let change = store.stage(&inside, "v2\n".to_string()).unwrap();
        let applied = store.apply(&change.id).unwrap();

        // Adulteramos el target de la entrada de historial a un path externo y
        // reconstruimos el store: revert debe rechazar por boundary.
        let outside = root.path().join("robado.md");
        let history_raw = fs::read_to_string(app_data.path().join("history.jsonl")).unwrap();
        let tampered = history_raw.replace(
            inside.to_string_lossy().replace('\\', "\\\\").as_str(),
            outside.to_string_lossy().replace('\\', "\\\\").as_str(),
        );
        fs::write(app_data.path().join("history.jsonl"), tampered).unwrap();

        let store = StagingStore::new(app_data.path()).with_boundary(&claude);
        let err = store.revert(&applied.id).unwrap_err();
        assert!(matches!(err, StagingError::OutsideBoundary { .. }));
        assert!(
            !outside.exists(),
            "revert no debió escribir fuera del boundary"
        );
    }

    #[test]
    fn applying_settings_json_preserves_keys_the_draft_did_not_mention() {
        let (_app_data, claude_home, store) = setup();
        let target = claude_home.path().join("settings.json");
        fs::write(
            &target,
            r#"{"model":"opus","theme":"dark","permissions":{"allow":["a"],"deny":["x"]}}"#,
        )
        .unwrap();

        // El borrador solo cambia el modelo y menciona permissions.allow — NO
        // menciona theme ni permissions.deny.
        let draft = r#"{"model":"sonnet","permissions":{"allow":["a","b"]}}"#;
        let change = store.stage(&target, draft.to_string()).unwrap();
        store.apply(&change.id).unwrap();

        let written: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(written["model"], "sonnet", "la clave editada gana");
        assert_eq!(
            written["theme"], "dark",
            "una clave no mencionada sobrevive"
        );
        assert_eq!(
            written["permissions"]["deny"],
            serde_json::json!(["x"]),
            "una clave anidada no mencionada sobrevive"
        );
        assert_eq!(
            written["permissions"]["allow"],
            serde_json::json!(["a", "b"]),
            "el array editado gana entero"
        );
    }

    #[test]
    fn applying_malformed_settings_json_fails_closed_and_leaves_the_file_intact() {
        let (_app_data, claude_home, store) = setup();
        let target = claude_home.path().join("settings.json");
        fs::write(&target, r#"{"model":"opus"}"#).unwrap();

        let change = store
            .stage(&target, "{ esto no es json ".to_string())
            .unwrap();
        let err = store.apply(&change.id).unwrap_err();
        assert!(matches!(err, StagingError::Serde(_)));
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            r#"{"model":"opus"}"#,
            "un borrador de JSON inválido no debe pisar el settings.json real"
        );
    }

    #[test]
    fn diff_of_settings_json_matches_the_merged_content_that_apply_writes() {
        let (_app_data, claude_home, store) = setup();
        let target = claude_home.path().join("settings.json");
        fs::write(
            &target,
            "{\n  \"model\": \"opus\",\n  \"theme\": \"dark\"\n}\n",
        )
        .unwrap();

        let change = store
            .stage(&target, r#"{"model":"sonnet"}"#.to_string())
            .unwrap();
        let diff = store.diff(&change.id).unwrap();
        // theme no aparece como eliminado en el diff porque el merge lo preserva
        // — el diff refleja exactamente lo que apply va a escribir.
        assert!(
            !diff.contains("-  \"theme\""),
            "theme no debería figurar como borrado"
        );
        assert!(diff.contains("sonnet"));

        store.apply(&change.id).unwrap();
        let written: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(written["theme"], "dark");
    }

    #[test]
    fn ensure_within_rejects_escapes_and_accepts_inside() {
        let root = tempdir().unwrap();
        let claude = root.path().join(".claude");
        fs::create_dir_all(&claude).unwrap();

        assert!(ensure_within(&claude, &claude.join("skills").join("s").join("SKILL.md")).is_ok());
        assert!(matches!(
            ensure_within(&claude, &root.path().join("afuera.md")),
            Err(StagingError::OutsideBoundary { .. })
        ));
    }
}
