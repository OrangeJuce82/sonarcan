<script lang="ts">
  import { chordDegreeLabel, keyboardFlatPitchNames, keyboardPitchNames, parseChordLabel } from "./chordNotes";
  import {
    INSTRUMENTS,
    fretMarkerCount,
    instrumentPitchAt,
    instrumentVoicings,
    type FrettedInstrument,
    type InstrumentVoicing,
  } from "./instrumentVoicings";
  import VoicingPositionNav from "./VoicingPositionNav.svelte";

  export let label: string;
  export let instrument: FrettedInstrument;
  export let accidentals: "flat" | "sharp";
  export let accessibleLabel: string;
  export let positionLabel: string;
  export let exactLabel: string;
  export let adaptedLabel: string;
  export let unavailableLabel: string;
  export let emptyLabel: string;
  export let omittedLabel: string;
  export let bassOmittedLabel: string;
  export let labelMode: "notes" | "degrees" = "notes";
  export let chordColor = "var(--accent)";

  const visibleFretCount = 5;
  let selectedIndex = 0;
  let lastIdentity = "";

  $: identity = `${instrument}:${label}`;
  $: if (identity !== lastIdentity) {
    lastIdentity = identity;
    selectedIndex = 0;
  }
  $: chord = parseChordLabel(label);
  $: voicings = instrumentVoicings(label, instrument);
  $: if (selectedIndex >= voicings.length) selectedIndex = 0;
  $: selected = voicings[selectedIndex] ?? null;
  $: definition = INSTRUMENTS[instrument];
  $: displayedStrings = definition.tuning.map((name, string) => ({ name, string })).reverse();
  $: pitchNames = accidentals === "flat" ? keyboardFlatPitchNames : keyboardPitchNames;
  $: diagramStartFret = selected && selected.baseFret >= 4 ? selected.baseFret : 1;
  $: displayedFrets = Array.from({ length: visibleFretCount }, (_, index) => diagramStartFret + index);
  $: omitted = selected?.omittedPitches.map((pitch) => pitchNames[pitch]).join(" · ") ?? "";
  function displayedPitch(pitch: number, mode: "notes" | "degrees"): string {
    return mode === "degrees" && chord ? chordDegreeLabel(chord, pitch) : pitchNames[pitch];
  }

  function selectVoicing(index: number): void {
    selectedIndex = index;
  }

  function selectPreviousVoicing(): void {
    selectedIndex = voicings.length ? (selectedIndex - 1 + voicings.length) % voicings.length : 0;
  }

  function selectNextVoicing(): void {
    selectedIndex = voicings.length ? (selectedIndex + 1) % voicings.length : 0;
  }

  function miniStart(voicing: InstrumentVoicing): number {
    return voicing.baseFret >= 4 ? voicing.baseFret : 1;
  }

  function miniMarkerLeft(voicing: InstrumentVoicing, string: number): number {
    return ((voicing.frets[string] - miniStart(voicing) + 0.5) / visibleFretCount) * 100;
  }

</script>

