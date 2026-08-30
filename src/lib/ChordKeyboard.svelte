<script lang="ts">
  import { keyboardPitchNames, parseChordLabel } from "./chordNotes";

  export let label: string;
  export let accessibleLabel: string;

  const whitePitches = [0, 2, 4, 5, 7, 9, 11] as const;
  const blackKeys = [
    { pitch: 1, left: 10.8 },
    { pitch: 3, left: 25.1 },
    { pitch: 6, left: 53.7 },
    { pitch: 8, left: 68 },
    { pitch: 10, left: 82.3 },
  ] as const;

  $: chord = parseChordLabel(label);
  $: notes = chord?.pitchNames.join(" · ") ?? "—";
  const active = (pitch: number): boolean => chord?.pitches.includes(pitch) ?? false;
</script>

<div class="chord-keyboard" role="img" aria-label={`${accessibleLabel}: ${label}, ${notes}`}>
  <div class="keys" aria-hidden="true">
    {#each whitePitches as pitch}
      <i class:active={active(pitch)} class:root={chord?.root === pitch} class:bass={chord?.bass === pitch}><span>{keyboardPitchNames[pitch]}</span></i>
    {/each}
    {#each blackKeys as key}
      <b style={`left:${key.left}%`} class:active={active(key.pitch)} class:root={chord?.root === key.pitch} class:bass={chord?.bass === key.pitch}><span>{keyboardPitchNames[key.pitch]}</span></b>
    {/each}
  </div>
</div>

<style>
  .chord-keyboard { display: flex; width: 100%; padding-top: 7px; border-top: 1px solid var(--border); }
  .keys { position: relative; display: grid; grid-template-columns: repeat(7, 1fr); width: 100%; height: 76px; border-radius: 5px; overflow: hidden; background: #080c0e; }
  .keys i { position: relative; display: block; border: solid #354047; border-width: 1px 1px 1px 0; background: #d9dedf; }
  .keys i:first-child { border-left-width: 1px; }
  .keys b { position: absolute; z-index: 2; top: 0; width: 8.5%; height: 47px; transform: translateX(-50%); border: 1px solid #020303; border-radius: 0 0 3px 3px; background: #11181b; box-shadow: 0 2px 2px #0008; }
  .keys span { position: absolute; right: 0; bottom: 4px; left: 0; color: #435056; font: 750 .5rem/1 ui-monospace, SFMono-Regular, monospace; text-align: center; }
  .keys b span { color: #829096; }
  .keys .active { background: color-mix(in srgb, var(--gold) 72%, #fff); box-shadow: inset 0 -3px 0 color-mix(in srgb, var(--gold) 65%, #5a3f13); }
  .keys b.active { background: color-mix(in srgb, var(--gold) 68%, #221a0c); box-shadow: inset 0 -3px 0 #704f18, 0 2px 2px #0008; }
  .keys .root { outline: 2px solid var(--accent); outline-offset: -3px; }
  .keys .bass span { font-weight: 900; text-decoration: underline; }
  @media (max-width: 720px) { .keys { width: 100%; } }
</style>
