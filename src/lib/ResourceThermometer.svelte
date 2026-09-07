<script lang="ts">
  import type { SystemMetrics } from "./types";
  import Icon from "./Icon.svelte";
  import { activeResourceLeds, formatResourceMemory, resourceLoad, resourcePressure } from "./resourceMetrics";

  let { metrics, label }: { metrics: SystemMetrics; label: string } = $props();
  const ledCount = 14;
  const ledLevels = Array.from({ length: ledCount }, (_, index) => index + 1);
  let load = $derived(resourceLoad(metrics));
  let pressure = $derived(resourcePressure(load));
  let expanded = $state(false);
  let rows = $derived([
    { label: "CPU", value: metrics.cpuPercent, detail: metrics.cpuPercent === null ? "—" : `${metrics.cpuPercent.toFixed(1)}%` },
    { label: "GPU", value: metrics.gpuPercent, detail: metrics.gpuPercent === null ? "—" : `${metrics.gpuPercent.toFixed(1)}%` },
    { label: "RAM", value: metrics.memoryPercent, detail: metrics.memoryPercent === null ? formatResourceMemory(metrics.memoryMegabytes) : `${formatResourceMemory(metrics.memoryMegabytes)} · ${metrics.memoryPercent.toFixed(1)}%` },
  ]);
  let accessibleValue = $derived(rows.map((row) => `${row.label} ${row.detail}`).join(", "));
</script>

<button
  type="button"
  class={`resource-thermometer ${pressure}`}
  aria-label={`${label}: ${accessibleValue}`}
  aria-describedby="resource-meter-tooltip"
  aria-expanded={expanded}
  onclick={() => expanded = !expanded}
  onblur={() => expanded = false}
  onkeydown={(event) => { if (event.key === "Escape") expanded = false; }}
>
  <span class="resource-icon" aria-hidden="true"><Icon name="microchip" size="19px" /></span>
  <div class="resource-meter-tooltip" id="resource-meter-tooltip" role="tooltip">
    {#each rows as row}
      <div class="resource-meter-row">
        <strong>{row.label}</strong>
        <span class="resource-led-meter" aria-hidden="true">
          {#each ledLevels as level}<i class:active={activeResourceLeds(row.value, ledCount) >= level}></i>{/each}
        </span>
        <output>{row.detail}</output>
      </div>
    {/each}
  </div>
</button>

<style>
  .resource-thermometer { --pressure-color: #52c8bd; position: relative; display: grid; width: 24px; min-width: 24px; height: 36px; padding: 0; place-items: center; border: 0; outline: none; background: transparent; cursor: help; }
  .resource-thermometer.warm { --pressure-color: #e0ad52; }
  .resource-thermometer.hot { --pressure-color: #ed6571; }
  .resource-icon { display: grid; place-items: center; color: var(--pressure-color); filter: drop-shadow(0 0 5px color-mix(in srgb, var(--pressure-color) 55%, transparent)); transition: color .25s ease, filter .25s ease; }
  .resource-meter-tooltip { position: absolute; z-index: 1200; top: calc(100% + 9px); right: -5px; display: grid; width: 310px; padding: 11px 12px; gap: 9px; border: 1px solid #52666e; border-radius: 9px; opacity: 0; visibility: hidden; color: #d8e4e7; background: #10181bcc; box-shadow: 0 12px 34px #000b; backdrop-filter: blur(14px); transform: translateY(-4px); transition: opacity .14s ease .18s, transform .14s ease .18s, visibility 0s linear .32s; pointer-events: none; }
  .resource-meter-tooltip::before { position: absolute; right: 10px; bottom: 100%; border: 6px solid transparent; border-bottom-color: #52666e; content: ""; }
  .resource-thermometer:hover .resource-meter-tooltip, .resource-thermometer:focus-visible .resource-meter-tooltip, .resource-thermometer[aria-expanded="true"] .resource-meter-tooltip { opacity: 1; visibility: visible; transform: translateY(0); transition-delay: .18s; }
  .resource-thermometer:focus-visible .resource-icon { border-radius: 3px; outline: 2px solid var(--accent); outline-offset: 3px; }
  .resource-meter-row { display: grid; grid-template-columns: 31px minmax(110px, 1fr) 92px; align-items: center; gap: 9px; }
  .resource-meter-row strong { color: #9babb0; font: 750 .58rem/1 ui-monospace, SFMono-Regular, Menlo, monospace; letter-spacing: .08em; }
  .resource-meter-row output { color: #e1e9eb; font: 700 .6rem/1 ui-monospace, SFMono-Regular, Menlo, monospace; font-variant-numeric: tabular-nums; text-align: right; white-space: nowrap; }
  .resource-led-meter { display: grid; grid-template-columns: repeat(14, minmax(3px, 1fr)); gap: 2px; }
  .resource-led-meter i { height: 8px; border-radius: 1px; background: #26343a; box-shadow: inset 0 0 0 1px #314148; }
  .resource-led-meter i.active { background: #4dc6b8; box-shadow: 0 0 4px #4dc6b877; }
  .resource-led-meter i.active:nth-child(n+10) { background: #e0ad52; box-shadow: 0 0 4px #e0ad5277; }
  .resource-led-meter i.active:nth-child(n+13) { background: #ed6571; box-shadow: 0 0 4px #ed657177; }
  @media (prefers-reduced-motion: reduce) {
    .resource-icon, .resource-meter-tooltip { transition: none; }
  }
</style>
