import { api } from "../lib/api";
import { emptyHtml, escapeHtml, withView } from "../lib/render";

export async function renderReglas(container: HTMLElement) {
  await withView(
    container,
    () => api.readClaudeMd(),
    (claudeMd) => `
      <h2>Reglas globales</h2>
      <p class="lead"><code>~/.claude/CLAUDE.md</code> — aplican a todos tus proyectos, salvo que el CLAUDE.md de un repo puntual diga otra cosa.</p>
      ${
        claudeMd.trim()
          ? `<div class="card"><pre>${escapeHtml(claudeMd)}</pre></div>`
          : emptyHtml(
              "Todavía no tenés reglas globales",
              "Creá ~/.claude/CLAUDE.md para que apliquen a todos tus proyectos.",
            )
      }
    `,
  );
}
