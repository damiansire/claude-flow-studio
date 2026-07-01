import { api, type Skill } from "../lib/api";
import { openEditor } from "../lib/editor";
import { emptyHtml, escapeHtml, stateHtml } from "../lib/render";

function cardHtml(s: Skill): string {
  return `
    <div class="card" data-path="${escapeHtml(s.path)}" data-title="${escapeHtml(s.name)}">
      <h3>🛠️ ${escapeHtml(s.name)}</h3>
      <p>${escapeHtml(s.description || "(sin descripción)")}</p>
    </div>
  `;
}

export async function renderSkills(container: HTMLElement) {
  container.innerHTML = stateHtml("cargando...");
  try {
    const skills = await api.listSkills();
    container.innerHTML = `
      <h2>Skills <span class="count">(${skills.length})</span></h2>
      <p class="lead">Conocimiento de dominio en <code>~/.claude/skills</code>, se activa cuando el tema calza.</p>
      <input class="search" id="skill-search" placeholder="Filtrar skills..." />
      <div class="grid" id="skill-grid">${skills.map(cardHtml).join("") || stateHtml("No hay skills instaladas.")}</div>
      <div id="skill-no-results" class="hidden">${emptyHtml("Ninguna skill coincide con tu búsqueda.", "Probá con otra palabra clave.")}</div>
    `;
    const grid = container.querySelector<HTMLElement>("#skill-grid")!;
    const noResults = container.querySelector<HTMLElement>("#skill-no-results")!;
    grid.querySelectorAll<HTMLElement>(".card").forEach((card) => {
      card.addEventListener("click", () => openEditor(card.dataset.title!, card.dataset.path!));
    });
    container.querySelector<HTMLInputElement>("#skill-search")!.addEventListener("input", (e) => {
      const q = (e.target as HTMLInputElement).value.toLowerCase();
      let visible = 0;
      grid.querySelectorAll<HTMLElement>(".card").forEach((card) => {
        const matches = card.textContent!.toLowerCase().includes(q);
        card.style.display = matches ? "" : "none";
        if (matches) visible++;
      });
      noResults.classList.toggle("hidden", visible > 0);
    });
  } catch (err) {
    container.innerHTML = stateHtml(`No se pudo cargar: ${String(err)}`, true);
  }
}
