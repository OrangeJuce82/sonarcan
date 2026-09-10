<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { ImportProvider } from "./types";
  export let provider: ImportProvider;
  export let size = "14px";
</script>

<span class="provider-logo" class:youtube={provider === "youtube"} class:soundcloud={provider === "soundcloud"} class:bandcamp={provider === "bandcamp"} class:mixcloud={provider === "mixcloud"} class:web={provider === "web"} style={`--provider-size:${size}`} aria-hidden="true">
  {#if provider === "youtube"}<Icon name="youtube" size="100%" />
  {:else if provider === "soundcloud"}<svg viewBox="0 0 28 16"><path d="M1 10v4M4 7v7M7 5v9M10 3v11M13 1v13M16 5a6 6 0 1 1 1 9h-4V5z" /></svg>
  {:else if provider === "bandcamp"}<svg viewBox="0 0 28 16"><path d="M8 2h19l-7 12H1L8 2z" /></svg>
  {:else if provider === "mixcloud"}<svg viewBox="0 0 28 16"><path d="M3 11a4 4 0 0 1 5-4 7 7 0 0 1 13 2 3 3 0 1 1 1 6H5a4 4 0 0 1-2-4zM10 5l2 7M14 3l2 9M18 6l2 6" /></svg>
  {:else}<Icon name={provider === "local" ? "file" : "cloud-arrow-down"} size="100%" />{/if}
</span>

<style>
  .provider-logo { display: inline-grid; width: var(--provider-size); height: var(--provider-size); flex: 0 0 auto; place-items: center; color: var(--muted); }
  .provider-logo svg { width: 100%; height: 100%; overflow: visible; }
  .provider-logo svg path { fill: currentColor; stroke: currentColor; stroke-width: 1.7; stroke-linecap: round; stroke-linejoin: round; }
  .provider-logo.soundcloud svg path, .provider-logo.mixcloud svg path { fill: none; }
  .provider-logo.youtube { color: #ff0033; }
  .provider-logo.soundcloud { color: #ff5500; }
  .provider-logo.bandcamp { color: #1da0c3; }
  .provider-logo.mixcloud { color: #52aad8; }
</style>
