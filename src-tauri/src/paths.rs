//! Resolución de paths + el guardrail de seguridad real de la app: nuestros
//! comandos hacen IO directo (no pasan por `tauri-plugin-fs`), así que las
//! capabilities de la Fase 1 no alcanzan solas para acotar el acceso — este
//! módulo es el que de verdad impide tocar algo fuera de `~/.claude`.

use std::path::{Path, PathBuf};

use tauri::Manager;

use crate::error::AppError;

pub fn claude_dir(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| AppError::Path(format!("no se pudo resolver HOME: {e}")))?;
    Ok(home.join(".claude"))
}

/// Directorio de datos PROPIO de la app (nunca `~/.claude`) — ahí vive el
/// motor de staging: borradores, backups e historial.
pub fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Path(format!("no se pudo resolver app_data_dir: {e}")))
}

/// Falla si `target` (existente o no) queda fuera de `claude_dir` una vez
/// resuelto. Resuelve por `canonicalize()` en vez de comparar strings: eso
/// también neutraliza intentos de escape vía `..` en cualquier posición del
/// path, no solo al final.
pub fn ensure_within_claude_dir(claude_dir: &Path, target: &Path) -> Result<(), AppError> {
    let canon_root = claude_dir.canonicalize().map_err(|source| AppError::Io {
        path: claude_dir.to_path_buf(),
        source,
    })?;

    let mut probe = target.to_path_buf();
    let canon_target = loop {
        match probe.canonicalize() {
            Ok(c) => break c,
            Err(_) if probe.pop() => continue,
            Err(source) => {
                return Err(AppError::Io {
                    path: target.to_path_buf(),
                    source,
                })
            }
        }
    };

    if canon_target.starts_with(&canon_root) {
        Ok(())
    } else {
        Err(AppError::Path(format!(
            "fuera de ~/.claude, rechazado: {}",
            target.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn accepts_a_path_inside_claude_dir_even_if_it_does_not_exist_yet() {
        let root = tempfile::tempdir().unwrap();
        let claude = root.path().join(".claude");
        fs::create_dir_all(&claude).unwrap();

        let target = claude.join("skills").join("nueva").join("SKILL.md");
        assert!(ensure_within_claude_dir(&claude, &target).is_ok());
    }

    #[test]
    fn rejects_a_path_outside_claude_dir() {
        let root = tempfile::tempdir().unwrap();
        let claude = root.path().join(".claude");
        let outside = root.path().join("otro-lado");
        fs::create_dir_all(&claude).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let target = outside.join("archivo.md");
        assert!(ensure_within_claude_dir(&claude, &target).is_err());
    }

    #[test]
    fn rejects_a_dotdot_escape_even_though_the_string_starts_with_claude_dir() {
        let root = tempfile::tempdir().unwrap();
        let claude = root.path().join(".claude");
        let outside = root.path().join("otro-lado");
        fs::create_dir_all(&claude).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let target = claude
            .join("skills")
            .join("..")
            .join("..")
            .join("otro-lado")
            .join("archivo.md");
        assert!(ensure_within_claude_dir(&claude, &target).is_err());
    }
}
