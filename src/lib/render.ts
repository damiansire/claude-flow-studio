import { errorMessage } from "./api";

export function escapeHtml(s: string): string {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

export function stateHtml(text: string, isError = false): string {
  return `<p class="state${isError ? " error" : ""}">${escapeHtml(text)}</p>`;
}

/** Estado vacío con más peso visual que `stateHtml` — para grillas sin resultados. */
export function emptyHtml(text: string, hint?: string): string {
  return `<div class="empty">${escapeHtml(text)}${hint ? `<span class="hint">${escapeHtml(hint)}</span>` : ""}</div>`;
}

/** Ciclo de vida de una vista: muestra "cargando...", corre `load`, pinta con
 *  `render`, y recién ahí llama a `onMounted` para el wiring (listeners sobre el
 *  DOM ya renderizado). Si algo falla, muestra el error en vez de tumbar la
 *  vista. Centraliza el bloque loading/try-catch que antes cada vista copiaba
 *  a mano — `onMounted` es lo que faltaba para que dejaran de esquivarlo. */
export async function withView<T>(
  container: HTMLElement,
  load: () => Promise<T>,
  render: (data: T) => string,
  onMounted?: (data: T, container: HTMLElement) => void,
) {
  container.innerHTML = stateHtml("cargando...");
  try {
    const data = await load();
    container.innerHTML = render(data);
    onMounted?.(data, container);
  } catch (err) {
    container.innerHTML = stateHtml(`No se pudo cargar: ${errorMessage(err)}`, true);
  }
}
