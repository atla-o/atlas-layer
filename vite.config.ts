import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    host: "0.0.0.0",
    port: 47331,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2022",
    minify: "esbuild",
    sourcemap: false,
    outDir: "dist",
  },
});
