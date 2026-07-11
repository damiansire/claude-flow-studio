//! Tests de integración de la capa de comandos Tauri (`src-tauri/src/commands/staging.rs`)
//! con I/O real: directorios temporales que hacen de `~/.claude` y de
//! `app_data_dir`, sin mockear el filesystem. No usan un `AppHandle` real
//! (requiere una ventana, imposible en CI headless) — llaman directo a las
//! funciones `_impl` que los `#[tauri::command]` delegan, que son el mismo
//! código que corre en producción salvo por la resolución de paths desde el
//! `AppHandle` (dos líneas, sin lógica).
//!
//! cf-core (`crates/cf-core/src/staging.rs`) ya cubre el motor de
//! staging/diff/apply/backup como dominio puro. Lo que este archivo agrega es
//! la otra mitad del contrato: que la capa de comandos que Tauri realmente
//! invoca (con el boundary re-cableado a `claude_dir`, logging y `blocking`)
//! se comporta igual.

use std::fs;
use std::path::PathBuf;

use claude_flow_studio_lib::commands::{
    apply_staged_impl, discard_staged_impl, list_history_impl, list_staged_impl,
    revert_applied_impl, stage_change_impl,
};
use claude_flow_studio_lib::error::AppError;
use tauri::async_runtime::block_on;

/// `~/.claude` temporal + `app_data_dir` temporal, aislados entre tests (cada
/// `tempfile::tempdir()` es una carpeta nueva) y de la máquina real.
struct Fixture {
    _claude_root: tempfile::TempDir,
    _app_data_root: tempfile::TempDir,
    claude_dir: PathBuf,
    app_data_dir: PathBuf,
}

fn fixture() -> Fixture {
    let claude_root = tempfile::tempdir().unwrap();
    let app_data_root = tempfile::tempdir().unwrap();
    let claude_dir = claude_root.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    Fixture {
        claude_dir,
        app_data_dir: app_data_root.path().to_path_buf(),
        _claude_root: claude_root,
        _app_data_root: app_data_root,
    }
}

// --- cfs-1: apply real de punta a punta -------------------------------------

#[test]
fn stage_change_does_not_touch_the_real_file_until_apply() {
    let f = fixture();
    let target = f.claude_dir.join("memory.md");
    fs::write(&target, "original\n").unwrap();

    block_on(stage_change_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        target.display().to_string(),
        "borrador nuevo\n".to_string(),
    ))
    .unwrap();

    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "original\n",
        "stage_change (comando real) no debe escribir el archivo real"
    );
}

#[test]
fn apply_staged_backs_up_before_writing_and_the_backup_predates_the_new_content() {
    let f = fixture();
    let target = f.claude_dir.join("memory.md");
    fs::write(&target, "original\n").unwrap();

    let staged = block_on(stage_change_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        target.display().to_string(),
        "contenido nuevo\n".to_string(),
    ))
    .unwrap();

    let applied = block_on(apply_staged_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        staged.id,
    ))
    .unwrap();

    // El backup existe y tiene el contenido de ANTES del apply...
    assert_eq!(
        fs::read_to_string(&applied.backup_path).unwrap(),
        "original\n",
        "el backup debe capturar el estado previo, no el nuevo"
    );
    // ...y el archivo real quedó con el contenido nuevo: el backup se hizo
    // ANTES de sobreescribir, nunca después (si fuera al revés, sería un
    // backup del contenido ya nuevo, o se perdería el original).
    assert_eq!(fs::read_to_string(&target).unwrap(), "contenido nuevo\n");

    let history = block_on(list_history_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
    ))
    .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].backup_path, applied.backup_path);
}

#[test]
fn apply_staged_on_unknown_id_fails_without_touching_anything() {
    let f = fixture();
    let err = block_on(apply_staged_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        "no-existe".to_string(),
    ))
    .unwrap_err();
    assert!(matches!(
        err,
        AppError::Staging(cf_core::staging::StagingError::NotFound(_))
    ));
}

// --- boundary: escritura fuera de ~/.claude falla, en TODO el camino -------

#[test]
fn stage_change_rejects_a_target_outside_claude_dir() {
    let f = fixture();
    let outside = f.claude_dir.parent().unwrap().join("fuera-de-claude.md");
    fs::write(&outside, "no debería poder pisarse\n").unwrap();

    let err = block_on(stage_change_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        outside.display().to_string(),
        "payload malicioso\n".to_string(),
    ))
    .unwrap_err();

    assert!(matches!(err, AppError::Path(_)));
    // No se creó ningún borrador: list_staged debe seguir vacío.
    let staged = block_on(list_staged_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
    ))
    .unwrap();
    assert!(staged.is_empty());
    assert_eq!(
        fs::read_to_string(&outside).unwrap(),
        "no debería poder pisarse\n"
    );
}

#[test]
fn apply_staged_rejects_a_tampered_draft_pointing_outside_the_boundary() {
    // `ensure_within_claude_dir` en `stage_change_impl` es la primera barrera,
    // pero el boundary se revalida DE NUEVO dentro de `apply` (cf-core) por si
    // el borrador en disco fue adulterado después de crearse. Para probar esa
    // segunda barrera hay que crear el borrador saltándose la primera, igual
    // que hace el test equivalente en crates/cf-core/src/staging.rs.
    let f = fixture();
    let outside = f.claude_dir.parent().unwrap().join("robado.md");

    let store_sin_boundary = cf_core::staging::StagingStore::new(&f.app_data_dir);
    let change = store_sin_boundary
        .stage(&outside, "payload\n".to_string())
        .unwrap();

    let err = block_on(apply_staged_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        change.id,
    ))
    .unwrap_err();

    assert!(matches!(
        err,
        AppError::Staging(cf_core::staging::StagingError::OutsideBoundary { .. })
    ));
    assert!(
        !outside.exists(),
        "apply_staged no debió escribir fuera del boundary"
    );
}

