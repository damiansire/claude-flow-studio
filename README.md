# claude-flow-studio

App de escritorio (Tauri v2 + Rust) para ver y editar tu configuración de
Claude Code (`~/.claude`): memoria, skills, comandos, workflows y settings —
con staging + revisión, nunca escritura directa.

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

`cf-core` no depende de Tauri — se testea aislado (`cargo test -p cf-core`)
sin compilar WebView. `src-tauri` tiene su propio `[workspace]` vacío para
mantener ese desacople.

## Carpetas

| Carpeta | Qué hay |
|---|---|
| `crates/cf-core/` | Dominio: parseo de frontmatter, modelos, escaneo de `~/.claude`, motor de staging |
| `src-tauri/` | Comandos Tauri (delgados), capability scoped a `~/.claude`, guardrail de paths |
| `src/` | Frontend: `views/` por sección, `lib/api.ts` (wrapper de `invoke`), `lib/editor.ts` (modal genérico) |

## El invariante de seguridad

Ningún comando escribe directo a un archivo real de `~/.claude`. El único
camino es: **`stage_change`** (guarda un borrador en el directorio de datos
propio de la app) → revisar el diff → **`apply_staged`** (backup con timestamp
+ escribe el archivo real). `discard_staged` tira el borrador sin tocar nada.
`list_history` / `revert_applied` permiten deshacer cualquier cambio ya
aplicado.

Alcance editable: **memorias, skills, comandos/workflows y `settings.json`**.
Agentes y tareas programadas se muestran, pero de solo lectura.

## Desarrollo

Requiere: Rust estable (toolchain MSVC en Windows), Node 20+, y en Windows los
Build Tools de Visual Studio (MSVC) + WebView2 Runtime.

```
npm install
npm run tauri dev       # levanta Vite (1420) + la ventana
cargo test -p cf-core   # tests de dominio, sin compilar Tauri
./verify.sh             # el mismo gate que corre CI
```

## Estado

Funcional: dashboard en vivo (memoria/skills/comandos/workflows/agentes/
automatización), edición con staging+diff+aplicar/descartar para memorias,
skills, comandos/workflows y `settings.json`, historial con revert.

## Licencia

MIT — ver [LICENSE](LICENSE).
