# CLAUDE.md — claude-flow-studio

Convenciones de trabajo: commits en español, no push fin de semana, fases chicas,
tests antes que UI, revisión de diseño con captura real antes de cerrar pantallas.
Lo que sigue es lo específico de este repo.

## Qué es

App de escritorio (Tauri v2 + Rust) para ver y editar la configuración de Claude Code
(`~/.claude`): memoria, skills, comandos, workflows y settings.

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
archivo real → "Aplicar" (con backup timestamped) o "Descartar". Detalles que son
contrato, no aspiración (hay tests que fallan si se rompen):

- **El boundary se revalida en TODO camino de escritura**, no solo al crear el
  borrador: `StagingStore::with_boundary` hace que `apply`/`revert` rechacen
  (fail-closed) cualquier `target_path` fuera de `~/.claude` antes de escribir —
  defensa en profundidad ante un borrador o `history.jsonl` adulterado.
- **`settings.json` (y todo `.json`) se parchea por clave**: se parsea el borrador
  (`serde_json::Value`; JSON inválido falla sin tocar el archivo) y se mergea
  recursivamente sobre el archivo real, preservando las claves que el borrador no
  menciona. El diff se calcula contra ese resultado mergeado, así lo que se revisa
  es exactamente lo que se aplica.

Si tocás `staging.rs`, no rompas este flujo — es la razón de ser del proyecto.

**Asimetría abierta (pendiente de decisión de producto):** el editor muestra el
archivo `.json` ENTERO, pero se aplica con merge por clave, así que borrar una
clave en el borrador no la elimina del archivo real. Hoy el editor lo avisa al
abrir (`editor.ts`) y el test `applying_settings_json_cannot_delete_a_key` fija
la consecuencia, para que deje de ser tácita. Resolverlo de verdad exige elegir
entre semántica de reemplazo (coherente con el textarea de archivo entero, pero
rompe la garantía de preservar claves que la app no conoce) y un camino de
borrado explícito en la UI. No lo cambies sin esa decisión.

**Limitación conocida:** los backups y el `history.jsonl` crecen sin cota. No hay
retención automática *a propósito*: podar backups sin orfanar entradas revertibles
del historial requiere una decisión de producto (cuánto historial conservar vs.
revertibilidad), y una retención ingenua sería un bug de pérdida de datos. Pendiente
de diseño, no un descuido.

## Permisos filesystem

`src-tauri/capabilities/claude-config-access.json` declara **solo `core:default`**:
cero permisos `fs:*`, ni siquiera scoped. El IO real lo hacen los comandos propios
(`std::fs` en `cf-core`) acotados por `ensure_within_claude_dir`, no la capability.
Un `fs:allow-write-file` le abriría a la webview un `plugin:fs|write_file` que
saltea staging, diff, backup e historial: el invariante del producto. Si un comando
nuevo necesita filesystem, se escribe como comando propio con el guardrail, no se
concede un permiso de plugin. `tests/config_contract.rs` falla si vuelve a aparecer
un permiso `fs:*` o el registro de `tauri_plugin_fs`.

## Dev

```
npm install
npm run tauri dev      # levanta Vite (1420) + la ventana
cargo test -p cf-core  # tests de dominio, sin compilar Tauri
```

`vite.config.ts` ignora `src-tauri/**` en el watcher — sin eso, Vite choca (EBUSY en
Windows) contra los `.dll` que Cargo reescribe en cada build.

## Estándar nivel mundial

Esta es la barra contra la que se **construye** el repo, no una auditoría a posteriori: si
algo de acá no se cumple, no está listo. Las reglas se apoyan en proyectos open source de
referencia, y donde hay fuente pública se cita su URL. Este archivo es la única fuente de
verdad de la barra: no inventes reglas nuevas acá, y si falta una, primero se respalda con
una fuente pública.

