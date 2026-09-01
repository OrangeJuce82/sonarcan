<script lang="ts">
  import { chordDegreeLabel, keyboardFlatPitchNames, keyboardPitchNames, parseChordLabel } from "./chordNotes";

  export let label: string;
  export let accessibleLabel: string;
  export let accidentals: "flat" | "sharp";
  export let positions: readonly number[];
  export let labelMode: "notes" | "degrees" = "notes";
  export let chordColor = "var(--accent)";

  const whitePitches = [0, 2, 4, 5, 7, 9, 11] as const;
  const blackPitches = [{ pitch: 1, boundary: 1 }, { pitch: 3, boundary: 2 }, { pitch: 6, boundary: 4 }, { pitch: 8, boundary: 5 }, { pitch: 10, boundary: 6 }] as const;
  const whiteKeys = Array.from({ length: 3 }, (_, octave) => whitePitches.map((pitch) => ({ pitch, position: pitch + octave * 12 }))).flat();
  const blackKeys = Array.from({ length: 3 }, (_, octave) => blackPitches.map(({ pitch, boundary }) => ({
    pitch,
    position: pitch + octave * 12,
    left: ((octave * 7 + boundary) / 21) * 100,
  }))).flat();

  $: chord = parseChordLabel(label);
  $: displayedPitchNames = accidentals === "flat" ? keyboardFlatPitchNames : keyboardPitchNames;
  $: notes = chord?.pitches.map((pitch) => displayedPitchNames[pitch]).join(" · ") ?? "—";
  $: activePositions = new Set(positions);
  const active = (position: number): boolean => activePositions.has(position);
  const root = (position: number): boolean => chord ? active(position) && position % 12 === chord.root : false;
</script>

<div class="chord-keyboard" style={`--instrument-chord-color:${chordColor}`} role="img" aria-label={`${accessibleLabel}: ${label}, ${notes}`}>
  <div class="keys" aria-hidden="true">
    {#each whiteKeys as key}
      <i class:active={active(key.position)} class:root={root(key.position)}><span>{labelMode === "notes" ? displayedPitchNames[key.pitch] : chord ? chordDegreeLabel(chord, key.pitch) : ""}</span></i>
    {/each}
    {#each blackKeys as key}
      <b style={`left:${key.left}%`} class:active={active(key.position)} class:root={root(key.position)}><span>{labelMode === "notes" ? displayedPitchNames[key.pitch] : chord ? chordDegreeLabel(chord, key.pitch) : ""}</span></b>
    {/each}
  </div>
</div>

<style>
  .chord-keyboard { display: flex; align-self: stretch; width: 100%; min-height: 0; }
  .keys { position: relative; display: grid; grid-template-columns: repeat(21, 1fr); width: 100%; min-height: 170px; height: 100%; max-height: 220px; border: 1px solid var(--border-strong); border-radius: 7px; overflow: hidden; background: var(--surface-deep); }
  .keys i { position: relative; display: block; border: solid var(--border-strong); border-width: 0 1px 0 0; background: var(--keyboard-white); }
  .keys i:first-child { border-left-width: 1px; }
  .keys b { position: absolute; z-index: 2; top: 0; width: 3.27%; height: 62%; transform: translateX(-50%); border: 1px solid var(--surface-deep); border-radius: 0 0 4px 4px; background: var(--keyboard-black); box-shadow: 0 3px 4px #0007; }
  .keys span { position: absolute; right: 0; bottom: 7px; left: 0; color: var(--keyboard-white-text); font: 750 .55rem/1 ui-monospace, SFMono-Regular, monospace; text-align: center; }
  .keys b span { color: var(--keyboard-black-text); }
  .keys .active { background: color-mix(in srgb, var(--instrument-chord-color) 88%, var(--surface-raised)); box-shadow: inset 0 -4px 0 var(--instrument-chord-color); }
  .keys b.active { background: color-mix(in srgb, var(--instrument-chord-color) 88%, var(--surface-raised)); box-shadow: inset 0 -4px 0 var(--instrument-chord-color), 0 3px 4px #0006; }
  .keys .active span { color: color-mix(in srgb, var(--instrument-chord-color) 25%, var(--chord-ink)); }
  .keys .root { outline: 3px solid color-mix(in srgb, var(--instrument-chord-color) 55%, var(--text-strong)); outline-offset: -4px; }
  @media (max-width: 1080px) { .keys { height: 170px; } }
</style>
