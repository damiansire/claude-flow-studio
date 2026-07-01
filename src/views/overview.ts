import { api } from "../lib/api";
import { stateHtml } from "../lib/render";

export async function renderOverview(container: HTMLElement) {
  container.innerHTML = stateHtml("cargando...");
  try {
    const [memories, skills, agents, tasks, commands, workflows, settings] = await Promise.all([
      api.listMemories(),
      api.listSkills(),
      api.listAgents(),
      api.listScheduledTasks(),
      api.listCommands(),
      api.listWorkflows(),
      api.readSettingsSummary(),
    ]);

    container.innerHTML = `
      <h2>Visión general</h2>
      <p class="lead">Estado en vivo de <code>~/.claude</code> — todo lo de acá se lee directo de tus archivos reales.</p>
      <div class="grid">
        <div class="card"><h3>🧩 Memoria <span class="count">(${memories.length})</span></h3><p>Entradas activas en tu memoria del chat global.</p></div>
        <div class="card"><h3>🛠️ Skills <span class="count">(${skills.length})</span></h3><p>Skills instaladas en <code>~/.claude/skills</code>.</p></div>
        <div class="card"><h3>🤖 Agentes <span class="count">(${agents.length})</span></h3><p>Agentes custom en <code>~/.claude/agents</code>.</p></div>
        <div class="card"><h3>⏱️ Tareas programadas <span class="count">(${tasks.length})</span></h3><p>En <code>~/.claude/scheduled-tasks</code>.</p></div>
        <div class="card"><h3>⚡ Comandos <span class="count">(${commands.length})</span></h3><p>Slash commands en <code>~/.claude/commands</code>.</p></div>
        <div class="card"><h3>🔀 Workflows <span class="count">(${workflows.length})</span></h3><p>Scripts en <code>~/.claude/workflows</code>.</p></div>
        <div class="card"><h3>⚙️ Config</h3><p>Modelo <code>${settings.model ?? "?"}</code> · tema <code>${settings.theme ?? "?"}</code> · ${settings.permissions_allow.length} permisos allowlist · ${settings.hooks_events.length} hooks · ${settings.enabled_plugins.length} plugins.</p></div>
      </div>
      <p class="lead">Todo lo que ves acá se puede editar desde la app (memorias, skills, comandos/workflows y config) con staging + revisión: nada se escribe directo a tus archivos reales sin que lo apliques vos.</p>
    `;
  } catch (err) {
    container.innerHTML = stateHtml(`No se pudo cargar: ${String(err)}`, true);
  }
}
