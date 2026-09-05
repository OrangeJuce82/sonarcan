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
          return id.endsWith("/instrumentChordCorpus.json") ? "instrument-chord-corpus" : undefined;
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
