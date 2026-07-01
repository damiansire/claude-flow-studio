import { api, type Skill } from "../lib/api";
import { openEditor } from "../lib/editor";
import { escapeHtml, stateHtml } from "../lib/render";

function taskCard(t: Skill): string {
  return `
    <div class="card" data-path="${escapeHtml(t.path)}" data-title="${escapeHtml(t.name)}">
      <h3>⏰ ${escapeHtml(t.name)}</h3>
      <p>${escapeHtml(t.description || "(sin descripción)")}</p>
    </div>
  `;
}

function tagList(items: string[]): string {
  return items.length
    ? items.map((i) => `<span class="tag">${escapeHtml(i)}</span>`).join(" ")
    : '<span class="state">(ninguno)</span>';
}

export async function renderAutomatizacion(container: HTMLElement) {
  container.innerHTML = stateHtml("cargando...");
  try {
    const [settings, tasks] = await Promise.all([api.readSettingsSummary(), api.listScheduledTasks()]);
    container.innerHTML = `
      <h2>Automatización</h2>
      <p class="lead">Lo que corre solo, leído en vivo de <code>~/.claude/settings.json</code> y <code>~/.claude/scheduled-tasks</code>.</p>
      <div class="grid">
        <div class="card" id="config-card"><h3>⚙️ Config <span class="tag a">editable</span></h3><p>Modelo <code>${escapeHtml(settings.model ?? "?")}</code> · tema <code>${escapeHtml(settings.theme ?? "?")}</code> — click para editar <code>settings.json</code>.</p></div>
        <div class="card"><h3>🪝 Hooks</h3><p>${tagList(settings.hooks_events)}</p></div>
        <div class="card"><h3>🔌 Plugins habilitados</h3><p>${tagList(settings.enabled_plugins)}</p></div>
      </div>
      <h3>Permisos allowlist <span class="count">(${settings.permissions_allow.length})</span></h3>
      <p class="lead">${tagList(settings.permissions_allow)}</p>
      <h3>Tareas programadas <span class="count">(${tasks.length})</span></h3>
      <p class="lead">Solo lectura desde acá.</p>
      <div class="grid">${tasks.map(taskCard).join("") || stateHtml("No hay tareas programadas.")}</div>
    `;
    container.querySelector<HTMLElement>("#config-card")!.addEventListener("click", async () => {
      const path = await api.settingsPath();
      openEditor("settings.json", path);
    });
    container.querySelectorAll<HTMLElement>(".card[data-path]").forEach((card) => {
      card.addEventListener("click", () => openEditor(card.dataset.title!, card.dataset.path!, { readOnly: true }));
    });
  } catch (err) {
    container.innerHTML = stateHtml(`No se pudo cargar: ${String(err)}`, true);
  }
}