Stacks que aplican: **rust** + **tauri** (workspace raíz con `crates/cf-core`, y `src-tauri`)
y **node-ts** + **typescript** (frontend Vite plano en `src/`). Arquetipo: app de escritorio
local de edición segura de configuración, con el motor de staging/diff/backup/revert como
corazón.

### Piso Craft (a-j): no negociable

Principio rector, propio del proyecto: el código tiene que ser tan evidente que un senior
entienda el porqué sin preguntar ni ejecutarlo.

- **a. El nombre revela la intención de dominio, no el mecanismo.** `stage` / `diff` /
  `apply` / `revert` / `discard` son términos del problema. Nada de `data`, `manager`,
  `process`, `handle` donde el dominio ya tiene su palabra.
- **b. Los comentarios explican POR QUÉ, nunca QUÉ.** El estilo ya vigente en `staging.rs` y
  `paths.rs` (por qué el frontmatter se guarda crudo, por qué el parser YAML-lite es
  deliberadamente tosco) es la barra, no la excepción.
- **c. Superficie pública autodocumentada.** La firma comunica el contrato. Si hay que leer
  el cuerpo de un `#[tauri::command]` o de `StagingStore` para saber qué hace, se arregla la
  firma, no se agrega un comentario.
- **d. Impacto mínimo al cambiar el core.** Una regla vive en un solo lugar. Lógica de
  dominio o de boundary duplicada entre `cf-core` y `src-tauri` es violación directa (dos
  copias divergen solas), no "defensa en profundidad": la defensa en profundidad son dos
  **llamadas** en dos momentos distintos a una **única** implementación.
- **e. Features borrables sin cirugía.** Cada vista y cada tipo editable vive localizado;
  sacarlo no deja tentáculos en el resto.
- **f. Flujo de datos inmutable y rastreable.** El estado se deriva; nada de mutar estado
  compartido a escondidas desde una vista o un handler.
- **g. Consistencia ante excepción.** Toda escritura a `~/.claude` deja el archivo entero o
  no lo toca: temporal + rename atómico, y el registro del historial **antes** de aplicar
  (log-before-apply). Un fallo a mitad no puede dejar un archivo truncado sin revert.
- **h. Los boundaries comunican lo que pasa.** Toda mutación (intento y resultado) y todo
  fallo de un comando IPC quedan logueados. Nada se traga en silencio, incluido el panic:
  un arranque que falla sin mensaje es un boundary mudo.
- **i. Límites explícitos.** Sin loops ni reintentos sin tope. Ninguna lectura de disco
  potencialmente ilimitada se parsea entera en memoria; `history.jsonl` es el caso vivo
  (una lectura sin tope puede agotar la memoria).
- **j. Fail-closed donde importa.** Ante duda de path o de permiso, se deniega, y la rama por
  defecto de toda decisión de boundary DENIEGA. Un guardrail evadible cuenta como inerte: si
  existe un segundo canal de escritura a `~/.claude` que no pasa por el staging, el ítem está
  violado por definición, por más tests que tenga el camino principal.

### Legibilidad en frío (k-m): el artefacto se explica solo en 30 segundos

