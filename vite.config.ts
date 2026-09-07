import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig(() => {
  const lightEdition = process.env.SONARCAN_EDITION === "light";
  const source = (path: string): string => fileURLToPath(new URL(path, import.meta.url));
  return {
  plugins: [svelte()],
  resolve: {
    alias: lightEdition ? [
      { find: "./lib/FretboardChord.svelte", replacement: source("./src/lib/light/FretboardChord.svelte") },
      { find: "./lib/PianoChord.svelte", replacement: source("./src/lib/light/PianoChord.svelte") },
    ] : [],
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.endsWith("/instrumentChordCorpus.json")) return "instrument-chord-corpus";
          if (id.endsWith("/i18n-extra.ts")) return "translations";
          if (id.includes("/node_modules/")) return "vendor";
          return undefined;
        },
      },
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  };
});
