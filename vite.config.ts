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
  // Tauri expects a fixed port and fails rather than silently picking another.
  server: { port: 5173, strictPort: true },
  // Vite writes here; src-tauri/tauri.conf.json reads it as frontendDist.
  build: { outDir: "dist", emptyOutDir: true, target: "esnext" },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
  },
});
