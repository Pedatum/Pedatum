import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 1420,
    strictPort: true,
    allowedHosts: true,
  },
  build: {
    target: "esnext",
    outDir: "dist",
  },
});
