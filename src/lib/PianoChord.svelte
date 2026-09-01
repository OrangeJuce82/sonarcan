<script lang="ts">
  import ChordKeyboard from "./ChordKeyboard.svelte";
  import { parseChordLabel } from "./chordNotes";
  import { pianoVoicings } from "./instrumentVoicings";
  import VoicingPositionNav from "./VoicingPositionNav.svelte";

  export let label: string;
  export let accidentals: "flat" | "sharp";
  export let accessibleLabel: string;
  export let positionLabel: string;
  export let unavailableLabel: string;
  export let emptyLabel: string;
  export let labelMode: "notes" | "degrees" = "notes";
  export let chordColor = "var(--accent)";

  let selectedIndex = 0;
  let lastLabel = "";
  $: if (label !== lastLabel) {
    lastLabel = label;
    selectedIndex = 0;
  }
  $: positions = pianoVoicings(label);
  $: chord = parseChordLabel(label);
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
  <div class="position-toolbar">
    {#if selected}
      <VoicingPositionNav
        label={positionLabel}
        current={selectedIndex + 1}
        total={positions.length}
        onPrevious={previous}
        onNext={next}
      />
    {/if}
  </div>
  <ChordKeyboard {label} {accessibleLabel} {accidentals} positions={selected ?? []} {labelMode} {chordColor} />
  {#if !selected}<p>{chord ? unavailableLabel : emptyLabel}</p>{/if}
</div>

<style>
  .piano-chord-view { display: grid; align-content: start; gap: 8px; min-height: 224px; }
  .position-toolbar { display: flex; align-items: center; justify-content: center; min-height: 26px; }
  .piano-chord-view :global(.chord-keyboard) { align-self: center; height: 170px; min-height: 170px; max-height: 170px; }
  p { align-self: center; margin: 0; color: var(--muted); font-size: .55rem; text-align: center; }
</style>
