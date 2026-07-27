import { api, errorMessage } from "./api";
import { escapeHtml } from "./render";

function diffLineClass(line: string): string | null {
  // Cabecera de hunk.
  if (line.startsWith("@@")) return "diff-hunk";
  // Cabeceras de archivo del unified diff: llevan un espacio tras el marcador
  // ("--- a/x", "+++ b/x"). Una línea de CONTENIDO eliminada/agregada nunca
  // tiene ese espacio inmediato — por eso el frontmatter "---" borrado (que se
  // emite como "----") cae a diff-del (rojo) y no a hunk (gris) como antes.
  if (line.startsWith("--- ") || line.startsWith("+++ ")) return "diff-hunk";
  if (line.startsWith("+")) return "diff-add";
  if (line.startsWith("-")) return "diff-del";
  return null;
}

function renderDiffHtml(diff: string): string {
  return diff
    .split("\n")
    .map((line) => {
      const cls = diffLineClass(line);
      return cls ? `<span class="${cls}">${escapeHtml(line)}</span>` : escapeHtml(line);
    })
    .join("\n");
}

let overlay: HTMLDivElement;
let titleEl: HTMLHeadingElement;
let textarea: HTMLTextAreaElement;
let statusEl: HTMLDivElement;
let diffPre: HTMLPreElement;
let stageBtn: HTMLButtonElement;
let diffBtn: HTMLButtonElement;
let applyBtn: HTMLButtonElement;
let discardBtn: HTMLButtonElement;

let currentPath = "";
let currentStagedId: string | null = null;
/** El elemento que tenía el foco al abrir el modal, para devolvérselo al cerrar. */
let lastFocused: HTMLElement | null = null;
/** Lo que se cargó en el textarea (archivo real o borrador pendiente), para
 *  saber si el usuario tipeó algo que todavía no guardó. */
let loadedContent = "";
/** Generación de apertura: `openEditor` es async y dos clicks seguidos (o Enter
 *  en una card y click en otra) hacían que la respuesta más lenta pisara la
 *  última apertura — el modal quedaba con el título de B y el texto y el
 *  `currentStagedId` de A, así que "Aplicar" escribía el borrador equivocado. */
let openGeneration = 0;
/** Hay una mutación en vuelo (stage/apply/discard/diff). */
let busy = false;

/** Notifica que se tocó un archivo real de ~/.claude, para que la vista activa
 *  se refresque (contadores, listas, historial). Lo escucha `main.ts`. */
function notifyMutated() {
  document.dispatchEvent(new CustomEvent("cf:mutated"));
}

export function mountEditorModal(root: HTMLElement) {
  overlay = document.createElement("div");
  overlay.className = "overlay hidden";
  overlay.innerHTML = `
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="editor-title">
      <header>
        <h3 id="editor-title"></h3>
        <button id="editor-close" aria-label="Cerrar">&times;</button>
      </header>
      <div class="body">
        <textarea id="editor-textarea" class="editor-textarea" spellcheck="false"></textarea>
        <pre id="editor-diff" class="hidden"></pre>
      </div>
      <div class="modal-footer">
        <div id="editor-status" class="editor-status"></div>
        <div class="editor-actions">
          <button id="editor-stage" class="btn primary">Guardar borrador</button>
          <button id="editor-diffbtn" class="btn" disabled>Ver diff</button>
          <button id="editor-apply" class="btn primary" disabled>Aplicar</button>
          <button id="editor-discard" class="btn danger" disabled>Descartar</button>
        </div>
      </div>
    </div>
  `;
  root.appendChild(overlay);

  titleEl = overlay.querySelector("#editor-title")!;
  textarea = overlay.querySelector("#editor-textarea")!;
  statusEl = overlay.querySelector("#editor-status")!;
  diffPre = overlay.querySelector("#editor-diff")!;
  stageBtn = overlay.querySelector("#editor-stage")!;
  diffBtn = overlay.querySelector("#editor-diffbtn")!;
  applyBtn = overlay.querySelector("#editor-apply")!;
  discardBtn = overlay.querySelector("#editor-discard")!;

  overlay.querySelector("#editor-close")!.addEventListener("click", closeEditor);
  overlay.addEventListener("click", (e) => {
    if (e.target === overlay) closeEditor();
  });
  overlay.addEventListener("keydown", onKeydown);

  stageBtn.addEventListener("click", onStage);
  diffBtn.addEventListener("click", onShowDiff);
  applyBtn.addEventListener("click", onApply);
  discardBtn.addEventListener("click", onDiscard);
}

