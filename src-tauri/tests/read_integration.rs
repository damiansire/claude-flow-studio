//! Tests de integración de `read_file_content` con I/O real.
//!
//! Es el único comando que recibe un path arbitrario del IPC, y hasta ahora era
//! también el único sin test ni el corte `_impl` que el resto de la capa ya
//! usaba: el guardrail que lo protege (`ensure_within_claude_dir`) solo estaba
//! ejercitado indirectamente, desde los tests de staging. Acá se ejercita el
//! camino real del comando (validación + lectura) contra un `~/.claude`
//! temporal, sin `AppHandle`.

use std::fs;
use std::path::PathBuf;

use claude_flow_studio_lib::commands::read_file_content_impl;
use claude_flow_studio_lib::error::AppError;
use tauri::async_runtime::block_on;

struct Fixture {
    _root: tempfile::TempDir,
    root: PathBuf,
    claude_dir: PathBuf,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let claude_dir = root.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    Fixture {
        root: root.path().to_path_buf(),
        claude_dir,
        _root: root,
    }
}

#[test]
fn reads_a_file_inside_the_claude_dir() {
    let f = fixture();
    let skill_dir = f.claude_dir.join("skills").join("una");
    fs::create_dir_all(&skill_dir).unwrap();
    let target = skill_dir.join("SKILL.md");
    fs::write(&target, "contenido de la skill\n").unwrap();

    let content = block_on(read_file_content_impl(
        f.claude_dir.clone(),
        target.display().to_string(),
    ))
    .unwrap();

    assert_eq!(content, "contenido de la skill\n");
}

#[test]
fn rejects_a_relative_dotdot_escape_without_reading_anything() {
    let f = fixture();
    let secreto = f.root.join("secreto.md");
    fs::write(&secreto, "no se puede leer desde la app\n").unwrap();

    let escapado = f.claude_dir.join("..").join("secreto.md");
    let err = block_on(read_file_content_impl(
        f.claude_dir.clone(),
        escapado.display().to_string(),
    ))
    .unwrap_err();

    assert!(
        matches!(err, AppError::Path(_)),
        "esperaba rechazo por boundary, vino {err:?}"
    );
}

#[test]
fn rejects_an_absolute_path_outside_the_claude_dir() {
    let f = fixture();
    let afuera = f.root.join("afuera.md");
    fs::write(&afuera, "tampoco\n").unwrap();

    let err = block_on(read_file_content_impl(
        f.claude_dir.clone(),
        afuera.display().to_string(),
    ))
    .unwrap_err();

    assert!(
        matches!(err, AppError::Path(_)),
        "esperaba rechazo por boundary, vino {err:?}"
    );
}

/// El caso que motivó el guardrail de `..` detrás de un componente inexistente:
/// el path no existe entero, así que el chequeo tiene que rechazar en vez de
/// retroceder hasta un ancestro que sí esté dentro del boundary.
#[test]
fn rejects_a_dotdot_behind_a_directory_that_does_not_exist() {
    let f = fixture();
    let escapado = f
        .claude_dir
        .join("todavia-no-existe")
        .join("..")
        .join("..")
        .join("secreto.md");

    let err = block_on(read_file_content_impl(
        f.claude_dir.clone(),
        escapado.display().to_string(),
    ))
    .unwrap_err();

    assert!(
        matches!(err, AppError::Path(_)),
        "esperaba rechazo por boundary, vino {err:?}"
    );
}

/// Un archivo borrado entre el listado y el click no puede tumbar el modal con
/// un panic: tiene que volver como error tipado de I/O.
#[test]
fn a_file_deleted_between_listing_and_opening_returns_a_typed_io_error() {
    let f = fixture();
    let target = f.claude_dir.join("se-borro.md");

    let err = block_on(read_file_content_impl(
        f.claude_dir.clone(),
        target.display().to_string(),
    ))
    .unwrap_err();

    assert!(
        matches!(err, AppError::Io { .. }),
        "esperaba AppError::Io, vino {err:?}"
    );
}
