import { describe, expect, it } from "vitest";

import { cardHtml } from "./cards";
import { beginView, escapeHtml, withView } from "./render";

/** Parsea un fragmento de HTML igual que lo haría el navegador al asignarlo a
 *  `innerHTML`: sin esto, afirmar sobre el string es afirmar sobre la intención,
 *  no sobre lo que el parser realmente construye. */
function parse(html: string): HTMLElement {
  const host = document.createElement("div");
  host.innerHTML = html;
  return host;
}

describe("escapeHtml", () => {
  it("escapa los cinco caracteres, no solo los tres de contexto de texto", () => {
    expect(escapeHtml(`& < > " '`)).toBe("&amp; &lt; &gt; &quot; &#39;");
  });

  it("no deja pasar una comilla doble cruda (rompería un atributo)", () => {
    expect(escapeHtml('x" onmouseover="y')).not.toContain('"');
  });

  it("escapa el ampersand primero, sin doble escape de las entidades que emite", () => {
    expect(escapeHtml("<&>")).toBe("&lt;&amp;&gt;");
  });
});

describe("cardHtml en contexto de atributo", () => {
  it("un name con comilla doble no puede inyectar atributos ni anular data-readonly", () => {
    const host = parse(
      cardHtml({
        icon: "🤖",
        title: 'x" data-readonly="0',
        description: "hostil",
        path: "/home/u/.claude/agents/x.md",
        readOnly: true,
      }),
    );

    const card = host.querySelector<HTMLElement>(".card");
    expect(card).not.toBeNull();
    expect(card!.dataset.readonly).toBe("1");
    expect(card!.dataset.title).toBe('x" data-readonly="0');
  });

  it("un path con comilla doble no trunca data-path (el editor abriría otro archivo)", () => {
    const path = '/home/u/.claude/skills/dice "hola"/SKILL.md';
    const host = parse(
      cardHtml({ icon: "🛠️", title: "s", description: "d", path }),
    );

    expect(host.querySelector<HTMLElement>(".card")!.dataset.path).toBe(path);
  });

  it("contenido hostil en la descripción no se convierte en markup ejecutable", () => {
    const host = parse(
      cardHtml({
        icon: "🧩",
        title: "t",
        description: '<img src=x onerror="boom()">',
        path: "/home/u/.claude/memory/m.md",
      }),
    );

    expect(host.querySelector("img")).toBeNull();
    expect(host.textContent).toContain('<img src=x onerror="boom()">');
  });
});

describe("withView y el guard de generación de vista", () => {
  /** Promesa con resolve expuesto, para ordenar a mano quién responde primero. */
  function deferred<T>() {
    let resolve!: (v: T) => void;
    const promise = new Promise<T>((r) => {
      resolve = r;
    });
    return { promise, resolve };
  }

  it("una vista lenta no pisa la que se activó después", async () => {
    const container = document.createElement("div");
    const slow = deferred<string>();

    beginView();
    const viejo = withView(
      container,
      () => slow.promise,
      (d) => `<p>${d}</p>`,
    );

    // El usuario cambia de pestaña: la vista nueva pinta primero.
    beginView();
    await withView(
      container,
      () => Promise.resolve("pestaña nueva"),
      (d) => `<p>${d}</p>`,
    );
    expect(container.textContent).toBe("pestaña nueva");

    // Recién ahora contesta la vista vieja.
    slow.resolve("pestaña vieja");
    await viejo;

    expect(container.textContent).toBe("pestaña nueva");
  });

  it("el error de una vista abandonada tampoco pisa la vista vigente", async () => {
    const container = document.createElement("div");
    const slow = deferred<string>();

    beginView();
    const viejo = withView(
      container,
      () => slow.promise,
      (d) => `<p>${d}</p>`,
    );

    beginView();
    await withView(
      container,
      () => Promise.resolve("pestaña nueva"),
      (d) => `<p>${d}</p>`,
    );

    slow.resolve(Promise.reject(new Error("backend caído")) as unknown as string);
    await viejo;

    expect(container.textContent).toBe("pestaña nueva");
  });
});
