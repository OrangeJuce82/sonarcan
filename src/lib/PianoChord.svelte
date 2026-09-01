<script lang="ts">
  import ChordKeyboard from "./ChordKeyboard.svelte";
  import { pianoVoicings } from "./instrumentVoicings";

  export let label: string;
  export let accidentals: "flat" | "sharp";
  export let accessibleLabel: string;
  export let positionLabel: string;
  export let unavailableLabel: string;
  export let chordColor = "var(--accent)";

  let selectedIndex = 0;
  let lastLabel = "";
  $: if (label !== lastLabel) {
    lastLabel = label;
    selectedIndex = 0;
  }
  $: positions = pianoVoicings(label);
  $: if (selectedIndex >= positions.length) selectedIndex = 0;
  $: selected = positions[selectedIndex] ?? null;

  function previous(): void {
    selectedIndex = positions.length ? (selectedIndex - 1 + positions.length) % positions.length : 0;
  }

  function next(): void {
    selectedIndex = positions.length ? (selectedIndex + 1) % positions.length : 0;
  }
</script>

<div class="piano-chord-view">
  {#if selected}
    <div class="position-toolbar">
      <button disabled={positions.length < 2} aria-label="‹" onclick={previous}>‹</button>
      <strong>{positionLabel} {selectedIndex + 1}/{positions.length}</strong>
      <button disabled={positions.length < 2} aria-label="›" onclick={next}>›</button>
    </div>
    <ChordKeyboard {label} {accessibleLabel} {accidentals} positions={selected} {chordColor} />
  {:else}
    <p>{unavailableLabel}</p>
  {/if}
</div>

<style>
  .piano-chord-view { display: grid; align-content: center; gap: 12px; min-height: 208px; }
  .position-toolbar { display: flex; align-items: center; justify-content: center; gap: 8px; }
  .position-toolbar button { width: 28px; min-width: 28px; height: 26px; padding: 0; }
  .position-toolbar strong { min-width: 92px; color: var(--text); font-size: .65rem; text-align: center; }
  .piano-chord-view :global(.chord-keyboard) { align-self: center; height: 170px; min-height: 170px; max-height: 170px; }
  p { align-self: center; margin: 0; color: var(--muted); font-size: .62rem; text-align: center; }
</style>
