<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { MessageKey } from "./i18n";
  import type { MeterPeakHold, MeterUnit, SpectrumRange, SpectrumStyle, VisualizationKind, VisualizationResponse, VisualizationSetting } from "./types";
  import { decibelsFromLevel, linePath, meterPeakHoldMilliseconds, spectrumRangeSlice, visualizationKinds, type EnergyPoint } from "./visualization";

  export let kind: VisualizationKind;
  export let bands: number[];
  export let peakLeft: number;
  export let peakRight: number;
  export let heldPeakLeft: number;
  export let heldPeakRight: number;
  export let history: EnergyPoint[];
  export let spectrumStyle: SpectrumStyle;
  export let spectrumRange: SpectrumRange;
  export let response: VisualizationResponse;
  export let meterUnit: MeterUnit;
  export let meterPeakHold: MeterPeakHold;
  export let energyWindowSeconds: number;
  export let t: (key: MessageKey) => string;
  export let onKindChange: (kind: VisualizationKind) => void;
  export let onSettingChange: (setting: VisualizationSetting, value: string | number) => void;

  const kindLabels: Record<VisualizationKind, MessageKey> = { spectrum: "spectrum", meter: "stereoMeter", energy: "energyHistory" };

  $: visibleBands = spectrumRangeSlice(bands, spectrumRange);
  $: spectrumPath = linePath(visibleBands);
  $: visibleHistory = history.filter((point) => point.timestampMs >= Date.now() - energyWindowSeconds * 1_000);
  $: leftHistoryPath = linePath(visibleHistory.map((point) => point.left));
  $: rightHistoryPath = linePath(visibleHistory.map((point) => point.right));
  $: holdMilliseconds = meterPeakHoldMilliseconds(meterPeakHold);

  function setting(event: Event, name: VisualizationSetting): void {
    onSettingChange(name, (event.currentTarget as HTMLSelectElement).value);
  }
  function meterText(value: number): string {
    return meterUnit === "dbfs" ? `${decibelsFromLevel(value).toFixed(1)} dB` : `${Math.round(value * 100)}%`;
  }
</script>

