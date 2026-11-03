import { defineConfig } from "vite";

export default defineConfig({
  root: "ui",
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // The design tokens live in docs/design, outside the Vite root.
    fs: { allow: [".."] },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // main.ts uses top-level await, which every Tauri webview supports.
    target: "es2022",
  },
});
