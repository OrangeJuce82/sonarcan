<script lang="ts">
  import { keyboardFlatPitchNames, keyboardPitchNames } from "./chordNotes";
  import { fretboardStartFret, INSTRUMENTS, instrumentVoicings, type FrettedInstrument } from "./instrumentVoicings";

  export let label: string;
  export let instrument: FrettedInstrument;
  export let accidentals: "flat" | "sharp";
  export let accessibleLabel: string;
  export let positionLabel: string;
  export let adaptedLabel: string;
  export let unavailableLabel: string;
  export let omittedLabel: string;
  export let chordColor = "var(--accent)";

  let selectedIndex = 0;
  let lastIdentity = "";
  $: identity = `${instrument}:${label}`;
  $: if (identity !== lastIdentity) {
    lastIdentity = identity;
    selectedIndex = 0;
  }
  $: voicings = instrumentVoicings(label, instrument);
  $: if (selectedIndex >= voicings.length) selectedIndex = 0;
  $: selected = voicings[selectedIndex] ?? null;
  $: diagramStartFret = fretboardStartFret(selected);
  $: visibleFretCount = 7;
  $: definition = INSTRUMENTS[instrument];
  $: displayedStrings = definition.tuning.map((name, string) => ({ name, string })).reverse();
  $: pitchNames = accidentals === "flat" ? keyboardFlatPitchNames : keyboardPitchNames;
  $: omitted = selected?.omittedPitches.map((pitch) => pitchNames[pitch]).join(" · ") ?? "";

  function previous(): void {
    selectedIndex = voicings.length ? (selectedIndex - 1 + voicings.length) % voicings.length : 0;
  }

  function next(): void {
    selectedIndex = voicings.length ? (selectedIndex + 1) % voicings.length : 0;
  }
</script>

<div class="fretboard-view" style={`--instrument-chord-color:${chordColor};--visible-frets:${visibleFretCount}`} role="img" aria-label={`${accessibleLabel}: ${label}`}>
  {#if selected}
    <div class="voicing-toolbar">
      <button aria-label="‹" onclick={previous}>‹</button>
      <strong>{positionLabel} {selectedIndex + 1}/{voicings.length}</strong>
      <button aria-label="›" onclick={next}>›</button>
      {#if selected.coverage === "adapted"}<span>{adaptedLabel}</span>{/if}
    </div>
    <div class="fretboard-diagram" aria-hidden="true">
      {#each displayedStrings as item}
        <div class="fret-string">
          <b>{item.name}</b>
          <i class:open={selected.frets[item.string] === 0} class:muted={selected.frets[item.string] < 0}>{selected.frets[item.string] < 0 ? "×" : selected.frets[item.string] === 0 ? "○" : ""}</i>
          {#each Array(visibleFretCount) as _, fretOffset}
            <span>
              {#if selected.frets[item.string] > 0 && selected.frets[item.string] - diagramStartFret === fretOffset}
                <em>{selected.fingers[item.string]}</em>
              {/if}
            </span>
          {/each}
        </div>
      {/each}
      {#if diagramStartFret >= 5}<small>{diagramStartFret}</small>{/if}
    </div>
    {#if omitted}<p class="voicing-omissions">{omittedLabel}: {omitted}</p>{/if}
  {:else}
    <p class="voicing-empty">{unavailableLabel}</p>
  {/if}
</div>

<style>
  .fretboard-view { display: grid; align-content: center; gap: 12px; min-height: 175px; }
  .voicing-toolbar { display: flex; align-items: center; justify-content: center; gap: 8px; }
  .voicing-toolbar button { width: 28px; min-width: 28px; height: 26px; padding: 0; }
  .voicing-toolbar strong { min-width: 92px; color: var(--text); font-size: .65rem; text-align: center; }
  .voicing-toolbar span { padding: 3px 7px; border: 1px solid var(--gold); border-radius: 999px; color: var(--gold); font-size: .55rem; font-weight: 800; }
  .fretboard-diagram { position: relative; display: grid; gap: 0; width: min(560px, 98%); margin: 0 auto; padding-left: 28px; }
  .fret-string { display: grid; grid-template-columns: 20px 18px repeat(var(--visible-frets), 1fr); align-items: center; min-height: 24px; }
  .fret-string > b { color: var(--muted); font: 700 .56rem/1 ui-monospace, monospace; }
  .fret-string > i { color: var(--muted); font: 800 .72rem/1 ui-monospace, monospace; text-align: center; }
  .fret-string > i.open { color: var(--instrument-chord-color); }
  .fret-string > span { position: relative; height: 24px; border-left: 2px solid var(--border-strong); background: linear-gradient(transparent 47%, var(--muted) 48% 52%, transparent 53%); }
  .fret-string > span:last-child { border-right: 2px solid var(--border-strong); }
  .fret-string em { position: absolute; z-index: 1; top: 50%; left: 50%; display: grid; place-items: center; width: 17px; height: 17px; transform: translate(-50%, -50%); border: 1px solid color-mix(in srgb, var(--instrument-chord-color) 55%, var(--text-strong)); border-radius: 50%; color: var(--surface-deep); background: color-mix(in srgb, var(--instrument-chord-color) 88%, var(--surface-raised)); box-shadow: 0 0 5px color-mix(in srgb, var(--instrument-chord-color) 38%, transparent); font: 800 .55rem/1 ui-monospace, monospace; }
  .fretboard-diagram small { position: absolute; left: 7px; bottom: 3px; color: var(--muted); font-size: .55rem; }
  .voicing-omissions, .voicing-empty { margin: 0; color: var(--muted); font-size: .62rem; text-align: center; }
  .voicing-empty { align-self: center; }
</style>
