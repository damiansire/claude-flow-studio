import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
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
