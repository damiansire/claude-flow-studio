import { api, type MemoryEntry } from "../lib/api";
import { cardHtml, wireCards } from "../lib/cards";
import { stateHtml, withView } from "../lib/render";

function memCard(m: MemoryEntry): string {
  return cardHtml({ icon: "🧩", title: m.name, description: m.description, path: m.path, tag: m.mem_type });
}

export async function renderMemoria(container: HTMLElement) {
  await withView(
    container,
    () => api.listMemories(),
    (memories) => `
      <h2>Memoria <span class="count">(${memories.length})</span></h2>
      <p class="lead">Entradas del "chat global" (la sesión que corre desde tu home dir) en <code>~/.claude/projects/&lt;tu-home-sanitizado&gt;/memory</code> — contexto que persiste entre conversaciones.</p>
      <div class="grid">${memories.map(memCard).join("") || stateHtml("No hay memorias todavía.")}</div>
    `,
    (_data, root) => wireCards(root),
  );
}
