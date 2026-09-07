<script lang="ts">
  import sonarcanLogo from "../../docs/assets/sonarcan-rounded.png";
  import { formatInstallBytes, modelInstallationMessage, type ModelInstallationCopy, type ModelInstallProgress } from "./modelInstallation";

  export let progress: ModelInstallProgress;
  export let copy: ModelInstallationCopy;
  export let error = "";
  export let retry: () => void;
  export let quit: () => void;

  $: percent = Math.round(Math.max(0, Math.min(1, progress.progress)) * 100);
</script>

<section class="model-installation" role={error ? "alert" : "status"} aria-live="polite">
  <span class="model-installation-mark" aria-hidden="true"><img src={sonarcanLogo} alt="" /></span>
  <div class="model-installation-heading">
    <strong>{copy.title}</strong>
    <small>{copy.introduction}</small>
  </div>
  <div class="model-installation-status">
    <span>{modelInstallationMessage(progress, copy)}</span>
    {#if progress.modelCount > 0}<small>{progress.modelIndex}/{progress.modelCount}</small>{/if}
  </div>
  <div class="model-installation-progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={percent}>
    <i style={`width:${percent}%`}></i>
  </div>
  <div class="model-installation-totals">
    <span>{percent}%</span>
    {#if progress.totalBytes > 0}<span>{formatInstallBytes(progress.completedBytes)} / {formatInstallBytes(progress.totalBytes)}</span>{/if}
  </div>
  {#if error}
    <p>{copy.failure}</p>
    <details><summary>{error}</summary></details>
    <div class="model-installation-actions">
      <button class="primary" onclick={retry}>{copy.retry}</button>
      <button onclick={quit}>{copy.quit}</button>
    </div>
  {/if}
</section>