// --- cfs-3: atomicidad / recuperación tras una interrupción -----------------

#[test]
fn a_corrupted_target_after_apply_is_recoverable_via_revert_from_backup() {
    let f = fixture();
    let target = f.claude_dir.join("settings.json");
    fs::write(&target, r#"{"model":"opus"}"#).unwrap();

    let staged = block_on(stage_change_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        target.display().to_string(),
        r#"{"model":"sonnet"}"#.to_string(),
    ))
    .unwrap();
    let applied = block_on(apply_staged_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        staged.id,
    ))
    .unwrap();

    // Simula una interrupción a mitad de una escritura posterior (crash,
    // corte de luz, kill -9): el archivo real queda con contenido a medio
    // escribir / corrupto, distinto de cualquier estado válido conocido.
    fs::write(&target, "{ esto quedo a medio escribir").unwrap();
    assert_ne!(fs::read_to_string(&target).unwrap(), r#"{"model":"opus"}"#);

    // El sistema es recuperable: el backup del apply sigue intacto en disco
    // (nunca se toca a menos que se pida un revert), y revert_applied
    // restaura desde ahí sin importar en qué estado haya quedado el archivo
    // real mientras tanto.
    let reverted = block_on(revert_applied_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        applied.id,
    ))
    .unwrap();

    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        r#"{"model":"opus"}"#,
        "revert debe restaurar el estado pre-apply pase lo que pase con el archivo mientras tanto"
    );
    let history = block_on(list_history_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
    ))
    .unwrap();
    assert_eq!(
        history.len(),
        2,
        "apply + revert son dos entradas; el revert queda registrado, no es un callejón sin salida"
    );
    assert_eq!(
        history[0].id, reverted.id,
        "el revert es la entrada más reciente"
    );
}

// --- cfs-4: edición concurrente externa mientras hay staging pendiente -----

#[test]
fn applying_a_plain_file_overwrites_a_concurrent_external_edit_but_it_stays_recoverable_in_the_backup(
) {
    // Documenta el comportamiento real (no aspiracional) para archivos que NO
    // son settings.json: `apply` escribe el `draft_content` tal cual — no
    // sabe nada de lo que cambió por fuera después del `stage`. Si otro
    // proceso (u otra pestaña de la app, o el usuario a mano) edita el
    // archivo real mientras el borrador está pendiente, esa edición externa
    // se pierde en el archivo aplicado. No se pierde en absoluto, sin
    // embargo: el `apply` hace backup del contenido que estaba en disco EN
    // ESE MOMENTO (la edición externa, no el original de cuando se creó el
    // borrador), así que sigue siendo recuperable con un revert.
    let f = fixture();
    let target = f.claude_dir.join("memory.md");
    fs::write(&target, "original\n").unwrap();

    let staged = block_on(stage_change_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        target.display().to_string(),
        "editado en la app\n".to_string(),
    ))
    .unwrap();

    // Edición externa concurrente: otro proceso (o el usuario editando el
    // archivo a mano) cambia el archivo real mientras el borrador sigue
    // pendiente en la app.
    fs::write(&target, "editado por fuera mientras tanto\n").unwrap();

    let applied = block_on(apply_staged_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        staged.id,
    ))
    .unwrap();

    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "editado en la app\n",
        "el draft gana: la edición externa NO sobrevive en el archivo aplicado"
    );
    assert_eq!(
        fs::read_to_string(&applied.backup_path).unwrap(),
        "editado por fuera mientras tanto\n",
        "pero la edición externa perdida queda recuperable en el backup, no se destruye"
    );
}

#[test]
fn applying_settings_json_preserves_a_concurrent_external_edit_the_draft_did_not_touch() {
    // Para settings.json el motor de staging mergea contra el contenido REAL
    // al momento del apply (no contra una foto de cuando se creó el
    // borrador): una clave que la edición externa concurrente agregó, y que
    // el borrador nunca mencionó, sí sobrevive. Es la mitigación real (no
    // total) al problema de arriba, acotada a JSON.
    let f = fixture();
    let target = f.claude_dir.join("settings.json");
    fs::write(&target, r#"{"model":"opus"}"#).unwrap();

    let staged = block_on(stage_change_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        target.display().to_string(),
        r#"{"model":"sonnet"}"#.to_string(),
    ))
    .unwrap();

    // Edición externa concurrente agrega una clave que el borrador no toca.
    fs::write(&target, r#"{"model":"opus","theme":"agregado-por-fuera"}"#).unwrap();

    block_on(apply_staged_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        staged.id,
    ))
    .unwrap();

    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(
        written["model"], "sonnet",
        "la clave que edita el draft gana"
    );
    assert_eq!(
        written["theme"], "agregado-por-fuera",
        "una clave agregada externamente, que el draft no menciona, sobrevive al merge"
    );
}

// --- discard: no debe tocar nada tampoco -------------------------------------

#[test]
fn discard_staged_leaves_the_real_file_untouched() {
    let f = fixture();
    let target = f.claude_dir.join("memory.md");
    fs::write(&target, "original\n").unwrap();

    let staged = block_on(stage_change_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        target.display().to_string(),
        "borrador\n".to_string(),
    ))
    .unwrap();

    block_on(discard_staged_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
        staged.id,
    ))
    .unwrap();

    assert_eq!(fs::read_to_string(&target).unwrap(), "original\n");
    let staged = block_on(list_staged_impl(
        f.claude_dir.clone(),
        f.app_data_dir.clone(),
    ))
    .unwrap();
    assert!(staged.is_empty());
}