<div class="panel visualization-panel">
  <div class="visualization-header">
    <select class="visualization-kind" aria-label={t("visualizationType")} value={kind} onchange={(event) => onKindChange(event.currentTarget.value as VisualizationKind)}>
      {#each visualizationKinds as option}<option value={option}>{t(kindLabels[option])}</option>{/each}
    </select>
    <details class="visualization-settings">
      <summary aria-label={t("preferences")} title={t("preferences")}><Icon name="sliders" size="13px" /></summary>
      <div class="visualization-settings-menu">
        {#if kind === "spectrum"}
        <select aria-label={t("spectrumStyle")} value={spectrumStyle} onchange={(event) => setting(event, "spectrumStyle")}><option value="bars">{t("spectrumBars")}</option><option value="curve">{t("spectrumCurve")}</option></select>
        <select aria-label={t("frequencyRange")} value={spectrumRange} onchange={(event) => setting(event, "spectrumRange")}><option value="full">{t("rangeFull")}</option><option value="low">{t("rangeLow")}</option><option value="mid">{t("rangeMid")}</option><option value="high">{t("rangeHigh")}</option></select>
        <select aria-label={t("visualizationResponse")} value={response} onchange={(event) => setting(event, "visualizationResponse")}><option value="fast">{t("responseFast")}</option><option value="normal">{t("responseNormal")}</option><option value="smooth">{t("responseSmooth")}</option></select>
        {:else if kind === "meter"}
        <select aria-label={t("meterUnit")} value={meterUnit} onchange={(event) => setting(event, "meterUnit")}><option value="percent">%</option><option value="dbfs">dBFS</option></select>
        <select aria-label={t("peakHold")} value={meterPeakHold} onchange={(event) => setting(event, "meterPeakHold")}><option value="off">{t("off")}</option><option value="oneSecond">1 s</option><option value="threeSeconds">3 s</option></select>
        <select aria-label={t("visualizationResponse")} value={response} onchange={(event) => setting(event, "visualizationResponse")}><option value="fast">{t("responseFast")}</option><option value="normal">{t("responseNormal")}</option><option value="smooth">{t("responseSmooth")}</option></select>
        {:else}
        <select aria-label={t("historyDuration")} value={energyWindowSeconds} onchange={(event) => onSettingChange("energyWindowSeconds", Number(event.currentTarget.value))}><option value={5}>5 s</option><option value={15}>15 s</option><option value={30}>30 s</option></select>
        {/if}
      </div>
    </details>
  </div>

  <div class="visualization-body">
    {#if kind === "spectrum"}
      {#if spectrumStyle === "bars"}
        <div class="visual-spectrum-bars" aria-label={t("spectrum")}>{#each visibleBands as magnitude}<i style={`--spectrum-level:${Math.max(0.01, magnitude)}`}></i>{/each}</div>
      {:else}
        <svg class="visual-line-chart" viewBox="0 0 100 100" preserveAspectRatio="none" aria-label={t("spectrum")}><path class="area" d={`${spectrumPath} L100,100 L0,100 Z`}></path><path d={spectrumPath}></path></svg>
      {/if}
      <div class="visual-scale"><span>30 Hz</span><span>1 kHz</span><span>20 kHz</span></div>
    {:else if kind === "meter"}
      <div class="visual-meter-channels">
        {#each [["L", peakLeft, heldPeakLeft], ["R", peakRight, heldPeakRight]] as channel}
          <div class="visual-meter-channel"><span>{channel[0]}</span><div class="visual-meter-track" role="meter" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(Number(channel[1]) * 100)}><i style={`width:${Number(channel[1]) * 100}%`}></i>{#if holdMilliseconds}<b style={`left:${Number(channel[2]) * 100}%`}></b>{/if}</div><output>{meterText(Number(channel[1]))}</output></div>
        {/each}
      </div>
    {:else}
      <svg class="visual-line-chart energy-chart" viewBox="0 0 100 100" preserveAspectRatio="none" aria-label={t("energyHistory")}><path class="energy-left" d={leftHistoryPath}></path><path class="energy-right" d={rightHistoryPath}></path></svg>
      <div class="energy-legend"><span><i></i>L</span><span><i></i>R</span><small>{energyWindowSeconds} s</small></div>
    {/if}
  </div>
</div>

<style>
  .visualization-panel { display: flex; flex-direction: column; min-height: 0; padding: 10px 12px 9px; overflow: hidden; }
  .visualization-header { display: flex; align-items: center; justify-content: space-between; min-height: 26px; margin-bottom: 5px; gap: 8px; }
  select { height: 24px; min-width: 0; padding: 2px 20px 2px 6px; border: 1px solid var(--border); border-radius: 5px; color: var(--muted); background: var(--surface); font-size: .57rem; }
  .visualization-kind { flex: 1 1 210px; width: 210px; max-width: 210px; color: var(--panel-title); font-size: .67rem; font-weight: 760; letter-spacing: .08em; text-transform: uppercase; }
  .visualization-settings { position: relative; flex: 0 0 auto; }
  .visualization-settings summary { display: grid; width: 24px; height: 24px; padding: 0; place-items: center; border: 1px solid var(--border); border-radius: 5px; color: var(--muted); background: var(--surface); cursor: pointer; font-size: .7rem; list-style: none; }
  .visualization-settings summary::-webkit-details-marker { display: none; }
  .visualization-settings[open] summary { border-color: var(--accent-border); color: var(--accent-strong); background: var(--accent-soft); }
  .visualization-settings-menu { position: absolute; z-index: 20; top: 28px; right: 0; display: grid; width: 150px; padding: 6px; gap: 5px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface-raised); box-shadow: var(--shadow-menu); }
  .visualization-settings-menu select { width: 100%; max-width: none; }
  .visualization-body { position: relative; display: flex; flex: 1; flex-direction: column; min-height: 0; }
  .visual-spectrum-bars { display: flex; flex: 1; align-items: flex-end; min-height: 0; padding: 5px 4px 0; gap: 2px; overflow: hidden; border-bottom: 1px solid var(--border-strong); background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 8%, transparent), transparent), repeating-linear-gradient(0deg, transparent 0 24%, color-mix(in srgb, var(--muted) 10%, transparent) 24% 25%); }
  .visual-spectrum-bars i { flex: 1; min-width: 1px; height: 100%; border-radius: 2px 2px 0 0; background: linear-gradient(180deg, var(--gold), var(--accent)); transform: scaleY(var(--spectrum-level)); transform-origin: bottom; }
  .visual-line-chart { flex: 1; width: 100%; min-height: 0; overflow: visible; border-bottom: 1px solid var(--border-strong); background: repeating-linear-gradient(0deg, transparent 0 24%, color-mix(in srgb, var(--muted) 10%, transparent) 24% 25%); }
  .visual-line-chart path { fill: none; stroke: var(--accent); stroke-width: 1.4; vector-effect: non-scaling-stroke; }
  .visual-line-chart path.area { fill: color-mix(in srgb, var(--accent) 18%, transparent); stroke: none; }
  .visual-scale { display: flex; justify-content: space-between; padding: 3px 3px 0; color: var(--muted); font-size: .52rem; }
  .visual-meter-channels { display: grid; align-content: center; gap: 13px; flex: 1; }
  .visual-meter-channel { display: grid; grid-template-columns: 14px minmax(0, 1fr) 49px; align-items: center; gap: 7px; }
  .visual-meter-channel > span { color: var(--panel-title); font-size: .66rem; font-weight: 800; }
  .visual-meter-channel output { color: var(--muted); font-size: .55rem; text-align: right; }
  .visual-meter-track { position: relative; height: 9px; border: 1px solid var(--border-strong); border-radius: 999px; background: var(--meter-track); }
  .visual-meter-track i { display: block; width: 0; height: 100%; border-radius: inherit; background: linear-gradient(90deg, var(--accent) 0 72%, var(--gold) 86%, var(--danger) 100%); }
  .visual-meter-track b { position: absolute; top: -3px; bottom: -3px; width: 1px; background: var(--text-strong); }
  .energy-chart path.energy-left { stroke: var(--accent); }
  .energy-chart path.energy-right { stroke: var(--gold); }
  .energy-legend { display: flex; align-items: center; padding-top: 3px; gap: 8px; color: var(--muted); font-size: .52rem; }
  .energy-legend span { display: flex; align-items: center; gap: 3px; }
  .energy-legend i { width: 8px; height: 2px; background: var(--accent); }
  .energy-legend span:nth-child(2) i { background: var(--gold); }
  .energy-legend small { margin-left: auto; }
</style>
