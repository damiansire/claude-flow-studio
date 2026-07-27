//! Tests de contrato sobre la configuración de seguridad DECLARATIVA de Tauri:
//! la capability de filesystem (`capabilities/claude-config-access.json`) y la
//! CSP de producción (`tauri.conf.json`). Estos archivos son parte del
//! perímetro de seguridad tanto como el código de staging, pero al ser JSON
//! estático nadie los compila: sin estos tests, un PR podría ampliar el scope
//! a `$HOME/**` o borrar la CSP y el build seguiría verde.
//!
//! Trazabilidad README → test: sección "El invariante de seguridad".

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn manifest_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read_json(relative: &str) -> Value {
    let path = manifest_path(relative);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("JSON inválido en {}: {e}", path.display()))
}

/// Garantía del README: el ÚNICO camino de escritura a `~/.claude` es
/// `stage_change` → `apply_staged`.
///
/// Antes este test exigía `fs_scoped >= 2`, o sea fijaba en su lugar la
/// concesión de `fs:allow-write-file` sobre `$HOME/.claude/**`: un permiso que
/// habilita `invoke("plugin:fs|write_file", ...)` desde la webview y saltea
/// staging, diff, backup e historial. El contrato correcto es el opuesto:
/// NINGÚN permiso `fs:*`, scoped o no. Los comandos propios hacen `std::fs` y
/// se acotan con `ensure_within_claude_dir`, no con la capability.
#[test]
fn fs_capability_grants_no_filesystem_permission_at_all() {
    let capability = read_json("capabilities/claude-config-access.json");
    let permissions = capability["permissions"]
        .as_array()
        .expect("la capability debe tener un array `permissions`");

    for permission in permissions {
        let identifier = match permission {
            Value::String(identifier) => identifier.as_str(),
            Value::Object(o) => o["identifier"]
                .as_str()
                .expect("permiso objeto sin `identifier`"),
            other => panic!("forma de permiso inesperada en la capability: {other}"),
        };
        assert!(
            !identifier.starts_with("fs:"),
            "la capability concede `{identifier}`: cualquier permiso fs le abre a la webview un canal de escritura/lectura que no pasa por el staging"
        );
    }

    // La capability tiene que estar realmente cableada en tauri.conf.json:
    // un archivo de capability huérfano no protege nada.
    let conf = read_json("tauri.conf.json");
    let wired = conf["app"]["security"]["capabilities"]
        .as_array()
        .expect("tauri.conf.json debe listar capabilities explícitas")
        .iter()
        .any(|c| c == "claude-config-access");
    assert!(
        wired,
        "claude-config-access no está referenciada en app.security.capabilities"
    );
}

/// La otra mitad del mismo invariante: un permiso solo habilita comandos si el
/// plugin está registrado, y un plugin registrado puede recuperar permisos por
/// otra capability. Se gatea el registro en `lib.rs` además de la capability,
/// para que reintroducir cualquiera de las dos mitades rompa CI.
#[test]
fn the_app_does_not_register_the_filesystem_plugin() {
    let lib_rs =
        fs::read_to_string(manifest_path("src/lib.rs")).expect("no se pudo leer src/lib.rs");
    let code: String = lib_rs
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !code.contains("tauri_plugin_fs"),
        "src/lib.rs registra tauri_plugin_fs: eso expone `plugin:fs|write_file` a la webview y saltea el staging"
    );

    let cargo_toml =
        fs::read_to_string(manifest_path("Cargo.toml")).expect("no se pudo leer Cargo.toml");
    assert!(
        !cargo_toml.contains("tauri-plugin-fs"),
        "Cargo.toml sigue declarando tauri-plugin-fs: dependencia sin consumidor y superficie IPC latente"
    );
}

/// Garantía del README: "CSP restrictiva en producción". La CSP de `app.security.csp`
/// (la que rige el bundle real; `devCsp` es aparte y más laxa a propósito) debe
/// existir, restringir scripts a `'self'` sin `unsafe-inline`/`unsafe-eval`, y
/// cerrar los vectores clásicos de embedding (`object-src`, `frame-ancestors`,
/// `base-uri`).
#[test]
fn production_csp_is_restrictive() {
    let conf = read_json("tauri.conf.json");
    let csp = conf["app"]["security"]["csp"]
        .as_str()
        .expect("app.security.csp debe estar definida (null = sin CSP en producción)");

    let directive = |name: &str| -> Vec<String> {
        csp.split(';')
            .map(str::trim)
            .find(|d| d.starts_with(name))
            .unwrap_or_else(|| panic!("la CSP de producción no define `{name}`"))
            .split_whitespace()
            .skip(1)
            .map(str::to_string)
            .collect()
    };

    assert_eq!(
        directive("default-src"),
        vec!["'self'"],
        "default-src debe ser exactamente 'self'"
    );
    assert_eq!(
        directive("script-src"),
        vec!["'self'"],
        "script-src debe ser exactamente 'self': sin unsafe-inline ni unsafe-eval en producción"
    );
    assert_eq!(directive("object-src"), vec!["'none'"]);
    assert_eq!(directive("base-uri"), vec!["'none'"]);
    assert_eq!(directive("frame-ancestors"), vec!["'none'"]);
    assert!(
        !csp.contains("unsafe-eval"),
        "unsafe-eval no puede aparecer en ninguna directiva de la CSP de producción"
    );
}
