/// <reference types="vitest/config" />
import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  test: {
    // jsdom: el frontend manipula DOM real (innerHTML, dataset, focus) y los
    // controles de seguridad que hay que cubrir (escapado en contexto de
    // atributo, data-readonly) solo se pueden afirmar parseando ese DOM.
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Cargo reescribe los .dll en src-tauri/target durante cada build; sin este
      // ignore, el watcher de Vite choca (EBUSY en Windows) contra archivos lockeados.
      ignored: ["**/src-tauri/**"],
    },
  },
});
