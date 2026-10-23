import { resolve } from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  root: "ui",
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "es2022",
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, "ui/index.html"),
        popup: resolve(import.meta.dirname, "ui/popup.html"),
        ask: resolve(import.meta.dirname, "ui/ask.html"),
        snip: resolve(import.meta.dirname, "ui/snip.html"),
      },
    },
  },
});
