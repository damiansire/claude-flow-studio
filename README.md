# claude-flow-studio

[![CI](https://github.com/damiansire/claude-flow-studio/actions/workflows/ci.yml/badge.svg)](https://github.com/damiansire/claude-flow-studio/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

App de escritorio (Tauri v2 + Rust) para **ver y editar tu configuración de
Claude Code** (`~/.claude`): memoria, skills, comandos, workflows, agentes y
`settings.json`. Todo cambio pasa por **staging con revisión** — se guarda un
borrador, se revisa el diff, y recién ahí se aplica (con backup). Nunca se
escribe directo a tus archivos reales.

## Arquitectura

```
┌─────────────────────────┐        invoke()        ┌──────────────────────────┐
│  frontend (Vite + TS)   │ ──────────────────────▶ │   src-tauri (comandos)   │
│  views/ + editor modal  │ ◀────────────────────── │   delgado, sin lógica    │
└─────────────────────────┘        JSON              └────────────┬─────────────┘
                                                                   │ llama a
                                                                   ▼
                                                     ┌──────────────────────────┐
                                                     │   crates/cf-core         │
                                                     │   dominio puro:          │
                                                     │   frontmatter, scan,     │
                                                     │   staging (stage/diff/   │
                                                     │   apply/revert/backup)   │
                                                     └──────────────────────────┘
```

`cf-core` no depende de Tauri — se testea aislado (`cargo test -p cf-core`) sin
compilar el WebView. `src-tauri` tiene su propio `[workspace]` vacío para
mantener ese desacople, y solo contiene comandos delgados que delegan al
dominio.

## Carpetas

| Carpeta | Qué hay |
|---|---|
| `crates/cf-core/` | Dominio puro: parseo de frontmatter, modelos, escaneo de `~/.claude`, motor de staging (stage/diff/apply/backup/revert) |
| `src-tauri/` | Comandos Tauri (delgados, async), capability scoped a `~/.claude`, guardrail de paths, logging |
| `src/` | Frontend: `views/` por sección, `lib/api.ts` (wrapper de `invoke` con validación runtime), `lib/editor.ts` (modal de edición), `lib/cards.ts` |

## El invariante de seguridad: staging con revisión

Ningún comando escribe directo a un archivo real de `~/.claude`. El único
camino es:

1. **`stage_change`** — guarda un borrador en el directorio de datos propio de
   la app (no en `~/.claude`).
2. Revisar el **diff** del borrador contra el archivo real actual.
3. **`apply_staged`** — backup con timestamp del original, después escribe el
   archivo real. `discard_staged` tira el borrador sin tocar nada.
4. **`list_history` / `revert_applied`** — deshacer cualquier cambio aplicado,
   restaurando su backup.

### Garantías con test (contrato)

Cada garantía de esta tabla tiene tests con nombre 1:1 que fallan si se rompe.
Viven en tres lugares: el módulo de dominio (`crates/cf-core/src/staging.rs`,
mod `tests`), la capa de comandos con I/O real
(`src-tauri/tests/staging_integration.rs`) y la configuración declarativa de
seguridad (`src-tauri/tests/config_contract.rs`). Se corren con
`cargo test -p cf-core` (dominio), `cargo test` dentro de `src-tauri`
y `npm test` (frontend)
(comandos + config); CI ejecuta ambos en cada push y PR.

| Garantía | Test(s) que fallan si se rompe |
|---|---|
| `stage_change` nunca escribe el archivo real | `stage_does_not_touch_the_real_file` (cf-core) · `stage_change_does_not_touch_the_real_file_until_apply` (staging_integration) |
| `apply` hace backup del original ANTES de sobreescribir | `apply_writes_real_file_backs_up_original_and_clears_draft`, `apply_on_a_new_file_backs_up_as_empty` (cf-core) · `apply_staged_backs_up_before_writing_and_the_backup_predates_the_new_content` (staging_integration) |
| `discard` tira el borrador sin tocar nada | `discard_removes_the_draft_and_leaves_real_file_untouched` (cf-core) · `discard_staged_leaves_the_real_file_untouched` (staging_integration) |
| `revert` restaura el backup y queda registrado en el historial | `revert_restores_the_backup_and_logs_a_new_history_entry` (cf-core) · `a_corrupted_target_after_apply_is_recoverable_via_revert_from_backup` (staging_integration) |
| El boundary `~/.claude` se revalida en TODO camino de escritura, fail-closed, incluso con borrador/historial adulterado | `apply_rejects_a_target_outside_the_boundary`, `revert_rejects_a_target_outside_the_boundary`, `ensure_within_rejects_escapes_and_accepts_inside` (cf-core) · `stage_change_rejects_a_target_outside_claude_dir`, `apply_staged_rejects_a_tampered_draft_pointing_outside_the_boundary` (staging_integration) |
| Un `..` detrás de un componente que todavía no existe no escapa del boundary (el guardrail no descarta tramos del path sin resolverlos) | `ensure_within_rejects_a_dotdot_behind_a_component_that_does_not_exist_yet`, `apply_rejects_a_dotdot_escape_through_a_missing_directory_and_writes_nothing` (cf-core) |
| El `id` de un borrador llega del IPC pero nunca se usa crudo como nombre de archivo | `discard_rejects_a_traversal_id_instead_of_deleting_outside_the_staging_dir`, `get_rejects_a_traversal_id_instead_of_reading_outside_the_staging_dir` (cf-core) |
| Revertir la creación de un archivo lo borra (no deja un archivo vacío), y ese borrado también es reversible | `reverting_the_creation_of_a_file_deletes_it_instead_of_leaving_it_empty`, `reverting_the_deletion_of_a_created_file_can_bring_it_back`, `a_history_entry_without_the_existed_field_still_deserializes_as_existing` (cf-core) |
| `settings.json` (y todo `.json`) se parchea por clave: merge recursivo que preserva las claves que el borrador no menciona | `applying_settings_json_preserves_keys_the_draft_did_not_mention` (cf-core) · `applying_settings_json_preserves_a_concurrent_external_edit_the_draft_did_not_touch` (staging_integration) |
| JSON inválido en el borrador falla sin tocar el archivo real | `applying_malformed_settings_json_fails_closed_and_leaves_the_file_intact` (cf-core) |
| El diff que revisás es exactamente lo que `apply` escribe (se calcula contra el resultado mergeado y contra el disco actual, no contra una foto vieja) | `diff_of_settings_json_matches_the_merged_content_that_apply_writes`, `diff_reflects_current_disk_state_not_just_the_draft` (cf-core) |
| La webview no tiene NINGÚN permiso de filesystem (ni scoped): el único camino de escritura es `stage_change` → `apply_staged`, y el plugin `fs` no está registrado | `fs_capability_grants_no_filesystem_permission_at_all`, `the_app_does_not_register_the_filesystem_plugin` (config_contract) |
| Los caminos de LECTURA degradan en vez de romper: basura en el staging dir o una línea corrupta en `history.jsonl` no inutilizan borradores, historial ni revert | `an_unexpected_file_in_the_staging_dir_does_not_break_listing_or_staging`, `a_corrupt_line_in_the_history_does_not_kill_the_rest_of_the_log` (cf-core) |
| Una instalación sin `~/.claude/CLAUDE.md` muestra estado vacío, no un errno del SO | `read_claude_md_on_a_fresh_install_returns_empty_not_an_error` (scan_integration) |
| `read_file_content` (el único comando que recibe un path del IPC) rechaza todo path fuera de `~/.claude` y devuelve error tipado si el archivo desapareció | `rejects_a_relative_dotdot_escape_without_reading_anything`, `rejects_an_absolute_path_outside_the_claude_dir`, `rejects_a_dotdot_behind_a_directory_that_does_not_exist`, `a_file_deleted_between_listing_and_opening_returns_a_typed_io_error` (read_integration) |
| El escapado del frontend aguanta contexto de ATRIBUTO: un `name:` de frontmatter con comilla doble no inyecta atributos ni anula `data-readonly` | `render.test.ts` (vitest + jsdom, `npm test`) |
| La vista general renderiza `model`/`theme` de `settings.json` como texto, no como markup | `overview.test.ts` (vitest + jsdom) |
| CSP restrictiva en producción (`script-src 'self'`, sin `unsafe-eval`, `object-src`/`base-uri`/`frame-ancestors 'none'`) | `production_csp_is_restrictive` (config_contract) |

Comportamiento ante edición externa concurrente (documentado por test, no
aspiracional): en archivos planos el borrador gana pero la edición externa queda
recuperable en el backup
(`applying_a_plain_file_overwrites_a_concurrent_external_edit_but_it_stays_recoverable_in_the_backup`);
en `.json` una clave agregada por fuera que el borrador no menciona sobrevive al
merge (`applying_settings_json_preserves_a_concurrent_external_edit_the_draft_did_not_touch`).

### Defensas sin test dedicado (todavía)

Estas existen en el código pero hoy no tienen un test que falle si se rompen,
así que se listan como diseño, no como contrato:

- Logging de toda mutación (intento y resultado) a un archivo en el dir de la
  app (`log_result` en `src-tauri/src/commands/staging.rs`).

Alcance editable: **memorias, skills, comandos/workflows y `settings.json`**.
Agentes y tareas programadas se muestran, pero de solo lectura.

## Desarrollo

Requiere: Rust estable (toolchain MSVC en Windows), Node 20+, y en Windows los
Build Tools de Visual Studio (MSVC) + el WebView2 Runtime.

```bash
npm install
npm run tauri dev       # levanta Vite (1420) + la ventana
cargo test -p cf-core   # tests de dominio, sin compilar Tauri
npm test                # tests de frontend (vitest + jsdom)
npm run verify          # el gate completo (fmt + clippy + cargo test + tsc + vitest + build), igual que CI
```

## Qué corre en CI

Generado a partir de los workflows reales — si esta lista queda desactualizada
es porque `.github/workflows/*.yml` cambió y esto no se actualizó con eso.

**`ci.yml`** (push a `main`/`master` y todo PR):

- Job **`verify`** (`windows-latest` — hace falta MSVC + WebView2 para linkear
  el crate de Tauri):
  1. `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D
     warnings` + `cargo test --workspace` sobre el workspace raíz (`cf-core`).
  2. Lo mismo (`fmt --check`, `clippy --all-targets -- -D warnings`, `test`,
     `build`) sobre `src-tauri`, que tiene su propio `[workspace]`.
  3. Frontend: `npx tsc --noEmit` + `npm run build`.
  - `RUSTFLAGS: -D warnings` a nivel de job: cualquier warning del compilador
    (no solo de clippy) rompe el build.
- Job **`bundle`** (solo en tags `v*`, `windows-latest`): `npm run tauri
  build` — ejercita el empaquetado real (icon + bundle), deliberadamente no
  corre en cada push por ser caro.

**`links.yml`** (push, PR, y cron semanal los lunes 06:00 UTC): corre
[`lychee`](https://github.com/lycheeverse/lychee-action) sobre `README.md` y
`CLAUDE.md` para detectar links/badges rotos — incluye el cron porque los
links externos se pudren solos con el tiempo, no solo cuando alguien edita el
doc.

## Estado

Funcional end-to-end: dashboard en vivo (visión general, reglas, memoria,
skills, comandos/workflows, agentes, automatización, historial) y edición con
staging + diff + aplicar/descartar/revertir para los 4 tipos editables.

**Limitación conocida:** los backups y el `history.jsonl` crecen sin cota. No
hay retención automática a propósito — podar sin orfanar entradas revertibles
requiere una decisión de producto, y una retención ingenua sería un bug de
pérdida de datos. Pendiente de diseño.

## Licencia

MIT — ver [LICENSE](LICENSE).
