import { api, type AgentDef } from "../lib/api";
import { wireCards } from "../lib/cards";
import { escapeHtml, stateHtml, withView } from "../lib/render";

// Card custom (lleva la línea extra de tools), pero emite el mismo contrato
// data-* que lee `wireCards`, con data-readonly: agentes son solo lectura.
function agentCard(a: AgentDef): string {
  const tools = a.tools ? `<p><span class="tag">${escapeHtml(a.tools)}</span></p>` : "";
  return `
    <div class="card" data-path="${escapeHtml(a.path)}" data-title="${escapeHtml(a.name)}" data-readonly="1">
      <h3>🤖 ${escapeHtml(a.name)}</h3>
      <p>${escapeHtml(a.description || "(sin descripción)")}</p>
      ${tools}
    </div>
  `;
}

export async function renderAgentes(container: HTMLElement) {
  await withView(
    container,
    () => api.listAgents(),
    (agents) => `
      <h2>Agentes</h2>
      <p class="lead">Subagentes custom en <code>~/.claude/agents</code> — delegan trabajo especializado sin cargar tu contexto principal.</p>
      <div class="grid">${agents.map(agentCard).join("") || stateHtml("No hay agentes custom.")}</div>
      <h3>Agentes built-in de Claude Code</h3>
      <p class="lead">Vienen con el CLI, no son archivos en <code>~/.claude</code> — informativos, no editables desde acá.</p>
      <table>
        <thead><tr><th>Agente</th><th>Para qué</th></tr></thead>
        <tbody>
          <tr><td>Explore</td><td>Búsqueda read-only rápida por patrón/símbolo en el código.</td></tr>
          <tr><td>Plan</td><td>Diseña planes de implementación paso a paso.</td></tr>
          <tr><td>general-purpose</td><td>Investigación compleja y tareas multi-paso.</td></tr>
          <tr><td>claude-code-guide</td><td>Preguntas sobre Claude Code, Agent SDK y API de Claude.</td></tr>
          <tr><td>statusline-setup</td><td>Configura la status line del CLI.</td></tr>
        </tbody>
      </table>
    `,
    (_data, root) => wireCards(root),
  );
}
