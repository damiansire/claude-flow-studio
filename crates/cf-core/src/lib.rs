//! Dominio puro de claude-flow-studio: parseo, escaneo y staging de la
//! configuración de Claude Code (~/.claude). Sin dependencias de Tauri ni IPC.

pub mod frontmatter;
pub mod model;
pub mod scan;
pub mod staging;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
