import { api, type Skill } from "../lib/api";
import { cardHtml, wireCards } from "../lib/cards";
import { emptyHtml, stateHtml, withView } from "../lib/render";

function skillCard(s: Skill): string {
  return cardHtml({ icon: "🛠️", title: s.name, description: s.description, path: s.path });
}

export async function renderSkills(container: HTMLElement) {
  await withView(
    container,
    () => api.listSkills(),
    (skills) => `
      <h2>Skills <span class="count">(${skills.length})</span></h2>
      <p class="lead">Conocimiento de dominio en <code>~/.claude/skills</code>, se activa cuando el tema calza.</p>
      <input class="search" id="skill-search" placeholder="Filtrar skills..." />
      <div class="grid" id="skill-grid">${skills.map(skillCard).join("") || stateHtml("No hay skills instaladas.")}</div>
      <div id="skill-no-results" class="hidden">${emptyHtml("Ninguna skill coincide con tu búsqueda.", "Probá con otra palabra clave.")}</div>
    `,
    (_data, root) => {
      wireCards(root);
      const grid = root.querySelector<HTMLElement>("#skill-grid")!;
      const noResults = root.querySelector<HTMLElement>("#skill-no-results")!;
      root.querySelector<HTMLInputElement>("#skill-search")!.addEventListener("input", (e) => {
        const q = (e.target as HTMLInputElement).value.toLowerCase();
        let visible = 0;
        grid.querySelectorAll<HTMLElement>(".card").forEach((card) => {
          const matches = card.textContent!.toLowerCase().includes(q);
          card.style.display = matches ? "" : "none";
          if (matches) visible++;
        });
        noResults.classList.toggle("hidden", visible > 0);
      });
    },
  );
}
