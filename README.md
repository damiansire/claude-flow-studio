# claude-flow-studio

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

Detalles que son **contrato, no aspiración** (hay tests que fallan si se rompen):

- **El boundary `~/.claude` se revalida en TODO camino de escritura** (no solo
  al crear el borrador): `apply`/`revert` rechazan fail-closed cualquier path
  fuera del directorio antes de escribir — defensa en profundidad ante un
  borrador o historial adulterado.
- **`settings.json` (y todo `.json`) se parchea por clave**: se parsea el
  borrador (JSON inválido falla sin tocar el archivo) y se mergea recursivamente
  sobre el real, **preservando las claves que el borrador no menciona**. El diff
  se calcula contra ese resultado, así lo que revisás es lo que se aplica.

Alcance editable: **memorias, skills, comandos/workflows y `settings.json`**.
Agentes y tareas programadas se muestran, pero de solo lectura.

Además: capability de filesystem scoped a `$HOME/.claude/**` (mínimo
privilegio), CSP restrictiva en producción, escapado de HTML en todos los sinks,
y logging de las mutaciones a un archivo en el dir de la app.

## Desarrollo

Requiere: Rust estable (toolchain MSVC en Windows), Node 20+, y en Windows los
Build Tools de Visual Studio (MSVC) + el WebView2 Runtime.

```bash
npm install
npm run tauri dev       # levanta Vite (1420) + la ventana
cargo test -p cf-core   # tests de dominio, sin compilar Tauri
./verify.sh             # el gate completo (fmt + clippy + test + build + tsc), igual que CI
```

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
