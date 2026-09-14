<script lang="ts">
  import Icon from "./Icon.svelte";
  import MusicSourceLogo from "./MusicSourceLogo.svelte";
  import type { MusicSourceId } from "./musicSources";
  import type { ImportProvider } from "./types";
  export let provider: ImportProvider;
  export let size = "14px";
</script>

<span class="provider-logo" class:youtube={provider === "youtube"} class:soundcloud={provider === "soundcloud"} class:bandcamp={provider === "bandcamp"} class:mixcloud={provider === "mixcloud"} class:web={provider === "web"} style={`--provider-size:${size}`} aria-hidden="true">
  {#if provider === "hearthis.at"}<MusicSourceLogo source="hearthis" {size} />
  {:else if provider !== "web" && provider !== "local"}<MusicSourceLogo source={provider as MusicSourceId} {size} />
  {:else}<Icon name={provider === "local" ? "file" : "cloud-arrow-down"} size="100%" />{/if}
</span>

<style>
  .provider-logo { display: inline-grid; width: var(--provider-size); height: var(--provider-size); flex: 0 0 auto; place-items: center; color: var(--muted); }
</style>
