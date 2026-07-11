# Threat model de claude-flow-studio

Este documento cubre qué puede hacer un ataque contra `claude-flow-studio` (o un
bug propio) y qué protege el código real hoy — `crates/cf-core/src/staging.rs`,
`src-tauri/src/paths.rs`, `src-tauri/src/commands/`, `src-tauri/capabilities/` y
`src-tauri/tauri.conf.json` — no aspiración. Se basa en el mismo patrón que
`cognitive-substrate-os/docs/threat-model.md`, adaptado a esta app: acá no hay
sandbox de agentes, el activo es la configuración de Claude Code del usuario y
el vector es la app de escritorio misma (o su webview) escribiendo sobre
`~/.claude`.

## Activos sensibles

- **`~/.claude/settings.json`**: puede contener configuración con valor para un
  atacante (permisos habilitados, rutas de hooks, referencias a MCP servers).
  No es un vault de credenciales, pero un `apply` corrupto o malicioso ahí
  cambia qué puede ejecutar Claude Code sin que el usuario lo note.
- **Memorias, skills, comandos y workflows** (`~/.claude/memory/`, `skills/`,
  `commands/`): texto en Markdown que Claude Code lee y sigue como
  instrucciones. Escribir contenido arbitrario ahí es, en efecto, inyectar
  instrucciones persistentes en las sesiones futuras del usuario — el mismo
  activo que protege un `CLAUDE.md`.
- **Backups e historial** (`app_data_dir()/backups/`, `history.jsonl`): viven
  FUERA de `~/.claude`, sin la capability de fs que scopea el resto de la app.
  Son de solo-lectura desde la perspectiva del usuario (nadie los edita a
  mano), pero es el mecanismo de recuperación — si se corrompen o se pierden,
  se pierde la posibilidad de revertir.
- **El propio filesystem del usuario fuera de `~/.claude`**: el activo que
  todo el diseño de boundary existe para proteger. Un bug de path traversal acá
  no se limita a "mal funcionamiento de la app de config", escala a "una app de
  escritorio con capability de fs escribe donde quiera en la máquina".

Nota: esta app no maneja API keys ni tokens propios (no hay login, no hay
backend remoto). El riesgo no es exfiltración de secretos de la app — es
que la app misma es un vector de escritura no autorizada sobre `~/.claude`.

## Vectores de ataque considerados y cubiertos (con test real)

- **Escritura directa a un archivo real sin pasar por staging**: no existe
  ningún comando `#[tauri::command]` que escriba directo — el único camino es
  `stage_change` → `apply_staged` (`src-tauri/src/commands/staging.rs`).
  Cubierto por `stage_change_does_not_touch_the_real_file_until_apply` y
  `discard_staged_leaves_the_real_file_untouched`
  (`src-tauri/tests/staging_integration.rs`).
- **Path traversal / escape de `~/.claude` al crear un borrador**:
  `ensure_within_claude_dir` (`src-tauri/src/paths.rs`) canonicaliza el target
  antes de compararlo contra la raíz — neutraliza `..` en cualquier posición y
  symlinks que apunten afuera. Cubierto en `paths.rs` (`rejects_a_dotdot_escape…`)
  y en la integración (`stage_change_rejects_a_target_outside_claude_dir`).
- **Borrador o `history.jsonl` adulterado apuntando fuera del boundary**: el
  boundary se revalida DE NUEVO dentro de `apply`/`revert`
  (`StagingStore::with_boundary`, `crates/cf-core/src/staging.rs`) — defensa en
  profundidad, no confía en que el chequeo de `stage` sea el único guardia.
  Cubierto por `apply_rejects_a_target_outside_the_boundary` /
  `revert_rejects_a_target_outside_the_boundary` en cf-core, y
  `apply_staged_rejects_a_tampered_draft_pointing_outside_the_boundary` en la
  integración de `src-tauri`.
- **`settings.json` inválido pisando el real**: `resolved_content` parsea el
  borrador como JSON antes de tocar disco — un borrador malformado falla
  cerrado (`StagingError::Serde`) sin escribir nada. Cubierto por
  `applying_malformed_settings_json_fails_closed_and_leaves_the_file_intact`.
