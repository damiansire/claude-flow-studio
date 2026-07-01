export function escapeHtml(s: string): string {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

export function stateHtml(text: string, isError = false): string {
  return `<p class="state${isError ? " error" : ""}">${escapeHtml(text)}</p>`;
}

/** Corre `load`, y si falla muestra el error en vez de tumbar toda la vista. */
export async function withState<T>(container: HTMLElement, load: () => Promise<T>, render: (data: T) => string) {
  container.innerHTML = stateHtml("cargando...");
  try {
    const data = await load();
    container.innerHTML = render(data);
  } catch (err) {
    container.innerHTML = stateHtml(`No se pudo cargar: ${String(err)}`, true);
  }
}
