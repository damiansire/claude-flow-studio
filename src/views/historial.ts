import { api, type AppliedChange } from "../lib/api";
import { emptyHtml, escapeHtml, stateHtml } from "../lib/render";

function entryHtml(a: AppliedChange): string {
  return `
    <div class="card">
      <h3>📄 ${escapeHtml(a.target_path)}</h3>
      <p>Aplicado: ${escapeHtml(a.applied_at)}</p>
      <p class="mono">backup: ${escapeHtml(a.backup_path)}</p>
      <button class="btn danger" data-revert="${escapeHtml(a.id)}">Revertir a este backup</button>
    </div>
  `;
}

export async function renderHistorial(container: HTMLElement) {
  container.innerHTML = stateHtml("cargando...");
  try {
    const history = await api.listHistory();
    container.innerHTML = `
      <h2>Historial</h2>
      <p class="lead">Cambios ya aplicados a archivos reales de <code>~/.claude</code>, más reciente primero. Cada aplicación guardó un backup — revertir restaura ese backup (y registra un nuevo backup del estado actual, por si hace falta deshacer el revert).</p>
      <div class="grid">${history.map(entryHtml).join("") || emptyHtml("Todavía no aplicaste ningún cambio.", "Editá algo en Memoria o Skills y aplicalo para verlo acá.")}</div>
    `;
    container.querySelectorAll<HTMLButtonElement>("[data-revert]").forEach((btn) => {
      btn.addEventListener("click", async () => {
        btn.disabled = true;
        btn.textContent = "Revirtiendo...";
        try {
          await api.revertApplied(btn.dataset.revert!);
          await renderHistorial(container);
        } catch (err) {
          btn.disabled = false;
          btn.textContent = `Error: ${String(err)}`;
        }
      });
    });
  } catch (err) {
    container.innerHTML = stateHtml(`No se pudo cargar: ${String(err)}`, true);
  }
}