- **Pérdida de datos por interrupción a mitad de un `apply`**: el orden es
  siempre backup → escribir (nunca al revés), así que un crash entre ambos
  pasos deja el archivo real intacto (sin backup nuevo) o ya actualizado (con
  backup del estado previo) — nunca un estado sin backup del contenido de
  antes. Cubierto por
  `a_corrupted_target_after_apply_is_recoverable_via_revert_from_backup`: aunque
  el archivo real termine en un estado corrupto por CUALQUIER motivo posterior,
  el backup sigue intacto y `revert_applied` restaura desde ahí.
- **Escalada de privilegio de filesystem vía `fs:default`**: la capability
  (`src-tauri/capabilities/claude-config-access.json`) usa permisos scoped
  (`fs:allow-read-file`, `fs:allow-write-file`, etc. con `allow: [{ path:
  "$HOME/.claude/**" }]`), no `fs:default`. Cualquier comando nuevo que toque
  filesystem sin extender esa capability queda sin permiso a nivel del plugin.
- **Inyección de contenido vía la webview (XSS)**: `tauri.conf.json` fija una
  CSP restrictiva en producción (`script-src 'self'`, sin `unsafe-inline` ni
  `unsafe-eval`, `object-src 'none'`, `frame-ancestors 'none'`) — el modo dev
  relaja `script-src`/`connect-src` para el HMR de Vite, pero eso no aplica al
  build empaquetado. El frontend además escapa HTML en los sinks donde
  renderiza contenido de `~/.claude` (nombres/contenido de memorias, skills,
  etc. son datos no confiables por definición: el usuario o un agente los
  escribió).

## Vectores de ataque NO cubiertos todavía (gap real, no hipotético)

- **Edición externa concurrente de `~/.claude` mientras hay staging
  pendiente**: no es un vector "malicioso" en el sentido de ataque, pero es un
  gap de integridad real. Documentado y testeado en
  `applying_a_plain_file_overwrites_a_concurrent_external_edit_but_it_stays_recoverable_in_the_backup`
  y `applying_settings_json_preserves_a_concurrent_external_edit_the_draft_did_not_touch`
  (`src-tauri/tests/staging_integration.rs`): para archivos no-JSON, `apply`
  pisa cualquier edición externa concurrente con el `draft_content` tal cual
  (no hay merge de 3 vías ni detección de conflicto) — la edición externa NO
  se pierde del todo porque queda en el backup, pero sí se pierde del archivo
  aplicado sin ningún aviso al usuario. Para `settings.json` el merge por clave
  mitiga el caso de claves no superpuestas, pero una clave que ambos lados
  tocan sigue resolviéndose a favor del draft, en silencio.
- **Retención sin cota de backups e `history.jsonl`**: ya documentado en
  `CLAUDE.md` y el README como limitación conocida de producto, no de
  seguridad — pero tiene una arista de seguridad: un backup viejo de un
  `settings.json` con configuración sensible nunca se borra ni se rota, y
  vive en `app_data_dir()` sin la protección de la capability scoped a
  `~/.claude`. Pendiente de una decisión de retención (ver CLAUDE.md).
- **Sin límite de tamaño en `draft_content`**: `stage_change` acepta
  cualquier string como borrador sin validar tamaño — un borrador
  extremadamente grande (por bug del frontend, no por atacante externo, ya que
  no hay superficie de red) podría escribirse igual a `staging_dir` sin límite.
  Bajo impacto hoy (la app no expone `stage_change` a nada fuera del propio
  frontend empaquetado con CSP `script-src 'self'`), pero no hay guardrail
  explícito si eso cambiara.
- **Aislamiento del proceso de I/O**: los comandos hacen `std::fs` directo
  (no pasan por `tauri-plugin-fs`, ver comentario en `paths.rs`), así que la
  única barrera real es el chequeo de boundary en código Rust propio — no hay
  una segunda capa de sandboxing de sistema operativo (a diferencia de, por
  ejemplo, un contenedor). Un bug de lógica en `ensure_within`/
  `ensure_within_claude_dir` no cubierto por los tests de arriba sería
  explotable sin ninguna red de contención adicional.

## Cómo se actualiza este documento

Cuando se agregue un test `_impl`/de dominio que cubra un vector nuevo,
sumarlo a la lista de "cubiertos" de arriba con el nombre del test. Cuando se
resuelva un ítem de "no cubiertos" (p. ej. detección de conflicto en
`apply`, o retención de backups), moverlo a "cubiertos" con la referencia.
