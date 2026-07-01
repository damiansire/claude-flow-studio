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

function closeEditor() {
  overlay.classList.add("hidden");
  lastFocused?.focus();
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
  currentPath = path;
  currentStagedId = null;
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
      textarea.value = await api.readFileContent(path);
    } catch (err) {
      textarea.value = "";
      setStatus(`No se pudo leer el archivo: ${errorMessage(err)}`, true);
    }
    return;
  }

  try {
    const [content, staged] = await Promise.all([api.readFileContent(path), api.listStaged()]);
    const pending = staged.find((s) => s.target_path === path);
    if (pending) {
      currentStagedId = pending.id;
      textarea.value = pending.draft_content;
      setStatus("Hay un borrador guardado sin aplicar para este archivo.");
      setStagedControls(true);
    } else {
      textarea.value = content;
    }
  } catch (err) {
    textarea.value = "";
    setStatus(`No se pudo leer el archivo: ${errorMessage(err)}`, true);
  }
}

async function onStage() {
  try {
    const staged = await api.stageChange(currentPath, textarea.value);
    currentStagedId = staged.id;
    setStatus("Borrador guardado. No se tocó el archivo real todavía.");
    setStagedControls(true);
    diffPre.classList.add("hidden");
  } catch (err) {
    setStatus(`No se pudo guardar el borrador: ${errorMessage(err)}`, true);
  }
}

async function onShowDiff() {
  if (!currentStagedId) return;
  try {
    const diff = await api.diffStaged(currentStagedId);
    diffPre.innerHTML = diff ? renderDiffHtml(diff) : "(sin diferencias con el archivo real actual)";
    diffPre.classList.remove("hidden");
  } catch (err) {
    setStatus(`No se pudo calcular el diff: ${errorMessage(err)}`, true);
  }
}

async function onApply() {
  if (!currentStagedId) return;
  try {
    await api.applyStaged(currentStagedId);
    setStatus("Aplicado — el archivo real ya se actualizó (queda backup en el historial).");
    currentStagedId = null;
    setStagedControls(false);
    diffPre.classList.add("hidden");
    notifyMutated();
  } catch (err) {
    setStatus(`No se pudo aplicar: ${errorMessage(err)}`, true);
  }
}

async function onDiscard() {
  if (!currentStagedId) return;
  try {
    await api.discardStaged(currentStagedId);
    setStatus("Borrador descartado. El archivo real no se tocó.");
    currentStagedId = null;
    setStagedControls(false);
    diffPre.classList.add("hidden");
    notifyMutated();
  } catch (err) {
    setStatus(`No se pudo descartar: ${errorMessage(err)}`, true);
  }
}
