import { api, type MemoryEntry } from "../lib/api";
import { openEditor } from "../lib/editor";
import { escapeHtml, stateHtml } from "../lib/render";

function cardHtml(m: MemoryEntry): string {
  const tag = m.mem_type ? `<span class="tag a">${escapeHtml(m.mem_type)}</span>` : "";
  return `
    <div class="card" data-path="${escapeHtml(m.path)}" data-title="${escapeHtml(m.name)}">
      <h3>🧩 ${escapeHtml(m.name)} ${tag}</h3>
      <p>${escapeHtml(m.description || "(sin descripción)")}</p>
    </div>
  `;
}

export async function renderMemoria(container: HTMLElement) {
  container.innerHTML = stateHtml("cargando...");
  try {
    const memories = await api.listMemories();
    container.innerHTML = `
      <h2>Memoria <span class="count">(${memories.length})</span></h2>
      <p class="lead">Entradas del "chat global" (la sesión que corre desde tu home dir) en <code>~/.claude/projects/&lt;tu-home-sanitizado&gt;/memory</code> — contexto que persiste entre conversaciones.</p>
      <div class="grid">${memories.map(cardHtml).join("") || stateHtml("No hay memorias todavía.")}</div>
    `;
    container.querySelectorAll<HTMLElement>(".card").forEach((card) => {
      card.addEventListener("click", () => openEditor(card.dataset.title!, card.dataset.path!));
    });
  } catch (err) {
    container.innerHTML = stateHtml(`No se pudo cargar: ${String(err)}`, true);
  }
}
