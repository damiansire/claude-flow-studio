import { api } from "../lib/api";
import { escapeHtml, withState } from "../lib/render";

export async function renderReglas(container: HTMLElement) {
  await withState(
    container,
    () => api.readClaudeMd(),
    (claudeMd) => `
      <h2>Reglas globales</h2>
      <p class="lead"><code>~/.claude/CLAUDE.md</code> — aplican a todos tus proyectos, salvo que el CLAUDE.md de un repo puntual diga otra cosa.</p>
      <div class="card"><pre>${escapeHtml(claudeMd)}</pre></div>
    `,
  );
}
