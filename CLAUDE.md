# CLAUDE.md — claude-flow-studio

Hereda las reglas globales (commits en español, no push fin de semana, fases chicas,
tests antes que UI, design-review antes de cerrar pantallas). Esto es lo específico
de este repo.

## Qué es

App de escritorio (Tauri v2 + Rust) para ver y editar la configuración de Claude Code
(`~/.claude`): memoria, skills, comandos, workflows y settings. Nace del plan en
`C:\Users\tester\.claude\plans\swift-watching-muffin.md` — ese archivo tiene el
contexto completo y las fases.

## Límite de crates (no lo cruces)

- **`crates/cf-core`** — dominio puro. Parseo de frontmatter, modelos, escaneo de
  `~/.claude`, motor de staging/diff/apply/backup. **Cero dependencias de Tauri ni de
  IPC.** `cargo test -p cf-core` tiene que correr sin tocar `src-tauri`.
- **`src-tauri`** — tiene su **propio `[workspace]` vacío** (Cargo.toml), a propósito
  desacoplado del workspace raíz. Solo comandos delgados (`#[tauri::command]`) que
  delegan a `cf-core`; nada de lógica de dominio acá.
- **`src/`** — frontend Vite + TS plano (sin framework: es un dashboard chico, meter
  Angular sería sobre-ingeniería para este alcance). Llama al backend vía `invoke()`.

## El invariante de seguridad: staging con revisión

Nunca se escribe directo a un archivo real de `~/.claude`. Todo cambio pasa por:
borrador (en el directorio de datos de la app, no en `~/.claude`) → diff contra el
archivo real → "Aplicar" (con backup timestamped) o "Descartar". `settings.json` se
parchea por clave (`serde_json::Value`), nunca se reescribe entero. Si tocás
`staging.rs`, no rompas este flujo — es la razón de ser del proyecto (ver el plan
para el porqué).

## Permisos filesystem

`src-tauri/capabilities/claude-config-access.json` scoped a `$HOME/.claude/**` —
principio de mínimo privilegio. Si agregás un comando nuevo que toque filesystem,
extendé esa capability, no uses `fs:default`.

## Dev

```
npm install
npm run tauri dev      # levanta Vite (1420) + la ventana
cargo test -p cf-core  # tests de dominio, sin compilar Tauri
```

`vite.config.ts` ignora `src-tauri/**` en el watcher — sin eso, Vite choca (EBUSY en
Windows) contra los `.dll` que Cargo reescribe en cada build.