/** Escape cierra; Tab queda atrapado dentro del modal (focus trap) para que el
 *  teclado no se escape a los tabs de atrás mientras el diálogo está abierto. */
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    closeEditor();
    return;
  }
  if (e.key !== "Tab") return;
  const focusables = Array.from(
    overlay.querySelectorAll<HTMLElement>("button, textarea, [href], [tabindex]:not([tabindex='-1'])"),
  ).filter((el) => !el.hasAttribute("disabled") && el.offsetParent !== null);
  if (focusables.length === 0) return;
  const first = focusables[0];
  const last = focusables[focusables.length - 1];
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

/** `true` si hay texto tipeado que no se guardó como borrador. */
function hasUnsavedEdits(): boolean {
  return !textarea.readOnly && textarea.value !== loadedContent;
}

function closeEditor() {
  // Escape o click en el overlay cerraban tirando lo tipeado sin decir nada.
  // Guardar el borrador es no destructivo (no toca el archivo real), pero
  // hacerlo solo hace ruido si el usuario quería descartar: se pregunta.
  if (
    hasUnsavedEdits() &&
    !window.confirm("Tenés cambios sin guardar como borrador. ¿Cerrar y perderlos?")
  ) {
    return;
  }
  overlay.classList.add("hidden");
  // Devolver el foco al abridor. Si aplicar/descartar re-renderizó la vista, la
  // card original quedó huérfana (isConnected=false); en ese caso caemos al
  // panel activo en vez de dejar el foco en <body>.
  if (lastFocused?.isConnected) {
    lastFocused.focus();
  } else {
    document.querySelector<HTMLElement>("#section-active")?.focus();
  }
  lastFocused = null;
}

function setStatus(text: string, isError = false) {
  statusEl.textContent = text;
  statusEl.classList.toggle("error", isError);
}

function setStagedControls(hasStagedChange: boolean) {
  diffBtn.disabled = !hasStagedChange;
  applyBtn.disabled = !hasStagedChange;
  discardBtn.disabled = !hasStagedChange;
}

/** `readOnly`: para contenido fuera del alcance editable de esta app (agentes,
 * tareas programadas) — mismo modal, pero sin guardar/aplicar/descartar. */
export async function openEditor(title: string, path: string, opts: { readOnly?: boolean } = {}) {
  const gen = ++openGeneration;
  currentPath = path;
  currentStagedId = null;
  loadedContent = "";
  lastFocused = document.activeElement as HTMLElement | null;
  titleEl.textContent = opts.readOnly ? `${title} (solo lectura)` : title;
  textarea.value = "cargando...";
  textarea.readOnly = Boolean(opts.readOnly);
  stageBtn.classList.toggle("hidden", Boolean(opts.readOnly));
  diffPre.classList.add("hidden");
  setStatus("");
  setStagedControls(false);
  overlay.classList.remove("hidden");
  textarea.focus();

  if (opts.readOnly) {
    try {
      const content = await api.readFileContent(path);
      if (gen !== openGeneration) return;
      textarea.value = content;
    } catch (err) {
      if (gen !== openGeneration) return;
      textarea.value = "";
      setStatus(`No se pudo leer el archivo: ${errorMessage(err)}`, true);
    }
    return;
  }

  try {
    const [content, staged] = await Promise.all([api.readFileContent(path), api.listStaged()]);
    // Otra apertura ganó la carrera: descartar esta respuesta entera. Escribir
    // el textarea acá dejaría el modal mostrando el título de un archivo con el
    // contenido y el borrador de otro, y "Aplicar" escribiría el equivocado.
    if (gen !== openGeneration) return;
    const pending = staged.find((s) => s.target_path === path);
    if (pending) {
      currentStagedId = pending.id;
      textarea.value = pending.draft_content;
      setStatus("Hay un borrador guardado sin aplicar para este archivo.");
      setStagedControls(true);
    } else {
      textarea.value = content;
    }
    loadedContent = textarea.value;
  } catch (err) {
    if (gen !== openGeneration) return;
    textarea.value = "";
    setStatus(`No se pudo leer el archivo: ${errorMessage(err)}`, true);
  }
}

