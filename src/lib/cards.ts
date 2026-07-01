import { openEditor } from "./editor";
import { escapeHtml } from "./render";

/** Datos de una card clickeable que abre el editor. `readOnly` para contenido
 *  fuera del alcance editable (agentes, tareas programadas). */
export interface CardData {
  icon: string;
  title: string;
  description: string;
  path: string;
  tag?: string | null;
  readOnly?: boolean;
}

/** HTML de una card. Serializa el estado que `wireCards` lee de vuelta
 *  (data-path/data-title/data-readonly), así el markup y el wiring no se
 *  desincronizan entre vistas. */
export function cardHtml(c: CardData): string {
  const tag = c.tag ? ` <span class="tag a">${escapeHtml(c.tag)}</span>` : "";
  const ro = c.readOnly ? ` data-readonly="1"` : "";
  return `
    <div class="card" data-path="${escapeHtml(c.path)}" data-title="${escapeHtml(c.title)}"${ro}>
      <h3>${c.icon} ${escapeHtml(c.title)}${tag}</h3>
      <p>${escapeHtml(c.description || "(sin descripción)")}</p>
    </div>
  `;
}

/** Cablea el click de cada `.card[data-path]` dentro de `container` para abrir
 *  el editor. Un solo lugar con el acoplamiento data-* → openEditor, en vez de
 *  repetir el loop con `!` en cada vista. */
export function wireCards(container: HTMLElement) {
  container.querySelectorAll<HTMLElement>(".card[data-path][data-title]").forEach((card) => {
    card.addEventListener("click", () =>
      openEditor(card.dataset.title!, card.dataset.path!, { readOnly: card.dataset.readonly === "1" }),
    );
  });
}