Referencias públicas: [ripgrep](https://github.com/BurntSushi/ripgrep) (su README),
[la guía de GitHub sobre READMEs](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-readmes)
y [SQLite: qué lo hace distinto](https://sqlite.org/different.html).

- **k. El README lidera con prueba visible y framing honesto.** Esto es una **app con GUI**:
  la descripción textual NO cuenta como prueba visible. El primer screenful necesita
  **captura o GIF de la pantalla real** (el flujo stage → diff → aplicar es justo lo que hay
  que mostrar), más el statement de qué es y para quién. Sin captura, k está violado por
  arquetipo aunque el texto sea impecable: es el caso típico de sub-descripción, trabajo
  bueno que no se deja ver.
- **l. Donde prometés robustez, la prueba está y es reproducible.** Este repo promete
  **robustez**, no performance: el equivalente de "benchmark con números" es la matriz de
  casos borde ejercitada por tests, más la tabla garantía→test 1:1 del README. Toda línea del
  README o de `docs/threat-model.md` que afirme una protección nombra un test que existe y
  que falla si la protección se rompe. Prometer una garantía sin test en esa ruta es
  violación.
- **m. Framing honesto: reconocés el sesgo y el límite del claim.** Se etiqueta dónde el
  producto NO llega (límites, gaps del threat model, decisiones diferidas) en vez de
  cherry-pick. Una limitación nombrada da más confianza que una garantía pulida, y una
  garantía documentada que el código desmiente es peor que no documentar nada.

### Techo de Craft: lo que mueve de "ok" a referencia

Referencias públicas: [commit atómico en SQLite](https://www.sqlite.org/atomiccommit.html),
[cómo se testea SQLite](https://sqlite.org/testing.html),
[WAL en PostgreSQL](https://www.postgresql.org/docs/current/wal-intro.html),
[estándares de código de LLVM](https://llvm.org/docs/CodingStandards.html),
[import-restrictions de Kubernetes](https://github.com/kubernetes/kubernetes/blob/master/staging/publishing/import-restrictions.yaml)
y [tracing-error](https://github.com/tokio-rs/tracing/blob/master/tracing-error/src/lib.rs).

- **Nombres y superficie:** imposible de malusar, no solo legible. Un único contrato de
  boundary bien especificado (no dos copias), y tipos que hagan que el estado inválido no
  compile.
- **Encapsulamiento:** el límite es un hecho de CI, no una convención escrita. El corte
  "`cf-core` no conoce Tauri" se gatea (hoy: su propio workspace + `cargo test -p cf-core`),
  no se pide por favor.
- **Integridad de estado:** invariante observable **y fault-injected**. Commit atómico
  reificado, log-before-apply (WAL/REDO), recovery gateado por condiciones enumeradas, y
  crashes a mitad de escritura **simulados en test** (patrón SQLite/PostgreSQL). El motor de
  apply/revert no llega al techo hasta que exista un test que corte la escritura por la
  mitad y verifique que el archivo real quedó intacto o restaurable.
- **Observabilidad:** contexto capturado en el **origen**, render diferido.
- **Resiliencia:** fail-closed por defecto, y los límites de carga o retención son campos de
  config **explícitos y nombrados**, nunca implícitos (aplica directo a la retención de
  backups e historial que hoy está diferida).
- **Config declarativa como contrato ejecutable:** el patrón ya vivo en
  `src-tauri/tests/config_contract.rs` (parsear la capability y la CSP y assertearlas
  directiva por directiva) es techo real, y se extiende a toda config nueva que sea
  perímetro.

### Reglas enforzables: Rust y Tauri

- **Lints centralizados en `[workspace.lints]`**, heredados con `[lints] workspace = true`.
  Un solo lugar manda; un crate que redeclara strictness es drift. `src-tauri` tiene su
  propio `[workspace]` vacío a propósito, así que **espeja** el bloque del raíz: si tocás
  uno, tocás los dos. Es la única duplicación aceptada, y es de configuración, no de lógica.
  (Referencias: [oxc](https://github.com/oxc-project/oxc), [biome](https://github.com/biomejs/biome).)
- **`allow_attributes = "deny"`:** prohibido `#[allow]` suelto; va `#[expect(...)]`, que falla
  cuando el lint deja de dispararse. Nada de supresiones zombie.
  (Referencia: [biome](https://github.com/biomejs/biome).)
- **`unsafe_code = "forbid"`** a nivel workspace. Si algún día entra `unsafe`, pasa a `deny`
  con `// SAFETY:` obligatorio (`undocumented_unsafe_blocks`).
  (Referencias: [egui](https://github.com/emilk/egui), [oxc](https://github.com/oxc-project/oxc).)
- **Restriction lints prendidos:** `dbg_macro`, `todo`, `unimplemented`, `print_stdout`,
  `print_stderr`, `get_unwrap`, `mem_forget`, `clone_on_ref_ptr`. Dos del kit quedan como
  deuda declarada, no como excepción permanente: `unwrap_used` (allow-unwrap-in-tests no
  cubre los helpers fuera de `#[test]` en `crates/cf-core/tests/`, así que prenderlo pide
  reescribir esos helpers primero) y `exit` (lo dispara el código que genera
  `tauri::generate_context!`, no código nuestro). (Referencias:
  [rerun_template](https://github.com/rerun-io/rerun_template), [egui](https://github.com/emilk/egui).)
- **`unwrap()` / `expect()` / `panic!` en producción es red flag:** todo camino falible
  devuelve `Result` con `?` y errores tipados con `thiserror`. Incluye el arranque: un
  `expect()` en `run()` deja al usuario sin ningún mensaje. (Referencia: [The Rust Book](https://doc.rust-lang.org/book/).)
- **`.clippy.toml` con prohibiciones explícitas**, no solo thresholds: `allow-unwrap-in-tests`
  y `disallowed-methods` (`std::thread::spawn` → `Builder` con nombre, `std::env::temp_dir` →
  `tempfile`). (Referencias: [rerun_template](https://github.com/rerun-io/rerun_template),
  [spacedrive](https://github.com/spacedriveapp/spacedrive).)
- **Deps pinneadas una sola vez** en `[workspace.dependencies]`, los crates usan
  `dep.workspace = true`; MSRV declarado en `rust-version`.
  (Referencia: [spacedrive](https://github.com/spacedriveapp/spacedrive).)
- **Superficie pública cerrada:** `pub(crate)` por defecto en `cf-core`; `pub` es una decisión
  explícita, no el default. (Referencia: [babel](https://github.com/babel/babel).)
- **Capabilities mínimas (Tauri v2):** allowlist explícita, cero wildcards de filesystem, y
  **ningún permiso que abra un canal capaz de saltear el invariante del producto**. Cada
  permiso concedido corresponde a un comando que la app realmente invoca; si no lo invoca, se
  saca. (Referencias: [capabilities de Tauri v2](https://v2.tauri.app/security/capabilities/), [spacedrive](https://github.com/spacedriveapp/spacedrive).)
- **El IPC es cuello de botella y contrato a la vez:** el esquema Rust↔TS se valida en runtime
  del lado TS para **todos** los payloads, no para la mitad.
- **Formato determinista:** `.rustfmt.toml` (`max_width`, `newline_style = "Unix"`, reorder de
  imports y módulos) más `.gitattributes` con `* text=auto eol=lf`. Sin eso `cargo fmt
  --check` falla distinto en Windows que en CI.
  (Referencia: [spacedrive](https://github.com/spacedriveapp/spacedrive).)

### Reglas enforzables: frontend TypeScript

- **tsconfig estricto en UN solo lugar y con el techo de strictness.** Codebase chica: se nace
  con el techo, porque el costo es cero al inicio y solo sube con el tiempo. `strict` más
  `exactOptionalPropertyTypes`, `noImplicitOverride`, `noImplicitReturns`,
  `noFallthroughCasesInSwitch`, `isolatedModules`, `noUnusedLocals`, `noUnusedParameters`,
  `allowUnreachableCode: false`, `allowUnusedLabels: false`. Faltan `noUncheckedIndexedAccess`
  y `noPropertyAccessFromIndexSignature`, anotados en el propio `tsconfig.json` con los
  archivos que hay que arreglar antes de prenderlos: son deuda con fecha de vencimiento, no
  una decisión de bajar la barra. (Referencias: [backstage](https://github.com/backstage/backstage/blob/master/tsconfig.json),
  [directus/tsconfig](https://github.com/directus/tsconfig/blob/main/configs/base/tsconfig.json).)
- **ESM machine-enforced:** el repo es `"type": "module"`, así que `verbatimModuleSyntax: true`
  convierte la elección en invariante (los imports type-only pasan a ser error de compilación,
  no convención). (Referencia: [directus/tsconfig](https://github.com/directus/tsconfig/blob/main/configs/base/tsconfig.json).)
- **Typecheck como gate dedicado**, separado de build y de lint: existe un script cuyo comando
  es `tsc --noEmit` puro, y CI lo corre sin `continue-on-error`.
  (Referencia: [backstage](https://github.com/backstage/backstage/blob/master/package.json).)
- **Cero `any`; el tipo de lo desconocido es `unknown`.** `as` es apagar el compilador: en el
  borde IPC la respuesta se **valida** con un type guard real, no se castea.
- **Exhaustividad en uniones:** todo `switch` sobre discriminated unions cierra con
  `const _exhaustive: never = x`.
- **CI sin enmascaramiento de fallos:** cero `continue-on-error` y cero `|| true` en los
  workflows, y ningún comando que salga 0 cuando no hizo nada. Por eso el typecheck va por
  `npm run typecheck` y no por `npx tsc`: sin `typescript` instalado, `npx tsc` sale 0 sin
  typechequear una línea. (Referencias: CI de [backstage](https://github.com/backstage/backstage/blob/master/.github/workflows/ci.yml)
  y de [n8n](https://github.com/n8n-io/n8n/blob/master/.github/workflows/ci-pull-requests.yml).)
- **Un único gate reproducible:** `verify.sh` es la puerta, y dev y CI tienen que correr
  exactamente eso (mismos comandos y mismos flags). Ya es invocable desde cualquier shell con
  `npm run verify`; lo que falta es que `ci.yml` lo **invoque** en vez de reescribir los mismos
  comandos a mano en tres steps (más un `RUSTFLAGS` a nivel workflow que el script no tiene).
  Mientras eso siga así, la paridad local-CI no está garantizada por nada: es el gap abierto
  más caro de este bloque. (Referencias: el justfile de [spacedrive](https://github.com/spacedriveapp/spacedrive) y
  `just ready` de [oxc](https://github.com/oxc-project/oxc).)

### Documentación

Referencias públicas: [ripgrep](https://github.com/BurntSushi/ripgrep),
[architecture-decision-record](https://github.com/joelparkerhenderson/architecture-decision-record)
y [la guía de GitHub sobre READMEs](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-readmes).

- **El README nombra al proyecto igual que el manifest.** El `name` de `package.json`, el
  `[package] name` de `src-tauri/Cargo.toml`, el `productName` de `tauri.conf.json` y el
  título del README dicen lo mismo. Un rename se hace en los cuatro lados o no se hace.
- **No se linkea a archivos que no existen.** Todo link relativo resuelve en el árbol (gate:
  `links.yml` con lychee, ya vivo) y todo test citado entre backticks existe con ese nombre
  exacto en el árbol de tests. Citar un test inexistente o renombrado es peor que no citarlo:
  es exactamente lo que un revisor usa para NO volver a mirar el código.
- **No es un molde reciclado.** Nada de secciones de plantilla que no describan este repo:
  features fantasma, "coming soon", instrucciones de otro stack, badges de un pipeline que no
  existe. Si una sección no aplica, se borra.
- **Scope declarado por negación.** Hay una sección de límites y non-goals ("qué a propósito
  NO hace"), que es lo que distingue alcance deliberado de deriva (patrón ripgrep, "Why
  shouldn't I use ripgrep?").
- **El "por qué" visible vía ADR:** una decisión por documento, con contexto y consecuencias.
  Las decisiones diferidas (retención de backups e historial, merge por clave en
  `settings.json`) son las candidatas directas.
- **La doc de seguridad no puede afirmar lo que el código desmiente.** `docs/threat-model.md`
  distingue "cubierto con test real" de "gap real, no hipotético", y cada afirmación de
  cobertura queda atada al árbol de tests por un gate, no por buena voluntad.
