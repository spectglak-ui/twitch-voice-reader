import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

// Voir https://v2.tauri.app/start/frontend/vite/ : Tauri exige un port de
// dev fixe (référencé dans `src-tauri/tauri.conf.json` -> `build.devUrl`)
// et ignore le répertoire `src-tauri` lors du watch pour éviter des
// rebuilds Rust déclenchés par erreur par des changements frontend.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
