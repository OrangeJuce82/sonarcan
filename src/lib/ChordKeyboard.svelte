<script lang="ts">
  import { chordKeyboardPositions, keyboardFlatPitchNames, keyboardPitchNames, keyboardPosition, parseChordLabel } from "./chordNotes";

  export let label: string;
  export let accessibleLabel: string;
  export let accidentals: "flat" | "sharp";

  const whiteKeys = [
    { pitch: 0, position: 0 }, { pitch: 2, position: 2 }, { pitch: 4, position: 4 }, { pitch: 5, position: 5 }, { pitch: 7, position: 7 }, { pitch: 9, position: 9 }, { pitch: 11, position: 11 },
    { pitch: 0, position: 12 }, { pitch: 2, position: 14 }, { pitch: 4, position: 16 }, { pitch: 5, position: 17 }, { pitch: 7, position: 19 }, { pitch: 9, position: 21 }, { pitch: 11, position: 23 },
  ] as const;
  const blackKeys = [
    { pitch: 1, position: 1, left: 7.143 },
    { pitch: 3, position: 3, left: 14.286 },
    { pitch: 6, position: 6, left: 28.571 },
    { pitch: 8, position: 8, left: 35.714 },
    { pitch: 10, position: 10, left: 42.857 },
    { pitch: 1, position: 13, left: 57.143 },
    { pitch: 3, position: 15, left: 64.286 },
    { pitch: 6, position: 18, left: 78.571 },
    { pitch: 8, position: 20, left: 85.714 },
    { pitch: 10, position: 22, left: 92.857 },
  ] as const;

  $: chord = parseChordLabel(label);
  $: displayedPitchNames = accidentals === "flat" ? keyboardFlatPitchNames : keyboardPitchNames;
  $: notes = chord?.pitches.map((pitch) => displayedPitchNames[pitch]).join(" · ") ?? "—";
  $: activePositions = new Set(chord ? chordKeyboardPositions(chord) : []);
  const active = (position: number): boolean => activePositions.has(position);
  const root = (position: number): boolean => chord ? position === chord.root : false;
  const bass = (position: number): boolean => chord ? position === keyboardPosition(chord.bass, chord.root) : false;
</script>

<div class="chord-keyboard" role="img" aria-label={`${accessibleLabel}: ${label}, ${notes}`}>
  <div class="keys" aria-hidden="true">
    {#each whiteKeys as key}
      <i class:active={active(key.position)} class:root={root(key.position)} class:bass={bass(key.position)}><span>{displayedPitchNames[key.pitch]}</span></i>
    {/each}
    {#each blackKeys as key}
      <b style={`left:${key.left}%`} class:active={active(key.position)} class:root={root(key.position)} class:bass={bass(key.position)}><span>{displayedPitchNames[key.pitch]}</span></b>
    {/each}
  </div>
</div>

<style>
  .chord-keyboard { display: flex; align-self: stretch; width: 100%; min-height: 0; }
  .keys { position: relative; display: grid; grid-template-columns: repeat(14, 1fr); width: 100%; min-height: 170px; height: 100%; max-height: 220px; border: 1px solid var(--border-strong); border-radius: 7px; overflow: hidden; background: var(--surface-deep); }
  .keys i { position: relative; display: block; border: solid var(--border-strong); border-width: 0 1px 0 0; background: var(--keyboard-white); }
  .keys i:first-child { border-left-width: 1px; }
  .keys b { position: absolute; z-index: 2; top: 0; width: 4.9%; height: 62%; transform: translateX(-50%); border: 1px solid var(--surface-deep); border-radius: 0 0 4px 4px; background: var(--keyboard-black); box-shadow: 0 3px 4px #0007; }
  .keys span { position: absolute; right: 0; bottom: 7px; left: 0; color: var(--keyboard-white-text); font: 750 .55rem/1 ui-monospace, SFMono-Regular, monospace; text-align: center; }
  .keys b span { color: var(--keyboard-black-text); }
  .keys .active { background: color-mix(in srgb, var(--accent) 82%, var(--accent-strong)); box-shadow: inset 0 -4px 0 var(--accent-strong); }
  .keys b.active { background: color-mix(in srgb, var(--accent) 82%, var(--accent-strong)); box-shadow: inset 0 -4px 0 var(--accent-strong), 0 3px 4px #0006; }
  .keys .active span { color: var(--surface-deep); }
  .keys .root { outline: 3px solid var(--accent-strong); outline-offset: -4px; }
  .keys .bass span { font-weight: 900; text-decoration: underline; }
  @media (max-width: 1080px) { .keys { height: 170px; } }
</style>