/** Corre una mutación con los controles bloqueados y guard de reentrada.
 *
 *  Sin esto, un doble click en "Aplicar" disparaba dos `apply_staged` con el
 *  mismo id: el segundo hacía un backup del archivo YA modificado y duplicaba la
 *  entrada de historial, o sea el "revertir" de esa entrada restauraba el estado
 *  nuevo. `historial.ts` ya hacía este bloqueo; el modal no. */
async function withBusy(fn: () => Promise<void>) {
  if (busy) return;
  busy = true;
  stageBtn.disabled = true;
  setStagedControls(false);
  try {
    await fn();
  } finally {
    busy = false;
    stageBtn.disabled = false;
    // Restaurar según el estado real, no según una foto previa: `fn` pudo
    // crear o consumir el borrador.
    setStagedControls(currentStagedId !== null);
  }
}

function onStage() {
  return withBusy(async () => {
    try {
      const staged = await api.stageChange(currentPath, textarea.value);
      currentStagedId = staged.id;
      loadedContent = textarea.value;
      setStatus("Borrador guardado. No se tocó el archivo real todavía.");
      diffPre.classList.add("hidden");
    } catch (err) {
      setStatus(`No se pudo guardar el borrador: ${errorMessage(err)}`, true);
    }
  });
}

function onShowDiff() {
  const id = currentStagedId;
  if (!id) return Promise.resolve();
  return withBusy(async () => {
    try {
      const diff = await api.diffStaged(id);
      diffPre.innerHTML = diff
        ? renderDiffHtml(diff)
        : "(sin diferencias con el archivo real actual)";
      diffPre.classList.remove("hidden");
    } catch (err) {
      setStatus(`No se pudo calcular el diff: ${errorMessage(err)}`, true);
    }
  });
}

function onApply() {
  const id = currentStagedId;
  if (!id) return Promise.resolve();
  return withBusy(async () => {
    try {
      await api.applyStaged(id);
      setStatus("Aplicado — el archivo real ya se actualizó (queda backup en el historial).");
      currentStagedId = null;
      loadedContent = textarea.value;
      diffPre.classList.add("hidden");
      // El botón que tenía el foco se acaba de deshabilitar; sin esto el foco cae
      // al <body> (fuera del overlay) y Escape/focus-trap dejan de funcionar.
      textarea.focus();
      notifyMutated();
    } catch (err) {
      setStatus(`No se pudo aplicar: ${errorMessage(err)}`, true);
    }
  });
}

function onDiscard() {
  const id = currentStagedId;
  if (!id) return Promise.resolve();
  return withBusy(async () => {
    try {
      await api.discardStaged(id);
      setStatus("Borrador descartado. El archivo real no se tocó.");
      currentStagedId = null;
      loadedContent = textarea.value;
      diffPre.classList.add("hidden");
      // Ver nota en onApply: reenfocar para no perder el foco fuera del modal.
      textarea.focus();
      notifyMutated();
    } catch (err) {
      setStatus(`No se pudo descartar: ${errorMessage(err)}`, true);
    }
  });
}
