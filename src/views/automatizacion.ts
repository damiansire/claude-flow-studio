import { api, type Skill } from "../lib/api";
import { cardHtml, wireCards } from "../lib/cards";
import { openEditor } from "../lib/editor";
import { escapeHtml, stateHtml, withView } from "../lib/render";

function taskCard(t: Skill): string {
  // Tareas programadas: solo lectura.
  return cardHtml({ icon: "⏰", title: t.name, description: t.description, path: t.path, readOnly: true });
}

function tagList(items: string[]): string {
  return items.length
    ? items.map((i) => `<span class="tag">${escapeHtml(i)}</span>`).join(" ")
    : '<span class="state">(ninguno)</span>';
}

export async function renderAutomatizacion(container: HTMLElement) {
  await withView(
    container,
    () => Promise.all([api.readSettingsSummary(), api.listScheduledTasks()]),
    ([settings, tasks]) => `
      <h2>Automatización</h2>
      <p class="lead">Lo que corre solo, leído en vivo de <code>~/.claude/settings.json</code> y <code>~/.claude/scheduled-tasks</code>.</p>
      <div class="grid">
        <div class="card" id="config-card"><h3>⚙️ Config <span class="tag a">editable</span></h3><p>Modelo <code>${escapeHtml(settings.model ?? "?")}</code> · tema <code>${escapeHtml(settings.theme ?? "?")}</code> — click para editar <code>settings.json</code>.</p></div>
        <div class="card inert"><h3>🪝 Hooks</h3><p>${tagList(settings.hooks_events)}</p></div>
        <div class="card inert"><h3>🔌 Plugins habilitados</h3><p>${tagList(settings.enabled_plugins)}</p></div>
      </div>
      <h3>Permisos allowlist <span class="count">(${settings.permissions_allow.length})</span></h3>
      <p class="lead">${tagList(settings.permissions_allow)}</p>
      <h3>Tareas programadas <span class="count">(${tasks.length})</span></h3>
      <p class="lead">Solo lectura desde acá.</p>
      <div class="grid">${tasks.map(taskCard).join("") || stateHtml("No hay tareas programadas.")}</div>
    `,
    (_data, root) => {
      // Config abre settings.json por un camino propio (necesita settingsPath()),
      // así que no pasa por wireCards — pero igual debe ser operable por teclado.
      const configCard = root.querySelector<HTMLElement>("#config-card")!;
      configCard.setAttribute("role", "button");
      configCard.tabIndex = 0;
      const openConfig = async () => openEditor("settings.json", await api.settingsPath());
      configCard.addEventListener("click", openConfig);
      configCard.addEventListener("keydown", (e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          void openConfig();
        }
      });
      wireCards(root);
    },
  );
}
