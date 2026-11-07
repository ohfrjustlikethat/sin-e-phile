import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

// Tauri drives this; see src-tauri/tauri.conf.json.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  // Tauri expects a fixed port and fails rather than silently picking another. Cargo's
  // target/ is never the frontend's business: watching it costs and serves nothing.
  server: { port: 5173, strictPort: true, watch: { ignored: ["**/target/**", "**/src-tauri/**"] } },
  // The one entry. Unset, Vite's dependency scan takes every .html in the project as an
  // entry, including the copies of index.html that each debug build of the app leaves
  // under target/ (96 of them), and the scan never finished: the dev server hung.
  optimizeDeps: { entries: ["index.html"] },
  // Vite writes here; src-tauri/tauri.conf.json reads it as frontendDist.
  build: { outDir: "dist", emptyOutDir: true, target: "esnext" },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
  },
});
