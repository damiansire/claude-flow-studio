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

/// Garantía del README: "capability de filesystem scoped a `$HOME/.claude/**`
/// (mínimo privilegio)". Cada permiso `fs:*` de la capability debe declarar un
/// allow-list y cada path de ese allow-list debe vivir bajo `$HOME/.claude`.
/// Un permiso `fs:` en forma de string plano (como `fs:default`, que da acceso
/// amplio sin scope) rompe el test.
#[test]
fn fs_capability_scopes_every_permission_to_claude_dir() {
    let capability = read_json("capabilities/claude-config-access.json");
    let permissions = capability["permissions"]
        .as_array()
        .expect("la capability debe tener un array `permissions`");

    let mut fs_scoped = 0;
    for permission in permissions {
        match permission {
            // Permiso sin scope, p.ej. "core:default". Para fs sería un
            // agujero: "fs:default" habilita paths por defecto sin allow-list.
            Value::String(identifier) => {
                assert!(
                    !identifier.starts_with("fs:"),
                    "permiso fs sin scope en la capability: `{identifier}` — todo permiso fs debe llevar allow-list bajo $HOME/.claude"
                );
            }
            Value::Object(o) => {
                let identifier = o["identifier"]
                    .as_str()
                    .expect("permiso objeto sin `identifier`");
                if !identifier.starts_with("fs:") {
                    continue;
                }
                let allow = o["allow"]
                    .as_array()
                    .unwrap_or_else(|| panic!("permiso `{identifier}` sin allow-list"));
                assert!(
                    !allow.is_empty(),
                    "permiso `{identifier}` con allow-list vacío"
                );
                assert!(
                    o.get("deny").is_none(),
                    "permiso `{identifier}` usa deny-list: el contrato es allow-list pura (fail-closed)"
                );
                for entry in allow {
                    let path = entry["path"]
                        .as_str()
                        .unwrap_or_else(|| panic!("entrada de allow sin `path` en `{identifier}`"));
                    assert!(
                        path == "$HOME/.claude" || path.starts_with("$HOME/.claude/"),
                        "permiso `{identifier}` permite `{path}`, fuera de $HOME/.claude"
                    );
                }
                fs_scoped += 1;
            }
            other => panic!("forma de permiso inesperada en la capability: {other}"),
        }
    }

    assert!(
        fs_scoped >= 2,
        "la capability debería declarar al menos lectura y escritura scoped (hay {fs_scoped}); si se reestructuró, actualizar este test junto con el README"
    );

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
