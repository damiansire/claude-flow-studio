import { errorMessage } from "./api";

/** Escapa una string para interpolarla en HTML, tanto en contexto de TEXTO como
 *  dentro de un valor de atributo entre comillas.
 *
 *  La implementación anterior (`div.textContent = s; return div.innerHTML`) solo
 *  escapaba `&`, `<` y `>`: por spec, la serialización de un nodo de texto deja
 *  la comilla doble intacta. Como la salida se usa dentro de atributos
 *  (`data-path="${escapeHtml(...)}"` en cards.ts, agentes.ts e historial.ts) y
 *  esos valores vienen de `~/.claude` (el `name:` del frontmatter de una skill
 *  instalada, un path del filesystem), un `name: x" data-readonly="0` cerraba el
 *  atributo e inyectaba atributos propios: por la regla first-wins del parser
 *  ganaba sobre el `data-readonly="1"` literal y convertía en editable una card
 *  declarada solo-lectura. Se reemplazan los cinco caracteres a mano. */
export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
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
