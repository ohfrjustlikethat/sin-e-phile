import js from "@eslint/js";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    // Generated or produced, not authored. `target/**` matters: the Cargo
    // workspace root is the repo root, so Rust build output — including
    // JavaScript that tauri-codegen emits — lands here and would otherwise be
    // linted as project source.
    ignores: [
      "dist/**",
      "target/**",
      "src/lib/ipc.ts",
      "src-tauri/**",
      "crates/**",
      "spikes/**",
      "node_modules/**",
      // Standalone static HTML, deliberately not part of the app build. They are
      // a record of what was chosen in Phase 2 (ADR-0024), kept as they were shown.
      "docs/design/mockups/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    languageOptions: {
      globals: {
        window: "readonly",
        document: "readonly",
        console: "readonly",
        requestAnimationFrame: "readonly",
        setInterval: "readonly",
        setTimeout: "readonly",
        localStorage: "readonly",
        HTMLElement: "readonly",
      },
    },
    rules: {
      // A floating promise in a desktop app is a silently-swallowed failure.
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
      "no-console": ["warn", { allow: ["error", "warn"] }],
    },
  },
  {
    // tools/ are Node scripts, not app code: they run under Node, they have no
    // DOM, and printing a measurement to stdout is the entire point of a harness.
    files: ["tools/**/*.mjs", "*.config.js"],
    languageOptions: {
      globals: {
        process: "readonly",
        console: "readonly",
        fetch: "readonly",
        WebSocket: "readonly",
        Buffer: "readonly",
        URL: "readonly",
      },
    },
    rules: { "no-console": "off" },
  },
);
