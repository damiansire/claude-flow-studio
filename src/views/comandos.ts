import { api, errorMessage, type SlashCommand, type Workflow } from "../lib/api";
import { openEditor } from "../lib/editor";
import { escapeHtml, stateHtml } from "../lib/render";

function commandCard(c: SlashCommand): string {
  const hint = c.argument_hint ? `<span class="tag">${escapeHtml(c.argument_hint)}</span>` : "";
  return `
    <div class="card" data-path="${escapeHtml(c.path)}" data-title="/${escapeHtml(c.name)}">
      <h3>⚡ /${escapeHtml(c.name)} ${hint}</h3>
      <p>${escapeHtml(c.description || "(sin descripción)")}</p>
    </div>
  `;
}

function workflowCard(w: Workflow): string {
  return `
    <div class="card" data-path="${escapeHtml(w.path)}" data-title="${escapeHtml(w.name)}">
      <h3>🔀 ${escapeHtml(w.name)}</h3>
      <p>${escapeHtml(w.description || "(sin descripción)")}</p>
    </div>
  `;
}

export async function renderComandos(container: HTMLElement) {
  container.innerHTML = stateHtml("cargando...");
  try {
    const [commands, workflows] = await Promise.all([api.listCommands(), api.listWorkflows()]);
    container.innerHTML = `
      <h2>Comandos &amp; Workflows</h2>
      <p class="lead">Automatizan trabajo repetible: slash commands en <code>~/.claude/commands</code> y scripts multi-agente en <code>~/.claude/workflows</code>.</p>
      <h3>Comandos <span class="count">(${commands.length})</span></h3>
      <div class="grid">${commands.map(commandCard).join("") || stateHtml("No hay comandos.")}</div>
      <h3>Workflows <span class="count">(${workflows.length})</span></h3>
      <div class="grid">${workflows.map(workflowCard).join("") || stateHtml("No hay workflows.")}</div>
    `;
    container.querySelectorAll<HTMLElement>(".card").forEach((card) => {
      card.addEventListener("click", () => openEditor(card.dataset.title!, card.dataset.path!));
    });
  } catch (err) {
    container.innerHTML = stateHtml(`No se pudo cargar: ${errorMessage(err)}`, true);
  }
}
