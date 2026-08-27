<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { createProject, diagnostics, getWaveform, importAudio, openProject, renameProject, renameTrack, saveProjectAs } from "./lib/backend";
  import type { DiagnosticsSnapshot, ProjectSummary, TrackSummary, WaveformData, WaveformPeak } from "./lib/types";

  let project: ProjectSummary | null = null;
  let diagnosticInfo: DiagnosticsSnapshot | null = null;
  let errorMessage = "";
  let busy = false;
  let audioElement: HTMLAudioElement;
  let currentTrack: TrackSummary | null = null;
  let isPlaying = false;
  let currentSeconds = 0;
  let durationSeconds = 0;
  let playbackRate = 1;
  let volume = 0.8;
  let loopA: number | null = null;
  let loopB: number | null = null;
  let recentProjects: string[] = [];
  let waveform: WaveformData | null = null;
  let waveformLoading = false;
  let waveformZoom = 1;
  let waveformStart = 0;
  let dragStartX = 0;
  let dragStartViewport = 0;
  let dragMoved = false;

  $: detailedPeaks = visiblePeaks(waveform?.peaks ?? [], waveformZoom, waveformStart, 1_000);
  $: overviewPeaks = visiblePeaks(waveform?.peaks ?? [], 1, 0, 700);
  $: playheadPercent = durationSeconds > 0 ? ((currentSeconds / durationSeconds - waveformStart) * waveformZoom * 100) : 0;

  onMount(() => {
    try {
      recentProjects = JSON.parse(localStorage.getItem("sonarcan.recentProjects") ?? "[]");
    } catch {
      recentProjects = [];
    }
  });

  async function run(action: () => Promise<void>): Promise<void> {
    busy = true;
    errorMessage = "";
    try {
      await action();
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      busy = false;
    }
  }

  function newProject(): void {
    void run(async () => {
      const parentDirectory = await open({ directory: true, multiple: false, title: "Choose a project folder" });
      if (!parentDirectory) return;
      const name = window.prompt("Project name", "My Band")?.trim();
      if (!name) return;
      project = await createProject(name, parentDirectory);
      currentTrack = null;
      rememberProject(project.packagePath);
    });
  }

  function loadProject(): void {
    void run(async () => {
      const packagePath = await open({ directory: true, multiple: false, title: "Open a SonArcan project" });
      if (!packagePath) return;
      project = await openProject(packagePath);
      currentTrack = null;
      rememberProject(project.packagePath);
    });
  }

  function openRecent(packagePath: string): void {
    void run(async () => {
      project = await openProject(packagePath);
      currentTrack = null;
      rememberProject(project.packagePath);
    });
  }

  function renameCurrentProject(): void {
    if (!project) return;
    const name = window.prompt("Project name", project.name)?.trim();
    if (!name || name === project.name) return;
    void run(async () => {
      project = await renameProject(project!.packagePath, name);
    });
  }

  function renamePlaylistTrack(track: TrackSummary): void {
    if (!project) return;
    const name = window.prompt("Track name", track.title)?.trim();
    if (!name || name === track.title) return;
    void run(async () => {
      project = await renameTrack(project!.packagePath, track.id, name);
      if (currentTrack?.id === track.id) currentTrack = project.tracks.find((item) => item.id === track.id) ?? null;
    });
  }

  function saveAs(): void {
    if (!project) return;
    void run(async () => {
      const parentDirectory = await open({ directory: true, multiple: false, title: "Choose the destination folder" });
      if (!parentDirectory) return;
      const name = window.prompt("Name for the project copy", `${project!.name} Copy`)?.trim();
      if (!name) return;
      project = await saveProjectAs(project!.packagePath, parentDirectory, name);
      currentTrack = null;
      rememberProject(project.packagePath);
    });
  }

  function rememberProject(packagePath: string): void {
    recentProjects = [packagePath, ...recentProjects.filter((path) => path !== packagePath)].slice(0, 10);
    localStorage.setItem("sonarcan.recentProjects", JSON.stringify(recentProjects));
  }

  function addTracks(): void {
    if (!project) return;
    void run(async () => {
      const selected = await open({
        multiple: true,
        title: "Import audio",
        filters: [{ name: "Supported audio", extensions: ["wav", "mp3", "flac"] }],
      });
      if (!selected) return;
      const sourcePaths = Array.isArray(selected) ? selected : [selected];
      project = await importAudio(project!.packagePath, sourcePaths);
      if (!currentTrack && project.tracks.length > 0) selectTrack(project.tracks[0], false);
    });
  }

  function showDiagnostics(): void {
    void run(async () => {
      diagnosticInfo = await diagnostics();
    });
  }

  function selectTrack(track: TrackSummary, autoplay = true): void {
    currentTrack = track;
    currentSeconds = 0;
    durationSeconds = track.durationSeconds ?? 0;
    loopA = null;
    loopB = null;
    audioElement.src = convertFileSrc(track.sourcePath);
    audioElement.playbackRate = playbackRate;
    audioElement.preservesPitch = true;
    audioElement.volume = volume;
    audioElement.load();
    void loadTrackWaveform(track);
    if (autoplay) void play();
  }

  async function loadTrackWaveform(track: TrackSummary): Promise<void> {
    if (!project) return;
    waveformLoading = true;
    waveform = null;
    waveformZoom = 1;
    waveformStart = 0;
    try {
      const loaded = await getWaveform(project.packagePath, track.id);
      if (currentTrack?.id === track.id) {
        waveform = loaded;
        if (loaded.durationSeconds > 0) durationSeconds = loaded.durationSeconds;
      }
    } catch (error) {
      errorMessage = `Waveform generation failed: ${error instanceof Error ? error.message : String(error)}`;
    } finally {
      if (currentTrack?.id === track.id) waveformLoading = false;
    }
  }

  async function play(): Promise<void> {
    if (!currentTrack && project?.tracks.length) selectTrack(project.tracks[0], false);
    if (!audioElement.src) return;
    try {
      await audioElement.play();
    } catch (error) {
      errorMessage = `Playback failed: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  function togglePlayback(): void {
    if (audioElement.paused) void play();
    else audioElement.pause();
  }

  function moveTrack(offset: number): void {
    if (!project?.tracks.length) return;
    const index = Math.max(0, project.tracks.findIndex((track) => track.id === currentTrack?.id));
    const nextIndex = (index + offset + project.tracks.length) % project.tracks.length;
    selectTrack(project.tracks[nextIndex]);
  }

  function formatTime(value: number): string {
    if (!Number.isFinite(value) || value < 0) return "00:00";
    const minutes = Math.floor(value / 60);
    const seconds = Math.floor(value % 60);
    return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }

  function updatePosition(): void {
    currentSeconds = audioElement.currentTime;
    if (loopA !== null && loopB !== null && currentSeconds >= loopB) {
      audioElement.currentTime = loopA;
      if (audioElement.paused) void play();
    }
  }

  function seek(position: number): void {
    if (!Number.isFinite(position)) return;
    audioElement.currentTime = Math.max(0, Math.min(position, durationSeconds));
    currentSeconds = audioElement.currentTime;
  }

  function jump(seconds: number): void {
    seek(audioElement.currentTime + seconds);
  }

  function setLoopA(): void {
    loopA = audioElement.currentTime;
    if (loopB !== null && loopB <= loopA) loopB = null;
  }

  function setLoopB(): void {
    if (loopA === null) loopA = 0;
    if (audioElement.currentTime > loopA) loopB = audioElement.currentTime;
  }

  function clearLoop(): void {
    loopA = null;
    loopB = null;
  }

  function changeRate(delta: number): void {
    playbackRate = Math.max(0.5, Math.min(2, Math.round((playbackRate + delta) * 20) / 20));
    audioElement.playbackRate = playbackRate;
    audioElement.preservesPitch = true;
  }

  function changeVolume(value: number): void {
    volume = Math.max(0, Math.min(1, value));
    audioElement.volume = volume;
  }

  function visiblePeaks(source: WaveformPeak[], zoom: number, start: number, maximum: number): WaveformPeak[] {
    if (source.length === 0) return [];
    const first = Math.floor(start * source.length);
    const count = Math.max(1, Math.ceil(source.length / zoom));
    const selection = source.slice(first, Math.min(source.length, first + count));
    const groupSize = Math.max(1, Math.ceil(selection.length / maximum));
    const result: WaveformPeak[] = [];
    for (let index = 0; index < selection.length; index += groupSize) {
      const group = selection.slice(index, index + groupSize);
      result.push({
        min: Math.min(...group.map((peak) => peak.min)),
        max: Math.max(...group.map((peak) => peak.max)),
      });
    }
    return result;
  }

  function zoomWaveform(event: WheelEvent): void {
    event.preventDefault();
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const anchor = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    const oldSpan = 1 / waveformZoom;
    const anchorPosition = waveformStart + anchor * oldSpan;
    const factor = Math.exp(-event.deltaY * 0.002);
    const nextZoom = Math.max(1, Math.min(128, waveformZoom * factor));
    const nextSpan = 1 / nextZoom;
    waveformStart = Math.max(0, Math.min(1 - nextSpan, anchorPosition - anchor * nextSpan));
    waveformZoom = nextZoom;
  }

  function startWaveformDrag(event: PointerEvent): void {
    dragStartX = event.clientX;
    dragStartViewport = waveformStart;
    dragMoved = false;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function dragWaveform(event: PointerEvent): void {
    if (!(event.currentTarget as HTMLElement).hasPointerCapture(event.pointerId)) return;
    const width = (event.currentTarget as HTMLElement).clientWidth;
    const delta = event.clientX - dragStartX;
    if (Math.abs(delta) > 3) dragMoved = true;
    const span = 1 / waveformZoom;
    waveformStart = Math.max(0, Math.min(1 - span, dragStartViewport - delta / width * span));
  }

  function finishWaveformDrag(event: PointerEvent): void {
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    if (!dragMoved) {
      const bounds = target.getBoundingClientRect();
      const local = (event.clientX - bounds.left) / bounds.width;
      seek((waveformStart + local / waveformZoom) * durationSeconds);
    }
  }

  function seekFromOverview(event: PointerEvent): void {
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    seek(ratio * durationSeconds);
    const span = 1 / waveformZoom;
    waveformStart = Math.max(0, Math.min(1 - span, ratio - span / 2));
  }
</script>

<audio
  bind:this={audioElement}
  onplay={() => isPlaying = true}
  onpause={() => isPlaying = false}
  ontimeupdate={updatePosition}
  ondurationchange={() => durationSeconds = Number.isFinite(audioElement.duration) ? audioElement.duration : 0}
  onended={() => moveTrack(1)}
  onerror={() => errorMessage = currentTrack ? `Unable to play ${currentTrack.title}.` : "Unable to play this audio file."}
></audio>

<svelte:head><title>SonArcan — Dive into the music.</title></svelte:head>

<main class="shell">
  <header class="topbar">
    <div>
      <span class="eyebrow">SONARCAN</span>
      <h1>{project?.name ?? "Dive into the music."}</h1>
    </div>
    <nav aria-label="Project actions">
      <details class="file-menu">
        <summary>File</summary>
        <div class="menu-panel">
          <button onclick={newProject}>New Project…</button>
          <button onclick={loadProject}>Open…</button>
          <button onclick={saveAs} disabled={!project}>Save As…</button>
          <button onclick={renameCurrentProject} disabled={!project}>Rename Project…</button>
          {#if recentProjects.length > 0}
            <hr />
            <small>OPEN RECENT</small>
            {#each recentProjects as path}
              <button class="recent" title={path} onclick={() => openRecent(path)}>{path.split("/").at(-1)}</button>
            {/each}
          {/if}
        </div>
      </details>
      <button onclick={newProject} disabled={busy}>New project</button>
      <button onclick={loadProject} disabled={busy}>Open</button>
      <button class="primary" onclick={addTracks} disabled={busy || !project}>Import audio</button>
    </nav>
  </header>

  {#if errorMessage}<div class="error" role="alert">{errorMessage}</div>{/if}

  <section class="workspace">
    <aside class="playlist panel">
      <div class="panel-title"><h2>Playlist</h2><span>{project?.trackCount ?? 0}</span></div>
      {#if project && project.tracks.length > 0}
        <ol>
          {#each project.tracks as track, index}
            <li class:active={track.id === currentTrack?.id}>
              <button class="track-button" onclick={() => selectTrack(track)} ondblclick={() => renamePlaylistTrack(track)} title="Double-click to rename">
                <span class="track-number">{String(index + 1).padStart(2, "0")}</span>
                <span><strong>{track.title}</strong><small>{track.format.toUpperCase()} · {track.sampleRate ? `${track.sampleRate} Hz` : "Unknown rate"}</small></span>
              </button>
              <button class="rename-track" onclick={() => renamePlaylistTrack(track)} aria-label={`Rename ${track.title}`}>✎</button>
            </li>
          {/each}
        </ol>
      {:else}
        <div class="empty">Create or open a project, then import WAV, MP3, or FLAC files.</div>
      {/if}
    </aside>

    <section class="main-stage">
      <div class="visualizer panel">
        <div class="panel-title"><h2>Waveform</h2><span>Phase 0</span></div>
        <div
          class="wave detailed-wave"
          class:dragging={dragMoved}
          aria-label="Zoomable waveform"
          onwheel={zoomWaveform}
          onpointerdown={startWaveformDrag}
          onpointermove={dragWaveform}
          onpointerup={finishWaveformDrag}
        >
          {#if waveformLoading}<span class="wave-message">Generating waveform…</span>
          {:else if detailedPeaks.length === 0}<span class="wave-message">Select a track to generate its waveform.</span>
          {:else}
            <svg viewBox={`0 0 ${detailedPeaks.length} 100`} preserveAspectRatio="none" aria-hidden="true">
              {#each detailedPeaks as peak, index}
                <line x1={index} x2={index} y1={50 - peak.max * 48} y2={50 - peak.min * 48} />
              {/each}
            </svg>
            {#if playheadPercent >= 0 && playheadPercent <= 100}<i class="playhead" style={`left:${playheadPercent}%`}></i>{/if}
            {#if loopA !== null}<i class="loop-marker a" style={`left:${(loopA / durationSeconds - waveformStart) * waveformZoom * 100}%`}>A</i>{/if}
            {#if loopB !== null}<i class="loop-marker b" style={`left:${(loopB / durationSeconds - waveformStart) * waveformZoom * 100}%`}>B</i>{/if}
          {/if}
        </div>
        <div class="zoom-info"><span>{waveformZoom.toFixed(1)}× zoom</span><span>Wheel/pinch to zoom · drag to navigate · click to seek</span></div>
        <div class="overview-wave" aria-label="Full song waveform" onpointerdown={seekFromOverview}>
          {#if overviewPeaks.length > 0}
            <svg viewBox={`0 0 ${overviewPeaks.length} 60`} preserveAspectRatio="none" aria-hidden="true">
              {#each overviewPeaks as peak, index}
                <line x1={index} x2={index} y1={30 - peak.max * 28} y2={30 - peak.min * 28} />
              {/each}
            </svg>
            <i class="viewport" style={`left:${waveformStart * 100}%;width:${100 / waveformZoom}%`}></i>
            <i class="overview-playhead" style={`left:${durationSeconds ? currentSeconds / durationSeconds * 100 : 0}%`}></i>
          {/if}
        </div>
        <div class="timeline"><span>00:00</span><span>{formatTime(durationSeconds * .25)}</span><span>{formatTime(durationSeconds * .5)}</span><span>{formatTime(durationSeconds * .75)}</span><span>{formatTime(durationSeconds)}</span></div>
        <input
          class="seek"
          aria-label="Playback position"
          type="range"
          min="0"
          max={durationSeconds || 1}
          step="0.01"
          value={currentSeconds}
          oninput={(event) => seek(Number(event.currentTarget.value))}
        />
        <div class="loop-status">
          <span>A {loopA === null ? "—" : formatTime(loopA)}</span>
          <span>{loopA !== null && loopB !== null ? `Loop ${formatTime(loopB - loopA)}` : "No active loop"}</span>
          <span>B {loopB === null ? "—" : formatTime(loopB)}</span>
        </div>
      </div>

      <div class="transport panel">
        <button aria-label="Jump back five seconds" onclick={() => jump(-5)}>−5s</button>
        <button class="round" aria-label="Previous" onclick={() => moveTrack(-1)}>◀</button>
        <button class="play" aria-label={isPlaying ? "Pause" : "Play"} onclick={togglePlayback}>{isPlaying ? "Ⅱ" : "▶"}</button>
        <button class="round" aria-label="Next" onclick={() => moveTrack(1)}>▶</button>
        <button aria-label="Jump forward five seconds" onclick={() => jump(5)}>+5s</button>
        <div class="readout"><small>POSITION</small><strong>{formatTime(currentSeconds)} / {formatTime(durationSeconds)}</strong></div>
      </div>

      <div class="practice panel">
        <div class="control-group"><button onclick={setLoopA}>Set A</button><button onclick={clearLoop}>Clear</button><button onclick={setLoopB}>Set B</button></div>
        <div class="control-group"><button onclick={() => changeRate(-0.05)}>−</button><div class="readout"><small>TEMPO</small><strong>{Math.round(playbackRate * 100)}%</strong></div><button onclick={() => changeRate(0.05)}>+</button></div>
        <div class="control-group"><button disabled title="Pitch shifting requires the Rust DSP engine">♭</button><div class="readout"><small>PITCH</small><strong>0 st</strong></div><button disabled title="Pitch shifting requires the Rust DSP engine">♯</button></div>
        <label class="volume">Volume<input aria-label="Volume" type="range" min="0" max="1" step="0.01" value={volume} oninput={(event) => changeVolume(Number(event.currentTarget.value))} /></label>
      </div>

      <div class="lower-grid">
        <div class="panel"><div class="panel-title"><h2>Stems</h2><span>Idle</span></div><div class="chips"><button>Mix</button><button>Vocals</button><button>Bass</button><button>Drums</button></div></div>
        <div class="panel"><div class="panel-title"><h2>Chords</h2><span>Not analyzed</span></div><div class="chords"><b>Am7</b><b>Fmaj7</b><b>C</b><b>G</b></div></div>
      </div>
    </section>
  </section>

  <footer><span>{busy ? "Working…" : "Ready"}</span><button class="link" onclick={showDiagnostics}>Diagnostics</button></footer>

  {#if diagnosticInfo}
    <dialog open><h2>Diagnostics</h2><dl><dt>Version</dt><dd>{diagnosticInfo.appVersion}</dd><dt>OS</dt><dd>{diagnosticInfo.os}</dd><dt>Architecture</dt><dd>{diagnosticInfo.architecture}</dd><dt>Logging</dt><dd>{diagnosticInfo.rustLog}</dd></dl><button onclick={() => diagnosticInfo = null}>Close</button></dialog>
  {/if}
</main>
