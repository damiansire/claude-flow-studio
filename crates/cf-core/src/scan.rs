//! Escaneo de `~/.claude`: enumera memorias, skills, comandos y workflows, y
//! lee un resumen de `settings.json`. Solo lectura — el staging vive en
//! [`crate::staging`].

use std::path::{Path, PathBuf};

use crate::frontmatter;
use crate::model::{AgentDef, MemoryEntry, SettingsSummary, Skill, SlashCommand, Workflow};

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("no se pudo leer {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("no se pudo parsear {path} como JSON: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
}

fn read_to_string(path: &Path) -> Result<String, ScanError> {
    std::fs::read_to_string(path).map_err(|source| ScanError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn stem_or(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(fallback)
        .to_string()
}

/// Sanitiza un path absoluto al mismo slug que usa Claude Code para nombrar
/// `projects/<slug>/`: cada separador de carpeta y `:` se reemplaza por `-`.
/// Ej. `C:\Users\tester` → `C--Users-tester`.
pub fn path_slug(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| {
            if c == '\\' || c == '/' || c == ':' {
                '-'
            } else {
                c
            }
        })
        .collect()
}

/// Memorias del "chat global" (`projects/<slug-del-home>/memory/`) — la sesión
/// que corre con el home dir como cwd, que es lo que hoy usa este flujo. Otros
/// proyectos tienen su propia carpeta de memoria bajo `projects/<slug>/memory/`,
/// fuera de alcance de esta función.
pub fn list_memories(claude_dir: &Path) -> Result<Vec<MemoryEntry>, ScanError> {
    let home_dir = claude_dir.parent().unwrap_or(claude_dir);
    let dir = claude_dir
        .join("projects")
        .join(path_slug(home_dir))
        .join("memory");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|source| ScanError::Io {
        path: dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| ScanError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if path.file_name().and_then(|f| f.to_str()) == Some("MEMORY.md") {
            continue; // el índice, no una memoria en sí
        }
        let doc = frontmatter::parse(&read_to_string(&path)?);
        out.push(MemoryEntry {
            name: doc
                .field("name")
                .unwrap_or_else(|| stem_or(&path, "memoria")),
            description: doc.field("description").unwrap_or_default(),
            mem_type: doc.field_path(&["metadata", "type"]),
            path,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Skills: cada una es una carpeta `skills/<nombre>/SKILL.md`.
pub fn list_skills(claude_dir: &Path) -> Result<Vec<Skill>, ScanError> {
    scan_named_subfolders(&claude_dir.join("skills"), "skill")
}

/// Tareas programadas (`scheduled-tasks/<nombre>/SKILL.md`) — misma forma que
/// una skill, así que reusa el mismo escaneo.
pub fn list_scheduled_tasks(claude_dir: &Path) -> Result<Vec<Skill>, ScanError> {
    scan_named_subfolders(&claude_dir.join("scheduled-tasks"), "tarea")
}

/// Escanea `dir/<nombre>/SKILL.md` para cada subcarpeta — el patrón que
/// comparten skills y tareas programadas.
fn scan_named_subfolders(dir: &Path, fallback: &str) -> Result<Vec<Skill>, ScanError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|source| ScanError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ScanError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let subfolder = entry.path();
        if !subfolder.is_dir() {
            continue;
        }
        let skill_md = subfolder.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let doc = frontmatter::parse(&read_to_string(&skill_md)?);
        out.push(Skill {
            name: doc
                .field("name")
                .unwrap_or_else(|| stem_or(&subfolder, fallback)),
            description: doc.field("description").unwrap_or_default(),
            path: skill_md,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Agentes: archivos markdown sueltos en `agents/`.
pub fn list_agents(claude_dir: &Path) -> Result<Vec<AgentDef>, ScanError> {
    let dir = claude_dir.join("agents");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|source| ScanError::Io {
        path: dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| ScanError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let doc = frontmatter::parse(&read_to_string(&path)?);
        out.push(AgentDef {
            name: doc
                .field("name")
                .unwrap_or_else(|| stem_or(&path, "agente")),
            description: doc.field("description").unwrap_or_default(),
            tools: doc.field("tools"),
            path,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Slash commands: archivos markdown sueltos en `commands/`.
pub fn list_commands(claude_dir: &Path) -> Result<Vec<SlashCommand>, ScanError> {
    let dir = claude_dir.join("commands");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|source| ScanError::Io {
        path: dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| ScanError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let doc = frontmatter::parse(&read_to_string(&path)?);
        out.push(SlashCommand {
            name: stem_or(&path, "comando"),
            description: doc.field("description"),
            argument_hint: doc.field("argument-hint"),
            path,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Workflows: scripts `.js` sueltos en `workflows/`. No son YAML frontmatter
/// sino un `export const meta = {...}` de JS — se extraen los campos con un
/// parsing best-effort de strings, no un parser de JS real.
pub fn list_workflows(claude_dir: &Path) -> Result<Vec<Workflow>, ScanError> {
    let dir = claude_dir.join("workflows");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|source| ScanError::Io {
        path: dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| ScanError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("js") {
            continue;
        }
        let src = read_to_string(&path)?;
        out.push(Workflow {
            name: extract_js_string_field(&src, "name")
                .unwrap_or_else(|| stem_or(&path, "workflow")),
            description: extract_js_string_field(&src, "description"),
            when_to_use: extract_js_string_field(&src, "whenToUse"),
            path,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// El `CLAUDE.md` global (reglas de todos los proyectos). Solo lectura: no
/// forma parte de los 4 tipos de contenido editables de esta app.
pub fn read_claude_md(claude_dir: &Path) -> Result<String, ScanError> {
    read_to_string(&claude_dir.join("CLAUDE.md"))
}

/// Busca `key: '...'` o `key: "..."` en un source JS y devuelve el contenido
/// del string. No entiende escapes complejos ni comentarios — alcanza para los
/// bloques `meta = {...}` que escriben los workflows de este flujo.
fn extract_js_string_field(src: &str, key: &str) -> Option<String> {
    let pattern = format!("{key}:");
    let idx = src.find(&pattern)?;
    let after = src[idx + pattern.len()..].trim_start();
    let quote = after.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let rest = &after[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

pub fn read_settings_raw(claude_dir: &Path) -> Result<String, ScanError> {
    read_to_string(&claude_dir.join("settings.json"))
}

pub fn read_settings_summary(claude_dir: &Path) -> Result<SettingsSummary, ScanError> {
    let path = claude_dir.join("settings.json");
    if !path.is_file() {
        return Ok(SettingsSummary::default());
    }
    let raw = read_to_string(&path)?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|source| ScanError::Json {
            path: path.clone(),
            source,
        })?;

    let permissions_allow = value
        .pointer("/permissions/allow")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    let hooks_events = value
        .get("hooks")
        .and_then(|v| v.as_object())
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default();

    let enabled_plugins = value
        .get("enabledPlugins")
        .and_then(|v| v.as_object())
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default();

    Ok(SettingsSummary {
        model: value
            .get("model")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        theme: value
            .get("theme")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        permissions_allow,
        hooks_events,
        enabled_plugins,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_single_and_double_quoted_fields() {
        let src = r#"export const meta = { name: 'world-class-audit', description: "Auditoría multi-agente" }"#;
        assert_eq!(
            extract_js_string_field(src, "name").as_deref(),
            Some("world-class-audit")
        );
        assert_eq!(
            extract_js_string_field(src, "description").as_deref(),
            Some("Auditoría multi-agente")
        );
    }

    #[test]
    fn missing_field_returns_none() {
        let src = "export const meta = { name: 'x' }";
        assert_eq!(extract_js_string_field(src, "whenToUse"), None);
    }

    #[test]
    fn path_slug_replaces_separators_and_colon() {
        assert_eq!(path_slug(Path::new("C:\\Users\\tester")), "C--Users-tester");
        assert_eq!(
            path_slug(Path::new(
                "C:\\Users\\tester\\Documents\\claude-flow-studio"
            )),
            "C--Users-tester-Documents-claude-flow-studio"
        );
    }

    #[test]
    fn path_slug_replaces_unix_separators_too() {
        assert_eq!(path_slug(Path::new("/home/damian")), "-home-damian");
    }
}
