import { beforeEach, describe, expect, it, vi } from "vitest";

import type { StagedChange } from "./api";

/** Promesa con resolve expuesto, para ordenar a mano quién responde primero. */
function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

const files = new Map<string, string>();
const staged: StagedChange[] = [];
let applyCalls = 0;
let readFileContent: (path: string) => Promise<string> = (path) =>
  Promise.resolve(files.get(path) ?? "");

vi.mock("./api", () => ({
  api: {
    readFileContent: (path: string) => readFileContent(path),
    listStaged: () => Promise.resolve([...staged]),
    stageChange: (target_path: string, draft_content: string) => {
      const change: StagedChange = {
        id: `id-${staged.length}`,
        target_path,
        draft_content,
        created_at: "2026-01-01T00:00:00Z",
      };
      staged.push(change);
      return Promise.resolve(change);
    },
    applyStaged: () => {
      applyCalls += 1;
      return new Promise((r) => setTimeout(() => r(undefined), 10));
    },
    diffStaged: () => Promise.resolve(""),
    discardStaged: () => Promise.resolve(undefined),
  },
  errorMessage: (e: unknown) => String(e),
}));

const { mountEditorModal, openEditor } = await import("./editor");

function el<T extends HTMLElement>(id: string): T {
  const found = document.querySelector<T>(id);
  if (!found) throw new Error(`no se encontró ${id}`);
  return found;
}

describe("editor modal", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    files.clear();
    staged.length = 0;
    applyCalls = 0;
    readFileContent = (path) => Promise.resolve(files.get(path) ?? "");
    mountEditorModal(document.body);
  });

  it("una apertura lenta no pisa la que el usuario abrió después", async () => {
    files.set("/a.md", "contenido de A");
    files.set("/b.md", "contenido de B");
    const slowA = deferred<string>();

    readFileContent = (path) => (path === "/a.md" ? slowA.promise : Promise.resolve(files.get(path) ?? ""));

    const openA = openEditor("A", "/a.md");
    const openB = openEditor("B", "/b.md");
    await openB;

    // Recién ahora responde la lectura de A, ya obsoleta.
    slowA.resolve("contenido de A");
    await openA;

    expect(el<HTMLHeadingElement>("#editor-title").textContent).toBe("B");
    expect(el<HTMLTextAreaElement>("#editor-textarea").value).toBe("contenido de B");
  });

  it("doble click en Aplicar dispara un solo apply_staged", async () => {
    files.set("/a.md", "v1");
    await openEditor("A", "/a.md");

    el<HTMLTextAreaElement>("#editor-textarea").value = "v2";
    el<HTMLButtonElement>("#editor-stage").click();

    const apply = el<HTMLButtonElement>("#editor-apply");
    await vi.waitFor(() => expect(apply.disabled).toBe(false));

    // Dos clicks seguidos, más rápido de lo que tarda el backend.
    apply.click();
    apply.click();
    await new Promise((r) => setTimeout(r, 60));

    expect(applyCalls).toBe(1);
  });

  it("los controles se bloquean mientras hay una mutación en vuelo", async () => {
    files.set("/a.md", "v1");
    await openEditor("A", "/a.md");

    el<HTMLTextAreaElement>("#editor-textarea").value = "v2";
    el<HTMLButtonElement>("#editor-stage").click();

    const apply = el<HTMLButtonElement>("#editor-apply");
    await vi.waitFor(() => expect(apply.disabled).toBe(false));
    apply.click();

    expect(el<HTMLButtonElement>("#editor-stage").disabled).toBe(true);
    expect(el<HTMLButtonElement>("#editor-discard").disabled).toBe(true);

    // Dejar la operación terminada: el guard de reentrada es estado de módulo y
    // arrastrarlo prendido contaminaría el test siguiente.
    await vi.waitFor(() => expect(el<HTMLButtonElement>("#editor-stage").disabled).toBe(false));
  });

  it("cerrar con texto sin guardar pide confirmación y respeta el 'no'", async () => {
    files.set("/a.md", "original");
    await openEditor("A", "/a.md");
    const textarea = el<HTMLTextAreaElement>("#editor-textarea");
    textarea.value = "tipeado y sin guardar";

    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    el<HTMLButtonElement>("#editor-close").click();

    expect(confirmSpy).toHaveBeenCalledOnce();
    expect(document.querySelector(".overlay")!.classList.contains("hidden")).toBe(false);
    expect(textarea.value).toBe("tipeado y sin guardar");
    confirmSpy.mockRestore();
  });

  it("cerrar sin cambios no molesta con un confirm", async () => {
    files.set("/a.md", "original");
    await openEditor("A", "/a.md");

    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    el<HTMLButtonElement>("#editor-close").click();

    expect(confirmSpy).not.toHaveBeenCalled();
    expect(document.querySelector(".overlay")!.classList.contains("hidden")).toBe(true);
    confirmSpy.mockRestore();
  });

  it("guardar el borrador deja de contar como cambios sin guardar", async () => {
    files.set("/a.md", "original");
    await openEditor("A", "/a.md");
    el<HTMLTextAreaElement>("#editor-textarea").value = "editado";
    el<HTMLButtonElement>("#editor-stage").click();
    await vi.waitFor(() => expect(staged).toHaveLength(1));

    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    el<HTMLButtonElement>("#editor-close").click();

    expect(confirmSpy).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });
});
