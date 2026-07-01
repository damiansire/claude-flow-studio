import { api, type SlashCommand, type Workflow } from "../lib/api";
import { cardHtml, wireCards } from "../lib/cards";
import { stateHtml, withView } from "../lib/render";

function commandCard(c: SlashCommand): string {
  return cardHtml({
    icon: "⚡",
    title: `/${c.name}`,
    description: c.description ?? "",
    path: c.path,
    tag: c.argument_hint,
  });
}

function workflowCard(w: Workflow): string {
  return cardHtml({ icon: "🔀", title: w.name, description: w.description ?? "", path: w.path });
}

export async function renderComandos(container: HTMLElement) {
  await withView(
    container,
    () => Promise.all([api.listCommands(), api.listWorkflows()]),
    ([commands, workflows]) => `
      <h2>Comandos &amp; Workflows</h2>
      <p class="lead">Automatizan trabajo repetible: slash commands en <code>~/.claude/commands</code> y scripts multi-agente en <code>~/.claude/workflows</code>.</p>
      <h3>Comandos <span class="count">(${commands.length})</span></h3>
      <div class="grid">${commands.map(commandCard).join("") || stateHtml("No hay comandos.")}</div>
      <h3>Workflows <span class="count">(${workflows.length})</span></h3>
      <div class="grid">${workflows.map(workflowCard).join("") || stateHtml("No hay workflows.")}</div>
    `,
    (_data, root) => wireCards(root),
  );
}
