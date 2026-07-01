//! Modelos de solo-lectura usados para listar el contenido de `~/.claude`.
//! El contenido completo (para editar) es el `String` crudo del archivo — estos
//! structs son para mostrar tarjetas/listas, no para reconstruir el archivo.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MemoryEntry {
    pub name: String,
    pub description: String,
    pub mem_type: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// Ruta al `SKILL.md`, no a la carpeta contenedora.
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SlashCommand {
    pub name: String,
    pub description: Option<String>,
    pub argument_hint: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AgentDef {
    pub name: String,
    pub description: String,
    /// Lista de herramientas tal cual está en el frontmatter (texto libre, ej.
    /// `"Bash, Read, Grep, Glob"`), sin parsear a una lista tipada.
    pub tools: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Workflow {
    pub name: String,
    pub description: Option<String>,
    pub when_to_use: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Default)]
pub struct SettingsSummary {
    pub model: Option<String>,
    pub theme: Option<String>,
    pub permissions_allow: Vec<String>,
    pub hooks_events: Vec<String>,
    pub enabled_plugins: Vec<String>,
}
