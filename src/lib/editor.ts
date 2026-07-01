import { api } from "./api";

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

export function mountEditorModal(root: HTMLElement) {
  overlay = document.createElement("div");
  overlay.className = "overlay hidden";
  overlay.innerHTML = `
    <div class="modal">
      <header>
        <h3 id="editor-title"></h3>
        <button id="editor-close" aria-label="Cerrar">&times;</button>
      </header>
      <div class="body">
        <textarea id="editor-textarea" class="editor-textarea" spellcheck="false"></textarea>
        <pre id="editor-diff" class="hidden"></pre>
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

  stageBtn.addEventListener("click", onStage);
  diffBtn.addEventListener("click", onShowDiff);
  applyBtn.addEventListener("click", onApply);
  discardBtn.addEventListener("click", onDiscard);
}

function closeEditor() {
  overlay.classList.add("hidden");
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
  titleEl.textContent = opts.readOnly ? `${title} (solo lectura)` : title;
  textarea.value = "cargando...";
  textarea.readOnly = Boolean(opts.readOnly);
  stageBtn.classList.toggle("hidden", Boolean(opts.readOnly));
  diffPre.classList.add("hidden");
  setStatus("");
  setStagedControls(false);
  overlay.classList.remove("hidden");

  if (opts.readOnly) {
    try {
      textarea.value = await api.readFileContent(path);
    } catch (err) {
      textarea.value = "";
      setStatus(`No se pudo leer el archivo: ${String(err)}`, true);
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
    setStatus(`No se pudo leer el archivo: ${String(err)}`, true);
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
    setStatus(`No se pudo guardar el borrador: ${String(err)}`, true);
  }
}

async function onShowDiff() {
  if (!currentStagedId) return;
  try {
    const diff = await api.diffStaged(currentStagedId);
    diffPre.textContent = diff || "(sin diferencias con el archivo real actual)";
    diffPre.classList.remove("hidden");
  } catch (err) {
    setStatus(`No se pudo calcular el diff: ${String(err)}`, true);
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
  } catch (err) {
    setStatus(`No se pudo aplicar: ${String(err)}`, true);
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
  } catch (err) {
    setStatus(`No se pudo descartar: ${String(err)}`, true);
  }
}