<div class="fretboard-view" style={`--instrument-chord-color:${chordColor}`} role="group" aria-label={`${accessibleLabel}: ${label}`}>
  <div class="voicing-toolbar">
    {#if chord && selected}
      <div class="position-navigation">
        <VoicingPositionNav
          label={positionLabel}
          current={selectedIndex + 1}
          total={voicings.length}
          onPrevious={selectPreviousVoicing}
          onNext={selectNextVoicing}
        />
      </div>
      {#if selected.coverage === "adapted"}<span class="adapted">{adaptedLabel}</span>{/if}
    {/if}
  </div>

    <div class="fretboard-body" class:no-variants={!selected}>
      <div class="selected-voicing">
        <div class="short-neck" role="img" aria-label={selected ? `${accessibleLabel}: ${label}, ${positionLabel} ${selectedIndex + 1}` : accessibleLabel}>
          <div class="neck-markers" aria-hidden="true">
            {#each displayedFrets as fret}
              <span>
                {#if fretMarkerCount(fret) === 1}<i></i>{/if}
                {#if fretMarkerCount(fret) === 2}<i></i><i></i>{/if}
              </span>
            {/each}
          </div>
          {#each displayedStrings as item}
            <div class="neck-string">
              <b>{item.name}</b>
              <span class="open-cell" class:wound-string={instrument === "guitar" && item.string <= 2}>
                {#if selected && chord}
                  {#if selected.frets[item.string] < 0}
                    <em>×</em>
                  {:else if selected.frets[item.string] === 0}
                    <i
                      class:root-note={instrumentPitchAt(definition, item.string, 0) === chord.root}
                    >{displayedPitch(instrumentPitchAt(definition, item.string, 0), labelMode)}</i>
                  {/if}
                {/if}
              </span>
              {#each displayedFrets as fret}
                <span class="fret-cell" class:wound-string={instrument === "guitar" && item.string <= 2}>
                  {#if selected && chord && selected.frets[item.string] === fret}
                    <i
                      class:root-note={instrumentPitchAt(definition, item.string, fret) === chord.root}
                    >{displayedPitch(instrumentPitchAt(definition, item.string, fret), labelMode)}</i>
                  {/if}
                </span>
              {/each}
            </div>
          {/each}
          <div class="fret-numbers" aria-hidden="true">
            <b></b><span>0</span>{#each displayedFrets as fret}<span>{fret}</span>{/each}
          </div>
        </div>
      </div>

      {#if selected && chord}
        <div class="voicing-list" aria-label={positionLabel}>
          {#each voicings as voicing, index}
            <button
              class:selected={index === selectedIndex}
              aria-label={`${positionLabel} ${index + 1}, ${voicing.coverage === "adapted" ? adaptedLabel : exactLabel}`}
              aria-pressed={index === selectedIndex}
              onclick={() => selectVoicing(index)}
            >
              <span class="mini-neck" aria-hidden="true">
                {#each displayedStrings as item}
                  <i>
                    {#if voicing.frets[item.string] < 0}<em>×</em>
                    {:else if voicing.frets[item.string] === 0}<em>○</em>
                    {:else}<b style={`left:${miniMarkerLeft(voicing, item.string)}%`}></b>{/if}
                  </i>
                {/each}
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    {#if omitted || selected?.bassOmitted}
      <p class="voicing-omissions">
        {#if selected?.bassOmitted}{bassOmittedLabel}{/if}
        {#if selected?.bassOmitted && omitted}<span> · </span>{/if}
        {#if omitted}{omittedLabel}: {omitted}{/if}
      </p>
    {/if}
    {#if !selected}<p class="voicing-empty">{chord ? unavailableLabel : emptyLabel}</p>{/if}
</div>

<style>
  .fretboard-view { display: grid; align-content: start; gap: 8px; min-height: 224px; }
  .voicing-toolbar { display: flex; align-items: center; justify-content: center; gap: 8px; min-height: 26px; }
  .position-navigation { display: flex; align-items: center; }
  .voicing-toolbar > span { padding: 3px 7px; border: 1px solid color-mix(in srgb, var(--accent) 70%, var(--border-strong)); border-radius: 999px; color: var(--accent); font-size: .55rem; font-weight: 800; }
  .voicing-toolbar > span.adapted { border-color: var(--gold); color: var(--gold); }
  .fretboard-body { display: grid; grid-template-columns: minmax(0, 1fr) 110px; align-items: center; gap: 10px; min-width: 0; min-height: 194px; }
  .fretboard-body.no-variants { grid-template-columns: minmax(0, 1fr); }
  .selected-voicing { display: grid; gap: 6px; min-width: 0; }
  .short-neck { position: relative; width: min(100%, 450px); margin-inline: auto; }
  .neck-markers { position: absolute; z-index: 0; top: 0; right: 0; bottom: 17px; left: 50px; display: grid; grid-template-columns: repeat(5, minmax(30px, 1fr)); pointer-events: none; }
  .neck-markers > span { position: relative; }
  .neck-markers i { position: absolute; top: 50%; left: 50%; width: 7px; height: 7px; transform: translate(-50%, -50%); border-radius: 50%; background: color-mix(in srgb, var(--gold) 42%, transparent); }
  .neck-markers i:first-child:nth-last-child(2) { top: 32%; }
  .neck-markers i + i { top: 68%; }
  .neck-string, .fret-numbers { display: grid; grid-template-columns: 20px 30px repeat(5, minmax(30px, 1fr)); align-items: center; }
  .neck-string, .fret-numbers { position: relative; z-index: 2; }
  .neck-string > b { color: var(--muted); font: 750 .56rem/1 ui-monospace, SFMono-Regular, monospace; }
  .neck-string > span { position: relative; display: grid; place-items: center; height: 27px; background: linear-gradient(transparent 48%, var(--muted) 49% 52%, transparent 53%); }
  .neck-string > span.wound-string { background-image: repeating-linear-gradient(90deg, var(--muted) 0 4px, transparent 4px 6px); background-position: center; background-size: 100% 2px; background-repeat: no-repeat; }
  .open-cell { border-right: 4px solid var(--border-strong); }
  .fret-cell { border-right: 1px solid var(--border-strong); }
  .neck-string i { z-index: 1; display: grid; place-items: center; width: 22px; height: 22px; border: 1px solid color-mix(in srgb, var(--instrument-chord-color) 55%, var(--text-strong)); border-radius: 50%; color: color-mix(in srgb, var(--instrument-chord-color) 25%, var(--chord-ink)); background: color-mix(in srgb, var(--instrument-chord-color) 90%, var(--surface-raised)); box-shadow: 0 0 6px color-mix(in srgb, var(--instrument-chord-color) 42%, transparent); font: 850 .52rem/1 ui-monospace, SFMono-Regular, monospace; }
  .neck-string i.root-note { border-width: 2px; }
  .neck-string em { position: absolute; z-index: 3; top: 50%; left: 50%; display: grid; place-items: center; width: 22px; height: 22px; transform: translate(-50%, -50%); color: var(--danger); font: 950 1.08rem/1 ui-monospace, monospace; text-align: center; }
  .fret-numbers { color: var(--muted); font: 700 .5rem/1 ui-monospace, monospace; text-align: center; }
  .fret-numbers span { padding-top: 4px; }
  .voicing-list { box-sizing: border-box; display: grid; align-content: start; gap: 6px; width: 100%; max-width: 100%; max-height: 194px; overflow-x: hidden; overflow-y: auto; padding: 3px 8px 3px 3px; overscroll-behavior-y: contain; scrollbar-gutter: stable; }
  .voicing-list > button { box-sizing: border-box; display: grid; place-items: center; width: 100%; min-width: 0; height: 58px; padding: 7px 10px; border-color: var(--border); background: var(--surface-raised); }
  .voicing-list > button.selected { border-color: var(--instrument-chord-color); box-shadow: 0 0 0 1px var(--instrument-chord-color); }
  .mini-neck { position: relative; display: grid; align-content: center; width: 100%; }
  .mini-neck > i { position: relative; height: 7px; border-right: 1px solid var(--border-strong); border-left: 2px solid var(--border-strong); background: linear-gradient(transparent 43%, var(--muted) 44% 56%, transparent 57%), repeating-linear-gradient(90deg, transparent 0 19%, var(--border-strong) 20% 21%, transparent 22% 39%); }
  .mini-neck b { position: absolute; z-index: 2; top: 50%; width: 6px; height: 6px; transform: translate(-50%, -50%); border-radius: 50%; background: var(--instrument-chord-color); }
  .mini-neck em { position: absolute; left: -7px; top: 0; color: var(--muted); font: 800 .44rem/1 monospace; }
  .voicing-omissions, .voicing-empty { margin: 0; color: var(--muted); font-size: .58rem; text-align: center; }
  .voicing-empty { font-size: .55rem; }
  .voicing-empty { align-self: center; }
  @media (max-width: 1080px) {
    .fretboard-body { grid-template-columns: minmax(0, 1fr) 98px; gap: 8px; }
    .neck-string, .fret-numbers { grid-template-columns: 19px 27px repeat(5, minmax(26px, 1fr)); }
    .neck-markers { left: 46px; grid-template-columns: repeat(5, minmax(26px, 1fr)); }
  }
  @media (prefers-reduced-motion: no-preference) {
    .voicing-list > button { transition: border-color 120ms ease, box-shadow 120ms ease; }
  }
</style>
