<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { analyzeTempo, audioLoad, audioPause, audioPlay, audioPreload, audioSeek, audioSetBeatGrid, audioSetEndBehavior, audioSetLoop, audioSetLoopTrainer, audioSetMetronome, audioSetPitch, audioSetPlaybackRate, audioSetVolume, audioSpectrum, audioStatus, createProject, diagnostics, getWaveform, importAudio, listRecentProjects, openProject, renameProject, renameTrack, reorderTrack, saveProjectAs, setApplicationLanguage, stemDisable, stemSetMix, stemStart, stemStatus, updatePracticeState } from "./lib/backend";
  import { systemLanguage, translate, type Language, type MessageKey } from "./lib/i18n";
  import NumericControl from "./lib/NumericControl.svelte";
  import type { DiagnosticsSnapshot, EndBehavior, ProjectSummary, StemMix, StemStatus, TempoAnalysis, TrackSummary, WaveformData, WaveformPeak } from "./lib/types";

  let project: ProjectSummary | null = null;
  let diagnosticInfo: DiagnosticsSnapshot | null = null;
  let errorMessage = "";
  let busy = false;
  let currentTrack: TrackSummary | null = null;
  let isPlaying = false;
  let currentSeconds = 0;
  let durationSeconds = 0;
  let playbackRate = 1;
  let pitchSemitones = 0;
  let volume = 0.8;
  let volumeBeforeMute = 0.8;
  let loopEnabled = false;
  let loopA: number | null = null;
  let loopB: number | null = null;
  let preferencesVisible = false;
  let shortcutsVisible = false;
  let waveform: WaveformData | null = null;
  let waveformLoading = false;
  let audioLoading = false;
  let loadingTrackId: string | null = null;
  let tempoLoading = false;
  let detectedBpm: number | null = null;
  let gridBpm: number | null = null;
  let beatGridOffsetSeconds = 0;
  let metronomeEnabled = false;
  let metronomeVolume = 0.55;
  let tapTimes: number[] = [];
  let trainerEnabled = false;
  let trainerRepetitions = 3;
  let trainerIncrement = 0.05;
  let trainerTargetRate = 1;
  let trainerLoopCount = 0;
  let spectrumBands = Array<number>(64).fill(0);
  let spectrumRequestActive = false;
  let stems: StemStatus = { state: "disabled", progress: 0, stage: "disabled", trackId: null, cached: false, error: null };
  let stemMix: StemMix[] = Array.from({ length: 4 }, () => ({ gain: 1, muted: false, soloed: false }));
  let stemStatusRequestActive = false;
  let editingTrackId: string | null = null;
  let editingTrackTitle = "";
  let draggedTrackId: string | null = null;
  let dropTrackId: string | null = null;
  let endBehavior: EndBehavior = "stop";
  let endedGeneration = 0;
  let waveformZoom = 1;
  let waveformStart = 0;
  let dragStartX = 0;
  let dragStartViewport = 0;
  let dragMoved = false;
  let practiceSaveTimer: number | undefined;
  let statusRequestActive = false;
  let trackSelectionGeneration = 0;
  const waveformCache = new Map<string, WaveformData>();
  const tempoCache = new Map<string, TempoAnalysis>();
  const loadingWave = Array.from({ length: 72 }, (_, index) => Math.min(0.95, 0.12 + Math.abs(Math.sin(index * 0.71) * Math.cos(index * 0.17)) * 0.78));
  const warmedProjects = new Set<string>();
  type LoopDragMode = "a" | "b" | "region";
  let loopDrag: { mode: LoopDragMode; pointerId: number; originTime: number; a: number; b: number } | null = null;
  let language: Language = systemLanguage();
  const t = (key: MessageKey): string => translate(language, key);

  function focusOnMount(node: HTMLInputElement): void {
    queueMicrotask(() => {
      node.focus();
      node.select();
    });
  }

  $: detailedPeaks = visiblePeaks(waveform?.peaks ?? [], waveformZoom, waveformStart, 1_000);
  $: overviewPeaks = visiblePeaks(waveform?.peaks ?? [], 1, 0, 700);
  $: playheadPercent = durationSeconds > 0 ? ((currentSeconds / durationSeconds - waveformStart) * waveformZoom * 100) : 0;
  $: detailedBeatLines = beatLines(true);
  $: overviewBeatLines = beatLines(false);

  onMount(() => {
    let unlisten: UnlistenFn | undefined;
    void listen<string>("native-menu", (event) => handleNativeMenu(event.payload)).then((stop) => unlisten = stop);
    const handleKeydown = (event: KeyboardEvent): void => {
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, select, [contenteditable='true']") || !project) return;
      if (event.key.toLowerCase() === "a") { event.preventDefault(); setLoopA(); }
      else if (event.key.toLowerCase() === "b") { event.preventDefault(); setLoopB(); }
      else if (event.key.toLowerCase() === "l") { event.preventDefault(); toggleLoop(); }
      else if (event.key === "Escape") { event.preventDefault(); clearLoop(); }
      else if (target?.matches("button")) return;
      else if (event.code === "Space") { event.preventDefault(); togglePlayback(); }
      else if (event.key === "ArrowLeft") { event.preventDefault(); jump(-5); }
      else if (event.key === "ArrowRight") { event.preventDefault(); jump(5); }
      else if (event.key === "-" || event.key === "_") { event.preventDefault(); changePlaybackRate(-0.05); }
      else if (event.key === "+" || event.key === "=") { event.preventDefault(); changePlaybackRate(0.05); }
      else if (event.key === "[") { event.preventDefault(); changePitch(-1); }
      else if (event.key === "]") { event.preventDefault(); changePitch(1); }
      else if (event.key.toLowerCase() === "m") { event.preventDefault(); toggleMetronome(); }
      else if (event.key.toLowerCase() === "t") { event.preventDefault(); tapTempo(); }
    };
    window.addEventListener("keydown", handleKeydown);
    const statusTimer = window.setInterval(() => void refreshAudioStatus(), 33);
    const spectrumTimer = window.setInterval(() => void refreshSpectrum(), 50);
    const stemTimer = window.setInterval(() => void refreshStemStatus(), 400);
    const savedLanguage = localStorage.getItem("sonarcan.language");
    if (savedLanguage === "en" || savedLanguage === "fr") language = savedLanguage;
    document.documentElement.lang = language;
    void setApplicationLanguage(language);
    const savedEndBehavior = localStorage.getItem("sonarcan.endBehavior");
    if (savedEndBehavior === "restart" || savedEndBehavior === "advance" || savedEndBehavior === "stop") endBehavior = savedEndBehavior;
    void audioSetEndBehavior(endBehavior);
    void restoreLastProject();
    return () => {
      window.removeEventListener("keydown", handleKeydown);
      window.clearInterval(statusTimer);
      window.clearInterval(spectrumTimer);
      window.clearInterval(stemTimer);
      unlisten?.();
    };
  });

  function changeLanguage(nextLanguage: Language): void {
    language = nextLanguage;
    localStorage.setItem("sonarcan.language", language);
    document.documentElement.lang = language;
    void setApplicationLanguage(language);
  }

  async function restoreLastProject(): Promise<void> {
    if (project) return;
    try {
      const [mostRecent] = await listRecentProjects();
      if (!mostRecent || project) return;
      const restored = await openProject(mostRecent);
      if (project) return;
      project = restored;
      const firstTrack = restored.tracks[0];
      if (firstTrack) selectTrack(firstTrack, false);
    } catch {
      // Startup restoration is best-effort. The File menu remains available if
      // the recent project is invalid, inaccessible, or was moved meanwhile.
    }
  }

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
      const parentDirectory = await open({ directory: true, multiple: false, title: t("chooseProjectFolder") });
      if (!parentDirectory) return;
      const name = window.prompt(t("projectName"), t("defaultProjectName"))?.trim();
      if (!name) return;
      project = await createProject(name, parentDirectory);
      currentTrack = null;
    });
  }

  function loadProject(): void {
    void run(async () => {
      const packagePath = await open({ directory: true, multiple: false, title: t("openProject") });
      if (!packagePath) return;
      project = await openProject(packagePath);
      currentTrack = null;
    });
  }

  function openRecent(packagePath: string): void {
    void run(async () => {
      project = await openProject(packagePath);
      currentTrack = null;
    });
  }

  function renameCurrentProject(): void {
    if (!project) return;
    const name = window.prompt(t("projectName"), project.name)?.trim();
    if (!name || name === project.name) return;
    void run(async () => {
      project = await renameProject(project!.packagePath, name);
    });
  }

  function startTrackRename(track: TrackSummary): void {
    editingTrackId = track.id;
    editingTrackTitle = track.title;
  }

  function commitTrackRename(track: TrackSummary): void {
    const name = editingTrackTitle.trim();
    editingTrackId = null;
    if (!project || !name || name === track.title) return;
    void run(async () => {
      project = await renameTrack(project!.packagePath, track.id, name);
      if (currentTrack?.id === track.id) currentTrack = project.tracks.find((item) => item.id === track.id) ?? null;
    });
  }

  function dropTrack(event: DragEvent, newIndex: number): void {
    event.preventDefault();
    const trackId = draggedTrackId;
    draggedTrackId = null;
    dropTrackId = null;
    if (!project || !trackId) return;
    void run(async () => {
      project = await reorderTrack(project!.packagePath, trackId, newIndex);
      if (currentTrack) currentTrack = project.tracks.find((track) => track.id === currentTrack?.id) ?? null;
    });
  }

  function changeEndBehavior(behavior: EndBehavior): void {
    endBehavior = behavior;
    localStorage.setItem("sonarcan.endBehavior", behavior);
    void audioSetEndBehavior(behavior);
  }

  function saveAs(): void {
    if (!project) return;
    void run(async () => {
      const parentDirectory = await open({ directory: true, multiple: false, title: t("chooseDestination") });
      if (!parentDirectory) return;
      const name = window.prompt(t("copyName"), `${project!.name} ${t("copySuffix")}`)?.trim();
      if (!name) return;
      project = await saveProjectAs(project!.packagePath, parentDirectory, name);
      currentTrack = null;
    });
  }

  async function handleNativeMenu(id: string): Promise<void> {
    if (id === "file:new") newProject();
    else if (id === "file:open") loadProject();
    else if (id === "file:import") addTracks();
    else if (id === "file:save_as") saveAs();
    else if (id === "file:rename_project") renameCurrentProject();
    else if (id.startsWith("recent:")) {
      const index = Number(id.slice("recent:".length));
      const recent = await listRecentProjects();
      if (Number.isInteger(index) && recent[index]) openRecent(recent[index]);
    } else if (id === "view:zoom_in") setWaveformZoom(waveformZoom * 1.5);
    else if (id === "view:zoom_out") setWaveformZoom(waveformZoom / 1.5);
    else if (id === "view:zoom_reset") setWaveformZoom(1);
    else if (id === "playback:toggle") togglePlayback();
    else if (id === "playback:back") jump(-5);
    else if (id === "playback:forward") jump(5);
    else if (id === "playback:set_a") setLoopA();
    else if (id === "playback:set_b") setLoopB();
    else if (id === "playback:clear_loop") clearLoop();
    else if (id === "preferences") preferencesVisible = true;
    else if (id === "help:diagnostics") showDiagnostics();
    else if (id === "help:shortcuts") shortcutsVisible = true;
  }

  function setWaveformZoom(nextZoom: number): void {
    const center = waveformStart + 0.5 / waveformZoom;
    waveformZoom = Math.max(1, Math.min(128, nextZoom));
    const span = 1 / waveformZoom;
    waveformStart = Math.max(0, Math.min(1 - span, center - span / 2));
  }

  function addTracks(): void {
    if (!project) return;
    void run(async () => {
      const selected = await open({
        multiple: true,
        title: t("importAudio"),
        filters: [{ name: t("supportedAudio"), extensions: ["wav", "mp3", "flac"] }],
      });
      if (!selected) return;
      const sourcePaths = Array.isArray(selected) ? selected : [selected];
      project = await importAudio(project!.packagePath, sourcePaths);
      warmedProjects.delete(project.packagePath);
      if (!currentTrack && project.tracks.length > 0) selectTrack(project.tracks[0], false);
      else if (currentTrack) void warmPlaylistCache(project.packagePath, currentTrack.id);
    });
  }

  function showDiagnostics(): void {
    void run(async () => {
      diagnosticInfo = await diagnostics();
    });
  }

  function selectTrack(track: TrackSummary, autoplay = true): void {
    const selectionGeneration = ++trackSelectionGeneration;
    void persistCurrentPracticeState();
    void audioPause();
    void stemDisable();
    stems = { state: "disabled", progress: 0, stage: "disabled", trackId: null, cached: false, error: null };
    isPlaying = false;
    audioLoading = true;
    loadingTrackId = track.id;
    currentTrack = track;
    currentSeconds = track.practice.positionSeconds;
    durationSeconds = track.durationSeconds ?? 0;
    playbackRate = track.practice.playbackRate;
    pitchSemitones = track.practice.pitchSemitones ?? 0;
    volume = track.practice.volume;
    volumeBeforeMute = volume > 0 ? volume : 0.8;
    loopEnabled = track.practice.loopEnabled ?? (track.practice.loopASeconds !== null && track.practice.loopBSeconds !== null);
    loopA = track.practice.loopASeconds;
    loopB = track.practice.loopBSeconds;
    gridBpm = track.practice.gridBpm ?? null;
    beatGridOffsetSeconds = track.practice.beatGridOffsetSeconds ?? 0;
    metronomeEnabled = track.practice.metronomeEnabled ?? false;
    metronomeVolume = track.practice.metronomeVolume ?? 0.55;
    trainerEnabled = track.practice.trainerEnabled ?? false;
    trainerRepetitions = track.practice.trainerRepetitions ?? 3;
    trainerIncrement = track.practice.trainerIncrement ?? 0.05;
    trainerTargetRate = track.practice.trainerTargetRate ?? 1;
    trainerLoopCount = 0;
    spectrumBands = Array<number>(64).fill(0);
    tapTimes = [];
    void loadTrackWaveform(track);
    void loadTrackTempo(track);
    void loadSelectedAudio(track, selectionGeneration, autoplay);
  }

  async function enableStems(): Promise<void> {
    if (!project || !currentTrack) return;
    stems = { state: "separating", progress: 0, stage: "checkingCache", trackId: currentTrack.id, cached: false, error: null };
    try { await stemStart(project.packagePath, currentTrack.id); }
    catch (error) { errorMessage = `${t("stemFailed")}: ${error instanceof Error ? error.message : String(error)}`; }
  }

  async function disableStems(): Promise<void> {
    await stemDisable();
    stems = { state: "disabled", progress: 0, stage: "disabled", trackId: null, cached: false, error: null };
  }

  async function refreshStemStatus(): Promise<void> {
    if (stemStatusRequestActive || stems.state === "disabled") return;
    stemStatusRequestActive = true;
    try {
      const next = await stemStatus();
      if (!next.trackId || next.trackId === currentTrack?.id) stems = next;
    } finally { stemStatusRequestActive = false; }
  }

  function updateStem(index: number, change: Partial<StemMix>): void {
    stemMix = stemMix.map((value, candidate) => candidate === index ? { ...value, ...change } : value);
    const value = stemMix[index];
    void stemSetMix(index, value.gain, value.muted, value.soloed);
  }

  async function loadTrackTempo(track: TrackSummary): Promise<void> {
    if (!project) return;
    tempoLoading = true;
    detectedBpm = null;
    const cacheKey = `${project.packagePath}:${track.id}`;
    try {
      const analysis = tempoCache.get(cacheKey) ?? await analyzeTempo(project.packagePath, track.id);
      tempoCache.set(cacheKey, analysis);
      if (currentTrack?.id === track.id) {
        detectedBpm = analysis.bpm;
        if (gridBpm === null && analysis.bpm !== null) {
          gridBpm = analysis.bpm;
          applyBeatGridToEngine();
          schedulePracticeSave();
        }
      }
    } catch {
      if (currentTrack?.id === track.id) detectedBpm = null;
    } finally {
      if (currentTrack?.id === track.id) tempoLoading = false;
    }
  }

  async function loadSelectedAudio(track: TrackSummary, selectionGeneration: number, autoplay: boolean): Promise<void> {
    try {
      const status = await audioLoad(project!.packagePath, track.id);
      if (selectionGeneration !== trackSelectionGeneration || currentTrack?.id !== track.id) return;
      durationSeconds = status.durationSeconds;
      await audioSetVolume(volume);
      await audioSetPlaybackRate(playbackRate);
      await audioSetPitch(pitchSemitones);
      await audioSetBeatGrid(gridBpm, beatGridOffsetSeconds);
      await audioSetMetronome(metronomeEnabled, metronomeVolume);
      await audioSetLoopTrainer(trainerEnabled, trainerRepetitions, trainerIncrement, trainerTargetRate);
      await audioSetEndBehavior(endBehavior);
      endedGeneration = status.endedGeneration;
      applyLoopToEngine();
      await audioSeek(currentSeconds);
      audioLoading = false;
      loadingTrackId = null;
      if (autoplay) await play();
      preloadNeighbour(track);
      void warmPlaylistCache(project!.packagePath, track.id);
    } catch (error) {
      if (selectionGeneration === trackSelectionGeneration) {
        errorMessage = `${t("playbackError")}: ${error instanceof Error ? error.message : String(error)}`;
      }
    } finally {
      if (selectionGeneration === trackSelectionGeneration) {
        audioLoading = false;
        loadingTrackId = null;
      }
    }
  }

  function preloadNeighbour(track: TrackSummary): void {
    if (!project || project.tracks.length < 2) return;
    const index = project.tracks.findIndex((candidate) => candidate.id === track.id);
    const next = project.tracks[(index + 1) % project.tracks.length];
    if (next.id !== track.id) void audioPreload(project.packagePath, next.id).catch(() => undefined);
  }

  async function warmPlaylistCache(packagePath: string, selectedTrackId: string): Promise<void> {
    if (!project || warmedProjects.has(packagePath)) return;
    warmedProjects.add(packagePath);
    for (const track of project.tracks) {
      if (project?.packagePath !== packagePath) return;
      if (track.id === selectedTrackId) continue;
      try {
        await audioPreload(packagePath, track.id);
      } catch {
        // Cache warming is best-effort and must never block normal selection.
      }
    }
  }

  async function loadTrackWaveform(track: TrackSummary): Promise<void> {
    if (!project) return;
    waveformLoading = true;
    waveform = null;
    waveformZoom = 1;
    waveformStart = 0;
    try {
      const cacheKey = `${project.packagePath}:${track.id}`;
      const loaded = waveformCache.get(cacheKey) ?? await getWaveform(project.packagePath, track.id);
      waveformCache.set(cacheKey, loaded);
      if (currentTrack?.id === track.id) {
        waveform = loaded;
        if (loaded.durationSeconds > 0) durationSeconds = loaded.durationSeconds;
      }
    } catch (error) {
      errorMessage = `${t("waveformError")}: ${error instanceof Error ? error.message : String(error)}`;
    } finally {
      if (currentTrack?.id === track.id) waveformLoading = false;
    }
  }

  async function play(): Promise<void> {
    if (!currentTrack && project?.tracks.length) selectTrack(project.tracks[0], false);
    if (!currentTrack || audioLoading) return;
    try {
      if (loopEnabled && loopA !== null && loopB !== null) {
        currentSeconds = loopA;
      }
      await audioPlay();
      isPlaying = true;
    } catch (error) {
      errorMessage = `${t("playbackError")}: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  function togglePlayback(): void {
    if (isPlaying) {
      void audioPause();
      isPlaying = false;
      schedulePracticeSave(0);
    } else void play();
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

  function formatTimePrecise(value: number): string {
    if (!Number.isFinite(value) || value < 0) return "00:00.000";
    const minutes = Math.floor(value / 60);
    const seconds = value % 60;
    return `${String(minutes).padStart(2, "0")}:${seconds.toFixed(3).padStart(6, "0")}`;
  }

  function formatPitch(value: number): string {
    const cents = Math.round(value * 100);
    return Math.abs(value) < 1
      ? `${cents > 0 ? "+" : ""}${cents} ct`
      : `${value > 0 ? "+" : ""}${value.toFixed(2)} st`;
  }

  function seek(position: number): void {
    if (!Number.isFinite(position)) return;
    currentSeconds = Math.max(0, Math.min(position, durationSeconds));
    void audioSeek(currentSeconds);
  }

  function jump(seconds: number): void {
    seek(currentSeconds + seconds);
  }

  function changeGridBpm(value: number): void {
    if (!Number.isFinite(value)) return;
    gridBpm = Math.max(30, Math.min(300, Math.round(value * 10) / 10));
    applyBeatGridToEngine();
    schedulePracticeSave();
  }

  function tapTempo(): void {
    if (!currentTrack) return;
    const now = performance.now();
    const previous = tapTimes.at(-1);
    if (previous === undefined || now - previous > 2_000) tapTimes = [now];
    else tapTimes = [...tapTimes.slice(-7), now];
    if (tapTimes.length < 2) return;
    const intervals = tapTimes.slice(1).map((time, index) => time - tapTimes[index]);
    const sorted = [...intervals].sort((left, right) => left - right);
    const middle = Math.floor(sorted.length / 2);
    const median = sorted.length % 2 === 0
      ? (sorted[middle - 1] + sorted[middle]) / 2
      : sorted[middle];
    changeGridBpm(60_000 / median);
  }

  function applyBeatGridToEngine(): void {
    void audioSetBeatGrid(gridBpm, beatGridOffsetSeconds);
  }

  function setBeatGridAnchor(): void {
    beatGridOffsetSeconds = Math.max(0, currentSeconds);
    applyBeatGridToEngine();
    schedulePracticeSave();
  }

  function nudgeBeatGrid(milliseconds: number): void {
    beatGridOffsetSeconds = Math.max(0, Math.min(durationSeconds, beatGridOffsetSeconds + milliseconds / 1000));
    applyBeatGridToEngine();
    schedulePracticeSave();
  }

  function toggleMetronome(): void {
    if (gridBpm === null) return;
    metronomeEnabled = !metronomeEnabled;
    void audioSetMetronome(metronomeEnabled, metronomeVolume);
    schedulePracticeSave();
  }

  function changeMetronomeVolume(value: number): void {
    metronomeVolume = Math.max(0, Math.min(1, value));
    void audioSetMetronome(metronomeEnabled, metronomeVolume);
    schedulePracticeSave();
  }

  function applyLoopTrainer(): void {
    void audioSetLoopTrainer(trainerEnabled, trainerRepetitions, trainerIncrement, trainerTargetRate);
    schedulePracticeSave();
  }

  function toggleLoopTrainer(): void {
    trainerEnabled = !trainerEnabled;
    applyLoopTrainer();
  }

  function updateTrainerRepetitions(value: number): void {
    trainerRepetitions = Math.max(1, Math.min(99, Math.round(value)));
    applyLoopTrainer();
  }

  function updateTrainerIncrement(value: number): void {
    trainerIncrement = Math.max(0.01, Math.min(0.25, value / 100));
    applyLoopTrainer();
  }

  function updateTrainerTarget(value: number): void {
    trainerTargetRate = Math.max(0.5, Math.min(2, value / 100));
    applyLoopTrainer();
  }

  function beatLines(detailed: boolean): { percent: number; accent: boolean }[] {
    if (gridBpm === null || durationSeconds <= 0) return [];
    const period = 60 / gridBpm;
    const visibleStart = detailed ? waveformStart * durationSeconds : 0;
    const visibleEnd = detailed ? (waveformStart + 1 / waveformZoom) * durationSeconds : durationSeconds;
    const firstBeat = Math.ceil((visibleStart - beatGridOffsetSeconds) / period);
    const lastBeat = Math.floor((visibleEnd - beatGridOffsetSeconds) / period);
    const count = Math.min(500, Math.max(0, lastBeat - firstBeat + 1));
    const lines: { percent: number; accent: boolean }[] = [];
    for (let index = 0; index < count; index += 1) {
      const beat = firstBeat + index;
      const seconds = beatGridOffsetSeconds + beat * period;
      const percent = detailed
        ? (seconds / durationSeconds - waveformStart) * waveformZoom * 100
        : seconds / durationSeconds * 100;
      lines.push({ percent, accent: ((beat % 4) + 4) % 4 === 0 });
    }
    return lines;
  }

  function setLoopA(): void {
    loopA = currentSeconds;
    if (loopB !== null && loopB <= loopA) loopB = null;
    loopEnabled = true;
    applyLoopToEngine();
    schedulePracticeSave();
  }

  function setLoopB(): void {
    if (loopA === null) loopA = 0;
    if (currentSeconds > loopA) loopB = currentSeconds;
    loopEnabled = true;
    applyLoopToEngine();
    schedulePracticeSave();
  }

  function clearLoop(): void {
    loopA = null;
    loopB = null;
    loopEnabled = false;
    void audioSetLoop(null, null);
    schedulePracticeSave();
  }

  function changeVolume(value: number): void {
    volume = Math.max(0, Math.min(1, value));
    if (volume > 0) volumeBeforeMute = volume;
    void audioSetVolume(volume);
    schedulePracticeSave();
  }

  function changePlaybackRate(delta: number): void {
    playbackRate = Math.round(Math.max(0.5, Math.min(2, playbackRate + delta)) * 100) / 100;
    void audioSetPlaybackRate(playbackRate);
    schedulePracticeSave();
  }

  function resetPlaybackRate(): void {
    playbackRate = 1;
    void audioSetPlaybackRate(playbackRate);
    schedulePracticeSave();
  }

  function changePitch(delta: number): void {
    pitchSemitones = Math.max(-12, Math.min(12, pitchSemitones + delta));
    void audioSetPitch(pitchSemitones);
    schedulePracticeSave();
  }

  function resetPitch(): void {
    pitchSemitones = 0;
    void audioSetPitch(pitchSemitones);
    schedulePracticeSave();
  }

  function toggleMute(): void {
    changeVolume(volume > 0 ? 0 : volumeBeforeMute);
  }

  function toggleLoop(): void {
    if (loopA === null || loopB === null) {
      const span = Math.min(5, Math.max(0.25, durationSeconds));
      loopA = Math.min(currentSeconds, Math.max(0, durationSeconds - span));
      loopB = Math.min(durationSeconds, loopA + span);
    }
    loopEnabled = !loopEnabled;
    applyLoopToEngine();
    schedulePracticeSave();
  }

  function applyLoopToEngine(): void {
    void audioSetLoop(loopEnabled ? loopA : null, loopEnabled ? loopB : null);
  }

  function eventTime(event: PointerEvent, detailed: boolean): number {
    const selector = detailed ? ".detailed-wave" : ".overview-wave";
    const surface = (event.currentTarget as HTMLElement).closest(selector) as HTMLElement | null;
    const bounds = (surface ?? event.currentTarget as HTMLElement).getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    return (detailed ? waveformStart + ratio / waveformZoom : ratio) * durationSeconds;
  }

  function startLoopDrag(event: PointerEvent, mode: LoopDragMode, detailed: boolean): void {
    if (loopA === null || loopB === null) return;
    event.stopPropagation();
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    loopDrag = { mode, pointerId: event.pointerId, originTime: eventTime(event, detailed), a: loopA, b: loopB };
  }

  function moveLoopDrag(event: PointerEvent, detailed: boolean): void {
    if (!loopDrag || loopDrag.pointerId !== event.pointerId || durationSeconds <= 0) return;
    const time = eventTime(event, detailed);
    const minimum = Math.min(0.05, durationSeconds);
    if (loopDrag.mode === "a") loopA = Math.max(0, Math.min(time, loopDrag.b - minimum));
    else if (loopDrag.mode === "b") loopB = Math.min(durationSeconds, Math.max(time, loopDrag.a + minimum));
    else {
      const length = loopDrag.b - loopDrag.a;
      const a = Math.max(0, Math.min(durationSeconds - length, loopDrag.a + time - loopDrag.originTime));
      loopA = a;
      loopB = a + length;
    }
    loopEnabled = true;
    applyLoopToEngine();
  }

  function finishLoopDrag(event: PointerEvent): void {
    if (!loopDrag || loopDrag.pointerId !== event.pointerId) return;
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    loopDrag = null;
    schedulePracticeSave(0);
  }

  async function refreshAudioStatus(): Promise<void> {
    if (!currentTrack || statusRequestActive) return;
    statusRequestActive = true;
    try {
      const status = await audioStatus();
      currentSeconds = status.positionSeconds;
      durationSeconds = status.durationSeconds || durationSeconds;
      isPlaying = status.playing;
      if (status.endedGeneration !== endedGeneration) {
        endedGeneration = status.endedGeneration;
        if (endBehavior === "advance" && (project?.tracks.length ?? 0) > 1) {
          moveTrack(1);
          return;
        }
      }
      trainerLoopCount = status.trainerLoopCount;
      if (Math.abs(playbackRate - status.playbackRate) > 0.0001 || trainerEnabled !== status.trainerEnabled) {
        playbackRate = status.playbackRate;
        trainerEnabled = status.trainerEnabled;
        schedulePracticeSave(300);
      }
    } catch {
      // The engine may be unavailable during application shutdown.
    } finally {
      statusRequestActive = false;
    }
  }

  async function refreshSpectrum(): Promise<void> {
    if (!currentTrack || !isPlaying || spectrumRequestActive) return;
    spectrumRequestActive = true;
    try {
      const frame = await audioSpectrum();
      spectrumBands = frame.bands;
    } catch {
      // Spectrum visualization is optional and must never affect playback.
    } finally {
      spectrumRequestActive = false;
    }
  }

  function schedulePracticeSave(delay = 700): void {
    window.clearTimeout(practiceSaveTimer);
    practiceSaveTimer = window.setTimeout(() => void persistCurrentPracticeState(), delay);
  }

  async function persistCurrentPracticeState(): Promise<void> {
    if (!project || !currentTrack) return;
    const packagePath = project.packagePath;
    const trackId = currentTrack.id;
    try {
      const updated = await updatePracticeState(packagePath, trackId, {
        positionSeconds: Math.max(0, currentSeconds),
        playbackRate,
        pitchSemitones,
        volume,
        loopEnabled,
        loopASeconds: loopA,
        loopBSeconds: loopB,
        gridBpm,
        beatGridOffsetSeconds,
        metronomeEnabled,
        metronomeVolume,
        trainerEnabled,
        trainerRepetitions,
        trainerIncrement,
        trainerTargetRate,
      });
      if (project?.packagePath === packagePath) project = updated;
    } catch (error) {
      errorMessage = `${t("saveError")}: ${error instanceof Error ? error.message : String(error)}`;
    }
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

<svelte:head><title>SonArcan — {t("tagline")}</title></svelte:head>

<main class="shell">
  <header class="topbar">
    <div>
      <span class="eyebrow">SONARCAN</span>
      <h1>{project?.name ?? t("tagline")}</h1>
    </div>
    <div class="project-context">{project ? project.packagePath : t("noProject")}</div>
  </header>

  {#if errorMessage}<div class="error" role="alert">{errorMessage}</div>{/if}

  <section class="workspace">
    <aside class="playlist panel">
      <div class="panel-title"><h2>{t("playlist")}</h2><span>{project?.trackCount ?? 0}</span></div>
      {#if project && project.tracks.length > 0}
        <ol>
          {#each project.tracks as track, index}
            <li
              class:active={track.id === currentTrack?.id}
              class:loading={track.id === loadingTrackId}
              class:drop-target={track.id === dropTrackId}
              draggable={editingTrackId !== track.id}
              ondragstart={(event) => { draggedTrackId = track.id; event.dataTransfer?.setData("text/plain", track.id); }}
              ondragover={(event) => { event.preventDefault(); dropTrackId = track.id; }}
              ondragleave={() => { if (dropTrackId === track.id) dropTrackId = null; }}
              ondrop={(event) => dropTrack(event, index)}
              ondragend={() => { draggedTrackId = null; dropTrackId = null; }}
            >
              <span class="drag-handle" aria-hidden="true">⠿</span>
              <button class="track-select" onclick={() => selectTrack(track)} aria-label={`${t("loadingTrack")} ${track.title}`}><span class="track-number">{#if track.id === loadingTrackId}<i class="mini-spinner" aria-label={t("loadingTrack")}></i>{:else}{String(index + 1).padStart(2, "0")}{/if}</span></button>
              <div class="track-info">
                {#if editingTrackId === track.id}
                  <input class="track-title-input" bind:value={editingTrackTitle} aria-label={t("trackName")} use:focusOnMount onblur={() => commitTrackRename(track)} onkeydown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); else if (event.key === "Escape") { editingTrackId = null; } }} />
                {:else}
                  <button class="track-title" onclick={() => startTrackRename(track)} data-tooltip={t("renameTrack")}>{track.title}</button>
                {/if}
                <button class="track-meta" onclick={() => selectTrack(track)}>{track.format.toUpperCase()} · {track.sampleRate ? `${track.sampleRate} Hz` : t("unknownRate")}</button>
              </div>
            </li>
          {/each}
        </ol>
      {:else}
        <div class="empty">{t("emptyPlaylist")}</div>
      {/if}
    </aside>

    <section class="main-stage">
      <div class="visualizer panel">
        <div class="panel-title"><h2>{t("waveform")}</h2><div class="load-states">{#if audioLoading}<span><i class="mini-spinner"></i>{t("loadingAudio")}</span>{/if}{#if waveformLoading}<span><i class="mini-spinner"></i>{t("waveformLoading")}</span>{/if}{#if tempoLoading}<span><i class="mini-spinner"></i>{t("bpmAnalyzing")}</span>{:else if detectedBpm !== null}<span class="bpm" data-tooltip={t("bpmDetected")}>{detectedBpm.toFixed(1)} BPM</span>{/if}{#if currentTrack && !audioLoading && !waveformLoading}<span class="loaded">✓ {t("audioReady")}</span>{/if}</div></div>
        <div
          class="wave detailed-wave"
          class:dragging={dragMoved}
          role="application"
          aria-label={t("waveform")}
          data-tooltip={t("seekHelp")}
          onwheel={zoomWaveform}
          onpointerdown={startWaveformDrag}
          onpointermove={dragWaveform}
          onpointerup={finishWaveformDrag}
        >
          {#if waveformLoading}<div class="wave-skeleton" aria-label={t("waveformLoading")}><svg viewBox={`0 0 ${loadingWave.length} 100`} preserveAspectRatio="none" aria-hidden="true">{#each loadingWave as height, index}<line x1={index} x2={index} y1={50 - height * 45} y2={50 + height * 45}></line>{/each}</svg><i></i><span>{t("waveformLoading")}</span></div>
          {:else if detailedPeaks.length === 0}<span class="wave-message">{t("waveformEmpty")}</span>
          {:else}
            <svg viewBox={`0 0 ${detailedPeaks.length} 100`} preserveAspectRatio="none" aria-hidden="true">
              {#each detailedPeaks as peak, index}
                <line x1={index} x2={index} y1={50 - peak.max * 48} y2={50 - peak.min * 48} />
              {/each}
            </svg>
            <div class="beat-grid" aria-hidden="true">
              {#each detailedBeatLines as beat}<i class:accent={beat.accent} style={`left:${beat.percent}%`}></i>{/each}
            </div>
            {#if loopA !== null && loopB !== null}
              <button
                type="button"
                class="loop-region editable"
                class:disabled={!loopEnabled}
                aria-label={t("moveRegion")}
                data-tooltip={t("moveRegion")}
                style={`left:${(loopA / durationSeconds - waveformStart) * waveformZoom * 100}%;width:${(loopB - loopA) / durationSeconds * waveformZoom * 100}%`}
                onpointerdown={(event) => startLoopDrag(event, "region", true)}
                onpointermove={(event) => moveLoopDrag(event, true)}
                onpointerup={finishLoopDrag}
              ></button>
              <button class="loop-handle a" style={`left:${(loopA / durationSeconds - waveformStart) * waveformZoom * 100}%`} aria-label={t("moveStart")} data-tooltip={t("moveStart")} onpointerdown={(event) => startLoopDrag(event, "a", true)} onpointermove={(event) => moveLoopDrag(event, true)} onpointerup={finishLoopDrag}>A</button>
              <button class="loop-handle b" style={`left:${(loopB / durationSeconds - waveformStart) * waveformZoom * 100}%`} aria-label={t("moveEnd")} data-tooltip={t("moveEnd")} onpointerdown={(event) => startLoopDrag(event, "b", true)} onpointermove={(event) => moveLoopDrag(event, true)} onpointerup={finishLoopDrag}>B</button>
            {/if}
            {#if playheadPercent >= 0 && playheadPercent <= 100}<i class="playhead" style={`left:${playheadPercent}%`}></i>{/if}
          {/if}
        </div>
        <div class="zoom-info"><span>{waveformZoom.toFixed(1)}×</span><span>{t("waveformHelp")}</span></div>
        <div class="overview-wave" role="application" aria-label={t("overviewHelp")} data-tooltip={t("overviewHelp")} onpointerdown={seekFromOverview}>
          {#if waveformLoading}<div class="overview-skeleton"><svg viewBox={`0 0 ${loadingWave.length} 60`} preserveAspectRatio="none" aria-hidden="true">{#each loadingWave as height, index}<line x1={index} x2={index} y1={30 - height * 27} y2={30 + height * 27}></line>{/each}</svg><i></i></div>
          {:else if overviewPeaks.length > 0}
            <svg viewBox={`0 0 ${overviewPeaks.length} 60`} preserveAspectRatio="none" aria-hidden="true">
              {#each overviewPeaks as peak, index}
                <line x1={index} x2={index} y1={30 - peak.max * 28} y2={30 - peak.min * 28} />
              {/each}
            </svg>
            <div class="beat-grid overview" aria-hidden="true">
              {#each overviewBeatLines as beat}<i class:accent={beat.accent} style={`left:${beat.percent}%`}></i>{/each}
            </div>
            <i class="viewport" style={`left:${waveformStart * 100}%;width:${100 / waveformZoom}%`}></i>
            {#if loopA !== null && loopB !== null}
              <button type="button" class="loop-region overview editable" class:disabled={!loopEnabled} aria-label={t("moveRegion")} data-tooltip={t("moveRegion")} style={`left:${loopA / durationSeconds * 100}%;width:${(loopB - loopA) / durationSeconds * 100}%`} onpointerdown={(event) => startLoopDrag(event, "region", false)} onpointermove={(event) => moveLoopDrag(event, false)} onpointerup={finishLoopDrag}></button>
              <button class="loop-handle overview a" style={`left:${loopA / durationSeconds * 100}%`} aria-label={t("moveStart")} data-tooltip={t("moveStart")} onpointerdown={(event) => startLoopDrag(event, "a", false)} onpointermove={(event) => moveLoopDrag(event, false)} onpointerup={finishLoopDrag}>A</button>
              <button class="loop-handle overview b" style={`left:${loopB / durationSeconds * 100}%`} aria-label={t("moveEnd")} data-tooltip={t("moveEnd")} onpointerdown={(event) => startLoopDrag(event, "b", false)} onpointermove={(event) => moveLoopDrag(event, false)} onpointerup={finishLoopDrag}>B</button>
            {/if}
            <i class="overview-playhead" style={`left:${durationSeconds ? currentSeconds / durationSeconds * 100 : 0}%`}></i>
          {/if}
        </div>
        <div class="timeline"><span>00:00</span><span>{formatTime(durationSeconds * .25)}</span><span>{formatTime(durationSeconds * .5)}</span><span>{formatTime(durationSeconds * .75)}</span><span>{formatTime(durationSeconds)}</span></div>
        <input
          class="seek"
          aria-label={t("playbackPosition")}
          type="range"
          min="0"
          max={durationSeconds || 1}
          step="0.01"
          value={currentSeconds}
          oninput={(event) => seek(Number(event.currentTarget.value))}
        />
        <div class="loop-status">
          <span>A {loopA === null ? "—" : formatTime(loopA)}</span>
          <span>{loopA !== null && loopB !== null ? `${loopEnabled ? t("loop") : t("loopOff")} ${formatTime(loopB - loopA)}` : t("noLoop")}</span>
          <span>B {loopB === null ? "—" : formatTime(loopB)}</span>
        </div>
      </div>

      <div class="transport panel">
        <button disabled={audioLoading} aria-label={t("back5")} data-tooltip={t("back5")} onclick={() => jump(-5)}>−5s</button>
        <button disabled={audioLoading} class="round" aria-label={t("previous")} data-tooltip={t("previous")} onclick={() => moveTrack(-1)}>◀</button>
        <button disabled={audioLoading} class="play" class:loading={audioLoading} aria-label={audioLoading ? t("loadingAudio") : isPlaying ? t("pause") : t("play")} data-tooltip={audioLoading ? t("loadingAudio") : isPlaying ? t("pause") : t("play")} onclick={togglePlayback}>{#if audioLoading}<i class="button-spinner"></i>{:else}{isPlaying ? "Ⅱ" : "▶"}{/if}</button>
        <button disabled={audioLoading} class="round" aria-label={t("next")} data-tooltip={t("next")} onclick={() => moveTrack(1)}>▶</button>
        <button disabled={audioLoading} aria-label={t("forward5")} data-tooltip={t("forward5")} onclick={() => jump(5)}>+5s</button>
        <div class="readout"><small>{t("position")}</small><strong>{formatTime(currentSeconds)} / {formatTime(durationSeconds)}</strong></div>
        <div class="end-behavior" role="group" aria-label={t("endBehavior")}>
          <button class:active={endBehavior === "restart"} aria-pressed={endBehavior === "restart"} aria-label={t("restartAtEnd")} data-tooltip={t("restartAtEnd")} onclick={() => changeEndBehavior("restart")}>↶</button>
          <button class:active={endBehavior === "advance"} aria-pressed={endBehavior === "advance"} aria-label={t("advanceAtEnd")} data-tooltip={t("advanceAtEnd")} onclick={() => changeEndBehavior("advance")}>⇥</button>
          <button class:active={endBehavior === "stop"} aria-pressed={endBehavior === "stop"} aria-label={t("stopAtEnd")} data-tooltip={t("stopAtEnd")} onclick={() => changeEndBehavior("stop")}>■</button>
        </div>
      </div>

      <div class="practice panel">
        <div class="loop-controls">
          <div class="control-group loop-actions"><button onclick={setLoopA} data-tooltip={t("moveA")}>A</button><button onclick={clearLoop} data-tooltip={t("resetAB")}>×</button><button onclick={setLoopB} data-tooltip={t("moveB")}>B</button><button class:active={loopEnabled} onclick={toggleLoop} aria-pressed={loopEnabled} data-tooltip={t("toggleLoop")}>↻ {t("loop")}</button></div>
        </div>
        <NumericControl label={t("tempo")} value={playbackRate} defaultValue={1} minimum={0.5} maximum={2} step={0.05} display={(value) => `${Math.round(value * 100)}%`} onChange={(value) => { playbackRate = value; void audioSetPlaybackRate(value); schedulePracticeSave(); }} tooltip={t("numericHelp")} />
        <NumericControl label={t("pitch")} value={pitchSemitones} defaultValue={0} minimum={-12} maximum={12} step={0.01} display={formatPitch} onChange={(value) => { pitchSemitones = value; void audioSetPitch(value); schedulePracticeSave(); }} tooltip={t("pitchFineHelp")} />
        <div class="volume"><button class="volume-toggle" onclick={toggleMute} aria-label={volume > 0 ? t("mute") : t("unmute")} data-tooltip={volume > 0 ? t("mute") : t("unmute")}>{volume > 0 ? "🔊" : "🔇"}</button><NumericControl label={t("volume")} value={volume} defaultValue={0.8} minimum={0} maximum={1} step={0.05} display={(value) => `${Math.round(value * 100)}%`} onChange={changeVolume} tooltip={t("numericHelp")} /></div>
      </div>

      <div class="tempo-grid panel">
        <div class="tempo-editor"><NumericControl label={t("gridTempo")} value={gridBpm ?? detectedBpm ?? 120} defaultValue={detectedBpm ?? 120} minimum={30} maximum={300} step={0.1} display={(value) => `${value.toFixed(1)} BPM`} onChange={changeGridBpm} onTap={tapTempo} tooltip={t("tapTempoHelp")} /><small>{detectedBpm !== null ? `${t("detected")}: ${detectedBpm.toFixed(1)}` : t("bpmUnknown")}</small></div>
        <div class="grid-anchor">
          <button disabled={gridBpm === null} data-tooltip={t("setGridAnchorHelp")} onclick={setBeatGridAnchor}>{t("setGridAnchor")}</button>
          <div class="compact-controls"><button disabled={gridBpm === null} data-tooltip={t("nudgeGridBack")} onclick={() => nudgeBeatGrid(-10)}>−10 ms</button><button disabled={gridBpm === null} data-tooltip={t("nudgeGridForward")} onclick={() => nudgeBeatGrid(10)}>+10 ms</button></div>
          <small>{t("gridAnchor")}: {formatTimePrecise(beatGridOffsetSeconds)}</small>
        </div>
        <div class="metronome-control">
          <button class:active={metronomeEnabled} disabled={gridBpm === null} aria-pressed={metronomeEnabled} data-tooltip={t("metronomeHelp")} onclick={toggleMetronome}>♩ {t("metronome")}</button>
          <NumericControl label={t("metronomeVolume")} value={metronomeVolume} defaultValue={0.55} minimum={0} maximum={1} step={0.05} display={(value) => `${Math.round(value * 100)}%`} onChange={changeMetronomeVolume} tooltip={t("numericHelp")} />
        </div>
      </div>

      <div class="trainer panel">
        <div class="trainer-heading"><button class:active={trainerEnabled} aria-pressed={trainerEnabled} data-tooltip={t("trainerHelp")} onclick={toggleLoopTrainer}>↗ {t("loopTrainer")}</button><small>{trainerLoopCount} / {trainerRepetitions} {t("cycles")}</small></div>
        <NumericControl label={t("repetitions")} value={trainerRepetitions} defaultValue={3} minimum={1} maximum={99} step={1} display={(value) => String(value)} onChange={updateTrainerRepetitions} tooltip={t("numericHelp")} />
        <NumericControl label={t("increment")} value={trainerIncrement * 100} defaultValue={5} minimum={1} maximum={25} step={1} display={(value) => `+${value}%`} onChange={updateTrainerIncrement} tooltip={t("numericHelp")} />
        <NumericControl label={t("targetTempo")} value={trainerTargetRate * 100} defaultValue={100} minimum={50} maximum={200} step={5} display={(value) => `${value}%`} onChange={updateTrainerTarget} tooltip={t("numericHelp")} />
        <div class="trainer-progress"><i style={`width:${Math.max(0, Math.min(100, trainerLoopCount / trainerRepetitions * 100))}%`}></i></div>
      </div>

      <div class="spectrum panel">
        <div class="panel-title"><h2>{t("spectrum")}</h2><span>30 Hz — 20 kHz · FFT 2048</span></div>
        <div class="spectrum-bars" aria-label={t("spectrum")}>
          {#each spectrumBands as magnitude, index}<i style={`height:${Math.max(1, magnitude * 100)}%;--band:${index}`}></i>{/each}
        </div>
        <div class="spectrum-scale"><span>30</span><span>100</span><span>1k</span><span>10k</span><span>20k Hz</span></div>
      </div>

      <div class="lower-grid">
        <div class="panel stem-panel">
          <div class="panel-title"><h2>{t("stems")}</h2><span>{stems.state === "ready" ? t("stemsReady") : stems.state === "failed" ? t("stemFailed") : t("idle")}</span></div>
          {#if stems.state === "disabled"}
            <div class="stem-empty"><button class="primary" data-tooltip={t("stemHelp")} disabled={!currentTrack} onclick={enableStems}>{t("enableStems")}</button><small>HTDemucs · 4 stems · local</small></div>
          {:else if stems.state === "downloading" || stems.state === "separating"}
            <div class="stem-progress"><div class="stem-progress-label"><span class="mini-spinner"></span><span>{stems.state === "downloading" ? t("downloadingModel") : t("separatingStems")}</span><b>{Math.round(stems.progress * 100)}%</b></div><i><b style={`width:${Math.max(1, stems.progress * 100)}%`}></b></i><button onclick={disableStems}>{t("disableStems")}</button></div>
          {:else if stems.state === "failed"}
            <div class="stem-empty"><p>{stems.error ?? t("stemFailed")}</p><button onclick={enableStems}>{t("enableStems")}</button></div>
          {:else}
            <div class="stem-mixer">
              {#each [t("vocals"), t("drums"), t("bass"), t("other")] as name, index}
                <section class="stem-strip"><strong>{name}</strong><output>{Math.round(stemMix[index].gain * 100)}%</output><input aria-label={`${name} ${t("volume")}`} type="range" min="0" max="2" step="0.01" value={stemMix[index].gain} oninput={(event) => updateStem(index, { gain: Number(event.currentTarget.value) })} /><div><button class:active={stemMix[index].muted} onclick={() => updateStem(index, { muted: !stemMix[index].muted })}>M</button><button class:active={stemMix[index].soloed} onclick={() => updateStem(index, { soloed: !stemMix[index].soloed })}>S</button></div></section>
              {/each}
            </div>
            <button class="stem-disable" onclick={disableStems}>{t("disableStems")}</button>
          {/if}
        </div>
        <div class="panel"><div class="panel-title"><h2>{t("chords")}</h2><span>{t("notAnalyzed")}</span></div><div class="chords"><b>Am7</b><b>Fmaj7</b><b>C</b><b>G</b></div></div>
      </div>
    </section>
  </section>

  <footer><span>{busy ? t("working") : t("ready")}</span><button class="link" onclick={showDiagnostics}>{t("diagnostics")}</button></footer>

  {#if diagnosticInfo}
    <dialog open><h2>{t("diagnostics")}</h2><dl><dt>{t("version")}</dt><dd>{diagnosticInfo.appVersion}</dd><dt>OS</dt><dd>{diagnosticInfo.os}</dd><dt>{t("architecture")}</dt><dd>{diagnosticInfo.architecture}</dd><dt>{t("logging")}</dt><dd>{diagnosticInfo.rustLog}</dd></dl><button onclick={() => diagnosticInfo = null}>{t("close")}</button></dialog>
  {/if}

  {#if preferencesVisible}
    <dialog open><h2>{t("preferences")}</h2><p>{t("preferencesHelp")}</p><label class="language-select">{t("language")}<select value={language} onchange={(event) => changeLanguage(event.currentTarget.value as Language)}><option value="fr">{t("french")}</option><option value="en">{t("english")}</option></select></label><button onclick={() => preferencesVisible = false}>{t("close")}</button></dialog>
  {/if}

  {#if shortcutsVisible}
    <dialog open><h2>{t("shortcuts")}</h2><dl><dt>{t("playPause")}</dt><dd>Space</dd><dt>{t("jump")}</dt><dd>← / →</dd><dt>{t("loopAB")}</dt><dd>A / B</dd><dt>{t("clearLoop")}</dt><dd>Escape</dd><dt>{t("tempo")}</dt><dd>− / +</dd><dt>{t("tapTempo")}</dt><dd>T</dd><dt>{t("metronome")}</dt><dd>M</dd></dl><button onclick={() => shortcutsVisible = false}>{t("close")}</button></dialog>
  {/if}
</main>
