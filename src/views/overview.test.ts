import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SettingsSummary } from "../lib/api";

// El invoke real necesita el runtime de Tauri; acá interesa el sink de HTML, no
// el IPC. Se mockea la capa `api` entera con datos hostiles.
const hostileSettings: SettingsSummary = {
  model: '<img src=x onerror="boom()">',
  theme: '"><script>boom()</script>',
  permissions_allow: [],
  hooks_events: [],
  enabled_plugins: [],
};

vi.mock("../lib/api", async () => {
  const empty = () => Promise.resolve([]);
  return {
    api: {
      listMemories: empty,
      listSkills: empty,
      listAgents: empty,
      listScheduledTasks: empty,
      listCommands: empty,
      listWorkflows: empty,
      readSettingsSummary: () => Promise.resolve(hostileSettings),
    },
    errorMessage: (e: unknown) => String(e),
  };
});

const { renderOverview } = await import("./overview");

describe("vista general con un settings.json hostil", () => {
  let container: HTMLElement;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  it("no inyecta markup ejecutable a partir de model/theme", async () => {
    await renderOverview(container);

    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("script")).toBeNull();
  });

  it("muestra los valores como texto literal", async () => {
    await renderOverview(container);

    expect(container.textContent).toContain('<img src=x onerror="boom()">');
    expect(container.textContent).toContain('"><script>boom()</script>');
  });
});
