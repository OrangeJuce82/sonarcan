<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { analyzeImportText, analyzeTempo, audioLoad, audioPause, audioPlay, audioPreload, audioSeek, audioSetBeatGrid, audioSetEndBehavior, audioSetLoop, audioSetLoopTrainer, audioSetMetronome, audioSetPitch, audioSetPlaybackRate, audioSetVolume, audioSpectrum, audioStatus, cancelImport, confirmApplicationExit, createTemporaryProject, deleteTrack as deleteTrackFromProject, diagnostics, enqueueImports, exportPlaylist, getPreferences, getWaveform, importJobs, initializeProject, listRecentProjects, logsSnapshot, openExternalLink, openProject, pushFrontendLog, readClipboardText, readImportTextFiles, removeImportJob, renameProject, renameTrack, reorderTrack, requestApplicationExit, resolveYoutubeSearch, revealProject, savePreferences, saveProjectAs, setApplicationLanguage, stemDisable, stemSetMix, stemStart, stemStatus, systemMetrics, updatePracticeState } from "./lib/backend";
  import { systemLanguage, translate, type Language, type MessageKey } from "./lib/i18n";
  import { deduplicateImportCandidates } from "./lib/importCandidates";
  import Icon from "./lib/Icon.svelte";
  import NumericControl from "./lib/NumericControl.svelte";
  import Modal from "./lib/Modal.svelte";
  import { buildProjectPath, calculateBeatLines, defaultLoopBounds, formatPitch, formatProjectHeaderPath, formatTime, formatTimePrecise, moveWaveformViewport, resizeWaveformViewport, visiblePeaks, zoomWaveformViewport, type WaveformViewport, type WaveformViewportEdge } from "./lib/presentation";
  import { forgetTrackSelection, preferredTrack, rememberedTrackId, rememberTrackSelection } from "./lib/projectSelection";
  import type { AppLogEntry, DiagnosticsSnapshot, EndBehavior, ImportCandidate, ImportJob, ProjectSummary, StemMix, StemStatus, SystemMetrics, TempoAnalysis, TrackSummary, UserPreferences, WaveformData } from "./lib/types";

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
  let masterPeak = 0;
  let masterPeakLeft = 0;
  let masterPeakRight = 0;
  let loopEnabled = false;
  let loopA: number | null = null;
  let loopB: number | null = null;
  let usingDefaultLoopBounds = false;
  let preferencesVisible = false;
  let importVisible = false;
  let tasksVisible = false;
  let consoleVisible = false;
  let appLogs: AppLogEntry[] = [];
  let shortcutsVisible = false;
  let closePromptVisible = false;
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
  let stems: StemStatus = { state: "disabled", progress: 0, stage: "disabled", trackId: null, cached: false, error: null, computeBackend: null };
  let stemMix: StemMix[] = Array.from({ length: 4 }, () => ({ gain: 1, muted: false, soloed: false }));
  let stemStatusRequestActive = false;
  let preferences: UserPreferences = { theme: "system", language: "en", smartClipboard: false, searchMode: "chooseFive", maxImportBatch: 10, concurrentDownloads: 3, conversionFormat: "mp3", sampleRate: "preserve", channels: "stereo", mp3Quality: "vbrHigh", masterVolume: 0.8, metronomeVolume: 0.55, defaultPlaybackRate: 1, defaultPitchSemitones: 0, defaultTrainerEnabled: false, defaultTrainerRepetitions: 3, defaultTrainerIncrement: 0.05, defaultTrainerTargetRate: 1 };
  let importText = "";
  let importCandidates: ImportCandidate[] = [];
  let selectedImports = new Set<string>();
  let importAnalyzing = false;
  let importAnalysisError = "";
  let importHasAnalyzed = false;
  let importDropActive = false;
  let importTextarea: HTMLTextAreaElement | undefined;
  let importAnalysisTimer: number | undefined;
  let importAnalysisGeneration = 0;
  let importQueue: ImportJob[] = [];
  const importDismissTimers = new Map<string, number>();
  const masterMeterLevels = [8, 7, 6, 5, 4, 3, 2, 1] as const;
  let lastClipboardText = "";
  let clipboardDetected = 0;
  let clipboardReadActive = false;
  let clipboardForcePending = false;
  let editingTrackId: string | null = null;
  let editingTrackTitle = "";
  let draggedTrackId: string | null = null;
  let dropTrackId: string | null = null;
  let dropTrackIndex: number | null = null;
  let trackContextMenu: { trackId: string; x: number; y: number } | null = null;
  let editingProjectName = false;
  let projectNameDraft = "";
  let helpMessage = "";
  let endBehavior: EndBehavior = "stop";
  let endedGeneration = 0;
  let waveformZoom = 1;
  let waveformStart = 0;
  let dragStartX = 0;
  let dragStartViewport = 0;
  let dragStartZoom = 1;
  let dragMoved = false;
  let waveformDragPointerId: number | null = null;
  let practiceSaveTimer: number | undefined;
  let playbackRateTimer: number | undefined;
  let pitchTimer: number | undefined;
  let volumePreferenceTimer: number | undefined;
  let seekAnimationFrame: number | undefined;
  let pendingSeekPosition: number | null = null;
  let seekRequestActive = false;
  let statusRequestActive = false;
  let systemMetricsSnapshot: SystemMetrics = { cpuPercent: null, memoryMegabytes: null };
  let trackSelectionGeneration = 0;
  const waveformCache = new Map<string, WaveformData>();
  const tempoCache = new Map<string, TempoAnalysis>();
  const loadingWave = Array.from({ length: 72 }, (_, index) => Math.min(0.95, 0.12 + Math.abs(Math.sin(index * 0.71) * Math.cos(index * 0.17)) * 0.78));
  const warmedProjects = new Set<string>();
  type LoopDragMode = "a" | "b";
  let loopDrag: { mode: LoopDragMode; pointerId: number; a: number; b: number } | null = null;
  type ViewportDragMode = "move" | WaveformViewportEdge;
  let viewportDrag: { mode: ViewportDragMode; pointerId: number; originRatio: number; originClientX: number; start: number; zoom: number; moved: boolean } | null = null;
  let language: Language = systemLanguage();
  const t = (key: MessageKey): string => translate(language, key);

  $: projectHeaderPath = project ? formatProjectHeaderPath(project.packagePath) : null;

  function focusOnMount(node: HTMLInputElement): void {
    queueMicrotask(() => {
      node.focus();
      node.select();
    });
  }

  function helpTarget(target: EventTarget | null): HTMLElement | null {
    return target instanceof Element ? target.closest<HTMLElement>("[data-tooltip]") : null;
  }

  function updateHelp(target: EventTarget | null): void {
    helpMessage = helpTarget(target)?.dataset.tooltip ?? "";
  }

  $: detailedPeaks = visiblePeaks(waveform?.peaks ?? [], waveformZoom, waveformStart, 1_000);
  $: overviewPeaks = visiblePeaks(waveform?.peaks ?? [], 1, 0, 700);
  $: playheadPercent = durationSeconds > 0 ? ((currentSeconds / durationSeconds - waveformStart) * waveformZoom * 100) : 0;
  $: detailedBeatLines = beatLines(true);
  $: overviewBeatLines = beatLines(false);
  $: activeImports = importQueue.filter((job) => !["completed", "failed"].includes(job.state));
  $: importProgress = importQueue.length ? importQueue.reduce((sum, job) => sum + job.progress, 0) / importQueue.length : 0;

  onMount(() => {
    let unlisten: UnlistenFn | undefined;
    let unlistenDrag: (() => void) | undefined;
    let unlistenClose: (() => void) | undefined;
    let unlistenExit: UnlistenFn | undefined;
    const appWindow = getCurrentWindow();
    void listen<string>("native-menu", (event) => handleNativeMenu(event.payload)).then((stop) => unlisten = stop);
    void listen<void>("application-exit-requested", () => closePromptVisible = true).then((stop) => unlistenExit = stop);
    const handleKeydown = (event: KeyboardEvent): void => {
      if (event.key === "Escape" && document.querySelector("dialog[open]")) return;
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
    const handleHelpOver = (event: PointerEvent): void => updateHelp(event.target);
    const handleHelpOut = (event: PointerEvent): void => {
      const from = helpTarget(event.target);
      const to = helpTarget(event.relatedTarget);
      if (from !== to) helpMessage = to?.dataset.tooltip ?? "";
    };
    const handleHelpFocus = (event: FocusEvent): void => updateHelp(event.target);
    const handleHelpBlur = (event: FocusEvent): void => {
      if (!helpTarget(event.relatedTarget)) helpMessage = "";
    };
    window.addEventListener("pointerover", handleHelpOver);
    window.addEventListener("pointerout", handleHelpOut);
    window.addEventListener("focusin", handleHelpFocus);
    window.addEventListener("focusout", handleHelpBlur);
    const statusTimer = window.setInterval(() => void refreshAudioStatus(), 33);
    const spectrumTimer = window.setInterval(() => void refreshSpectrum(), 50);
    const stemTimer = window.setInterval(() => void refreshStemStatus(), 400);
    const metricsTimer = window.setInterval(() => void refreshSystemMetrics(), 1_500);
    const importTimer = window.setInterval(() => void refreshImportJobs(), 500);
    let clipboardTimer: number | undefined;
    const pollClipboard = (): void => {
      void inspectClipboard();
      clipboardTimer = window.setTimeout(pollClipboard, importVisible ? 300 : 900);
    };
    clipboardTimer = window.setTimeout(pollClipboard, 300);
    const inspectClipboardAfterEvent = (): void => {
      if (!importVisible || !preferences.smartClipboard) return;
      window.setTimeout(() => void inspectClipboard(), 0);
    };
    const inspectClipboardOnFocus = (): void => {
      if (importVisible && preferences.smartClipboard) void inspectClipboard();
    };
    window.addEventListener("copy", inspectClipboardAfterEvent);
    window.addEventListener("paste", inspectClipboardAfterEvent);
    window.addEventListener("focus", inspectClipboardOnFocus);
    const consoleTimer = window.setInterval(() => { if (consoleVisible) void refreshConsole(); }, 350);
    void refreshSystemMetrics();
    const originalConsole = installConsoleForwarding();
    const handleWindowError = (event: ErrorEvent): void => console.error(event.error ?? event.message);
    const handleUnhandledRejection = (event: PromiseRejectionEvent): void => console.error("Unhandled promise rejection", event.reason);
    window.addEventListener("error", handleWindowError);
    window.addEventListener("unhandledrejection", handleUnhandledRejection);
    void loadUserPreferences().finally(() => void restoreLastProject());
    const finishPlaylistDrag = (): void => finishTrackDrag();
    const closeTrackContextMenu = (): void => { trackContextMenu = null; };
    window.addEventListener("pointerup", finishPlaylistDrag);
    window.addEventListener("pointercancel", finishPlaylistDrag);
    window.addEventListener("pointerdown", closeTrackContextMenu);
    void appWindow.onCloseRequested((event) => {
      if (!project?.temporary) return;
      event.preventDefault();
      closePromptVisible = true;
    }).then((stop) => unlistenClose = stop);
    void appWindow.onDragDropEvent((event) => {
      if (!importVisible) return;
      const position = "position" in event.payload ? event.payload.position : undefined;
      if (event.payload.type === "enter" || event.payload.type === "over") importDropActive = isImportDropTarget(position);
      else if (event.payload.type === "leave") importDropActive = false;
      else if (event.payload.type === "drop") {
        importDropActive = false;
        if (isImportDropTarget(position)) void acceptDroppedPaths(event.payload.paths);
      }
    }).then((stop) => unlistenDrag = stop);
    const savedEndBehavior = localStorage.getItem("sonarcan.endBehavior");
    if (savedEndBehavior === "restart" || savedEndBehavior === "advance" || savedEndBehavior === "stop") endBehavior = savedEndBehavior;
    void audioSetEndBehavior(endBehavior);
    return () => {
      window.removeEventListener("keydown", handleKeydown);
      window.removeEventListener("pointerover", handleHelpOver);
      window.removeEventListener("pointerout", handleHelpOut);
      window.removeEventListener("focusin", handleHelpFocus);
      window.removeEventListener("focusout", handleHelpBlur);
      window.clearInterval(statusTimer);
      window.clearInterval(spectrumTimer);
      window.clearInterval(stemTimer);
      window.clearInterval(metricsTimer);
      window.clearInterval(importTimer);
      if (clipboardTimer !== undefined) window.clearTimeout(clipboardTimer);
      window.removeEventListener("copy", inspectClipboardAfterEvent);
      window.removeEventListener("paste", inspectClipboardAfterEvent);
      window.removeEventListener("focus", inspectClipboardOnFocus);
      window.clearInterval(consoleTimer);
      window.clearTimeout(playbackRateTimer);
      window.clearTimeout(pitchTimer);
      window.clearTimeout(volumePreferenceTimer);
      window.clearTimeout(importAnalysisTimer);
      cancelPendingSeek();
      for (const timer of importDismissTimers.values()) window.clearTimeout(timer);
      importDismissTimers.clear();
      window.removeEventListener("error", handleWindowError);
      window.removeEventListener("unhandledrejection", handleUnhandledRejection);
      console.log = originalConsole.log;
      console.info = originalConsole.info;
      console.warn = originalConsole.warn;
      console.error = originalConsole.error;
      unlisten?.();
      window.removeEventListener("pointerup", finishPlaylistDrag);
      window.removeEventListener("pointercancel", finishPlaylistDrag);
      window.removeEventListener("pointerdown", closeTrackContextMenu);
      unlistenDrag?.();
      unlistenClose?.();
      unlistenExit?.();
    };
  });

  function installConsoleForwarding(): Pick<Console, "log" | "info" | "warn" | "error"> {
    const original = { log: console.log, info: console.info, warn: console.warn, error: console.error };
    for (const level of ["log", "info", "warn", "error"] as const) {
      console[level] = (...values: unknown[]) => {
        original[level](...values);
        const message = values.map(formatLogValue).join(" ");
        void pushFrontendLog(level === "log" ? "info" : level, message).catch(() => undefined);
      };
    }
    return original;
  }

  function formatLogValue(value: unknown): string {
    if (value instanceof Error) return `${value.name}: ${value.message}${value.stack ? `\n${value.stack}` : ""}`;
    if (typeof value === "string") return value;
    try { return JSON.stringify(value); } catch { return String(value); }
  }

  async function refreshConsole(): Promise<void> {
    try { appLogs = await logsSnapshot(); } catch { /* The app may be shutting down. */ }
  }

  function toggleConsole(): void {
    consoleVisible = !consoleVisible;
    if (consoleVisible) void refreshConsole();
  }

  async function loadUserPreferences(): Promise<void> {
    try {
      preferences = await getPreferences();
      language = preferences.language;
      volume = preferences.masterVolume;
      metronomeVolume = preferences.metronomeVolume;
      applyTheme();
      document.documentElement.lang = language;
      await setApplicationLanguage(language);
      await audioSetVolume(volume);
    } catch { applyTheme(); }
  }

  function applyTheme(): void { document.documentElement.dataset.theme = preferences.theme; }

  async function persistPreferences(): Promise<void> {
    preferences = await savePreferences(preferences);
    language = preferences.language;
    volume = preferences.masterVolume;
    metronomeVolume = preferences.metronomeVolume;
    applyTheme();
    document.documentElement.lang = language;
  }

  function changeLanguage(nextLanguage: Language): void {
    language = nextLanguage;
    preferences = { ...preferences, language };
    void persistPreferences();
  }

  async function restoreLastProject(): Promise<void> {
    if (project) return;
    try {
      const initialized = await initializeProject();
      if (project) return;
      await activateProject(initialized.project);
      if (initialized.unavailableProjectPath) {
        errorMessage = `${t("previousProjectUnavailable")} ${initialized.unavailableProjectPath}\n${t("temporaryProjectCreated")}`;
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
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
      window.clearTimeout(practiceSaveTimer);
      if (!await persistCurrentPracticeState()) return;
      await activateProject(await createTemporaryProject());
    });
  }

  function loadProject(): void {
    void run(async () => {
      const packagePath = await open({ directory: true, multiple: false, title: t("openProject") });
      if (!packagePath) return;
      window.clearTimeout(practiceSaveTimer);
      if (!await persistCurrentPracticeState()) return;
      await activateProject(await openProject(packagePath));
    });
  }

  function openRecent(packagePath: string): void {
    void run(async () => {
      window.clearTimeout(practiceSaveTimer);
      if (!await persistCurrentPracticeState()) return;
      await activateProject(await openProject(packagePath));
    });
  }

  async function activateProject(nextProject: ProjectSummary): Promise<void> {
    await resetTrackState();
    project = nextProject;
    const rememberedId = rememberedTrackId(window.localStorage, nextProject.packagePath);
    const track = preferredTrack(nextProject.tracks, rememberedId);
    if (track) selectTrack(track, false);
    else forgetTrackSelection(window.localStorage, nextProject.packagePath);
  }

  async function resetTrackState(): Promise<void> {
    ++trackSelectionGeneration;
    cancelPendingSeek();
    window.clearTimeout(playbackRateTimer);
    window.clearTimeout(pitchTimer);
    window.clearTimeout(practiceSaveTimer);
    playbackRateTimer = undefined;
    pitchTimer = undefined;
    practiceSaveTimer = undefined;
    currentTrack = null;
    loadingTrackId = null;
    isPlaying = false;
    audioLoading = false;
    waveformLoading = false;
    tempoLoading = false;
    currentSeconds = 0;
    durationSeconds = 0;
    playbackRate = preferences.defaultPlaybackRate;
    pitchSemitones = preferences.defaultPitchSemitones;
    volume = preferences.masterVolume;
    volumeBeforeMute = volume > 0 ? volume : 0.8;
    masterPeak = 0;
    masterPeakLeft = 0;
    masterPeakRight = 0;
    loopEnabled = false;
    loopA = null;
    loopB = null;
    usingDefaultLoopBounds = false;
    loopDrag = null;
    waveform = null;
    waveformZoom = 1;
    waveformStart = 0;
    waveformDragPointerId = null;
    viewportDrag = null;
    detectedBpm = null;
    gridBpm = null;
    beatGridOffsetSeconds = 0;
    metronomeEnabled = false;
    metronomeVolume = preferences.metronomeVolume;
    tapTimes = [];
    trainerEnabled = preferences.defaultTrainerEnabled;
    trainerRepetitions = preferences.defaultTrainerRepetitions;
    trainerIncrement = preferences.defaultTrainerIncrement;
    trainerTargetRate = preferences.defaultTrainerTargetRate;
    trainerLoopCount = 0;
    endedGeneration = 0;
    spectrumBands = Array<number>(64).fill(0);
    stems = { state: "disabled", progress: 0, stage: "disabled", trackId: null, cached: false, error: null, computeBackend: null };
    stemMix = Array.from({ length: 4 }, () => ({ gain: 1, muted: false, soloed: false }));
    editingTrackId = null;
    draggedTrackId = null;
    dropTrackId = null;
    dropTrackIndex = null;
    trackContextMenu = null;
    await audioPause();
    await stemDisable();
    await audioSetLoop(null, null);
    await audioSetMetronome(false, metronomeVolume);
    await audioSetLoopTrainer(false, trainerRepetitions, trainerIncrement, trainerTargetRate);
    await audioSetPlaybackRate(playbackRate);
    await audioSetPitch(pitchSemitones);
    await audioSeek(0);
  }

  function renameCurrentProject(): void {
    if (!project) return;
    editingProjectName = true;
    projectNameDraft = project.name;
  }

  function commitProjectName(): void {
    if (!editingProjectName || !project) return;
    editingProjectName = false;
    const name = projectNameDraft.trim();
    if (!name || name === project.name) return;
    void run(async () => {
      project = await renameProject(project!.packagePath, name);
    });
  }

  function cancelProjectName(): void {
    editingProjectName = false;
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

  function startTrackDrag(event: PointerEvent, trackId: string): void {
    if (event.button !== 0) return;
    event.preventDefault();
    draggedTrackId = trackId;
    dropTrackId = trackId;
    dropTrackIndex = project?.tracks.findIndex((track) => track.id === trackId) ?? null;
  }

  function finishTrackDrag(): void {
    const trackId = draggedTrackId;
    draggedTrackId = null;
    dropTrackId = null;
    const newIndex = dropTrackIndex;
    dropTrackIndex = null;
    if (!project || !trackId || newIndex === null) return;
    const oldIndex = project.tracks.findIndex((track) => track.id === trackId);
    if (oldIndex === newIndex) return;
    void run(async () => {
      project = await reorderTrack(project!.packagePath, trackId, newIndex);
      if (currentTrack) currentTrack = project.tracks.find((track) => track.id === currentTrack?.id) ?? null;
    });
  }

  function openTrackContextMenu(event: MouseEvent, trackId: string): void {
    event.preventDefault();
    const menuWidth = 190;
    const menuHeight = 48;
    trackContextMenu = {
      trackId,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - menuWidth - 8)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - menuHeight - 8)),
    };
  }

  async function removeTrack(track: TrackSummary): Promise<void> {
    trackContextMenu = null;
    if (!project || !window.confirm(t("confirmRemoveTrack"))) return;
    const wasCurrent = currentTrack?.id === track.id;
    const deletedIndex = project.tracks.findIndex((item) => item.id === track.id);
    await run(async () => {
      if (wasCurrent) await audioPause();
      const packagePath = project!.packagePath;
      project = await deleteTrackFromProject(packagePath, track.id);
      waveformCache.delete(`${packagePath}:${track.id}`);
      tempoCache.delete(`${packagePath}:${track.id}`);
      if (wasCurrent) {
        await resetTrackState();
        const replacement = project.tracks[deletedIndex] ?? project.tracks[project.tracks.length - 1];
        if (replacement) selectTrack(replacement, false);
        else forgetTrackSelection(window.localStorage, packagePath);
      }
    });
  }

  async function cancelImportJob(jobId: string): Promise<void> {
    await run(async () => {
      const timer = importDismissTimers.get(jobId);
      if (timer !== undefined) {
        window.clearTimeout(timer);
        importDismissTimers.delete(jobId);
      }
      await cancelImport(jobId);
      importQueue = importQueue.filter((job) => job.id !== jobId);
    });
  }

  async function dismissCompletedImport(jobId: string): Promise<void> {
    importDismissTimers.delete(jobId);
    try {
      await removeImportJob(jobId);
      importQueue = importQueue.filter((job) => job.id !== jobId);
    } catch { /* The queue is already ephemeral; the next refresh will reconcile it. */ }
  }

  function changeEndBehavior(behavior: EndBehavior): void {
    endBehavior = behavior;
    localStorage.setItem("sonarcan.endBehavior", behavior);
    void audioSetEndBehavior(behavior);
  }

  async function saveProjectToChosenLocation(): Promise<boolean> {
    if (!project) return false;
    window.clearTimeout(practiceSaveTimer);
    if (!await persistCurrentPracticeState()) return false;
    const destination = await save({
      title: t("saveProjectFile"),
      defaultPath: `${project.name.replace(/[\\/:*?"<>|]/g, "_")}.sac`,
      filters: [{ name: "SonArcan project", extensions: ["sac"] }],
    });
    if (!destination) return false;
    const sourcePackagePath = project.packagePath;
    const selectedTrackId = currentTrack?.id;
    project = await saveProjectAs(project.packagePath, destination);
    currentTrack = selectedTrackId
      ? project.tracks.find((track) => track.id === selectedTrackId) ?? null
      : null;
    if (selectedTrackId) rememberTrackSelection(window.localStorage, project.packagePath, selectedTrackId);
    else forgetTrackSelection(window.localStorage, project.packagePath);
    if (sourcePackagePath !== project.packagePath && selectedTrackId) {
      rememberTrackSelection(window.localStorage, sourcePackagePath, selectedTrackId);
    }
    return true;
  }

  function saveCurrentProject(): void {
    if (!project) return;
    void run(async () => {
      if (project?.temporary) await saveProjectToChosenLocation();
      else {
        window.clearTimeout(practiceSaveTimer);
        await persistCurrentPracticeState();
      }
    });
  }

  function saveAs(): void {
    if (!project) return;
    void run(async () => { await saveProjectToChosenLocation(); });
  }

  function requestApplicationClose(): void {
    void requestApplicationExit();
  }

  function closeWithoutSavingElsewhere(): void {
    closePromptVisible = false;
    void confirmApplicationExit();
  }

  async function saveTemporaryAndClose(): Promise<void> {
    busy = true;
    errorMessage = "";
    try {
      if (!await saveProjectToChosenLocation()) return;
      closePromptVisible = false;
      await confirmApplicationExit();
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      busy = false;
    }
  }

  async function handleNativeMenu(id: string): Promise<void> {
    if (id === "file:new") newProject();
    else if (id === "file:open") loadProject();
    else if (id === "file:import") openImportCenter();
    else if (id === "file:save") saveCurrentProject();
    else if (id === "file:save_as") saveAs();
    else if (id === "app:quit") requestApplicationClose();
    else if (id === "file:rename_project") renameCurrentProject();
    else if (id === "playlist:add") openImportCenter();
    else if (id === "playlist:export_json") exportCurrentPlaylist("json");
    else if (id === "playlist:export_markdown") exportCurrentPlaylist("markdown");
    else if (id.startsWith("recent:")) {
      const index = Number(id.slice("recent:".length));
      const recent = await listRecentProjects();
      if (Number.isInteger(index) && recent[index]) openRecent(recent[index]);
    } else if (id === "view:zoom_in") setWaveformZoom(waveformZoom * 1.5);
    else if (id === "view:console") toggleConsole();
    else if (id === "view:zoom_out") setWaveformZoom(waveformZoom / 1.5);
    else if (id === "view:zoom_reset") setWaveformZoom(1);
    else if (id === "playback:toggle") togglePlayback();
    else if (id === "playback:back") jump(-5);
    else if (id === "playback:forward") jump(5);
    else if (id === "playback:set_a") setLoopA();
    else if (id === "playback:set_b") setLoopB();
    else if (id === "playback:clear_loop") clearLoop();
    else if (id === "preferences" || id === "file:preferences") preferencesVisible = true;
    else if (id === "help:diagnostics") showDiagnostics();
    else if (id === "help:shortcuts") shortcutsVisible = true;
  }

  function setWaveformZoom(nextZoom: number): void {
    const center = waveformStart + 0.5 / waveformZoom;
    applyWaveformViewport(zoomWaveformViewport(waveformStart, waveformZoom, nextZoom / waveformZoom, center));
  }

  function addTracks(): void {
    openImportCenter();
  }

  function openImportCenter(): void {
    importVisible = true;
    if (preferences.smartClipboard) queueMicrotask(() => void inspectClipboard(true));
  }

  function toggleSmartClipboard(): void {
    preferences = { ...preferences, smartClipboard: !preferences.smartClipboard };
    clipboardDetected = 0;
    void persistPreferences();
    if (preferences.smartClipboard && importVisible) queueMicrotask(() => void inspectClipboard(true));
  }

  function isImportDropTarget(position: { x: number; y: number } | undefined): boolean {
    if (!position || !importTextarea) return false;
    const rect = importTextarea.getBoundingClientRect();
    const scale = window.devicePixelRatio || 1;
    const contains = (x: number, y: number): boolean => x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
    return contains(position.x, position.y) || contains(position.x / scale, position.y / scale);
  }

  async function exportCurrentPlaylist(format: "json" | "markdown"): Promise<void> {
    if (!project) return;
    const extension = format === "json" ? "json" : "md";
    const destination = await save({
      title: format === "json" ? t("exportJson") : t("exportMarkdown"),
      defaultPath: `${project.name}.${extension}`,
      filters: [{ name: format === "json" ? "JSON" : "Markdown", extensions: [extension] }],
    });
    if (!destination) return;
    await run(() => exportPlaylist(project!.packagePath, destination, format));
  }

  async function chooseImportFiles(): Promise<void> {
    const selected = await open({
        multiple: true,
        title: t("importAudio"), filters: [{ name: "Audio and text", extensions: ["wav", "mp3", "flac", "txt", "md"] }],
      });
    if (!selected) return;
    await acceptDroppedPaths(Array.isArray(selected) ? selected : [selected]);
  }

  async function acceptDroppedPaths(paths: string[]): Promise<void> {
    const textPaths = paths.filter((path) => /\.(txt|md)(?:[?#].*)?$/i.test(path));
    const audioPaths = paths.filter((path) => /\.(wav|mp3|flac)(?:[?#].*)?$/i.test(path));
    if (textPaths.length) importText = [importText, await readImportTextFiles(textPaths)].filter(Boolean).join("\n");
    if (audioPaths.length) {
      importText = [importText, ...audioPaths.map((path) => path.startsWith("file://") ? path : `file://${path}`)].filter(Boolean).join("\n");
    }
    importVisible = true;
    if (!textPaths.length && !audioPaths.length) {
      importAnalysisError = t("unsupportedDrop");
      return;
    }
    await analyzeImports();
  }

  async function analyzeImports(): Promise<void> {
    const generation = ++importAnalysisGeneration;
    importAnalyzing = true;
    importAnalysisError = "";
    importHasAnalyzed = false;
    try {
      let candidates = await analyzeImportText(importText);
      if (preferences.searchMode === "chooseFive") {
        const resolved: ImportCandidate[] = [];
        for (const candidate of candidates) {
          if (candidate.kind === "search") resolved.push(...await resolveYoutubeSearch(candidate.input));
          else resolved.push(candidate);
        }
        candidates = resolved;
      }
      if (generation !== importAnalysisGeneration) return;
      candidates = deduplicateImportCandidates(candidates);
      importCandidates = candidates;
      selectedImports = new Set(candidates.map((candidate) => candidate.input));
      importHasAnalyzed = true;
    } catch (error) {
      if (generation !== importAnalysisGeneration) return;
      importCandidates = [];
      selectedImports = new Set();
      importHasAnalyzed = true;
      importAnalysisError = error instanceof Error ? error.message : String(error);
    } finally { if (generation === importAnalysisGeneration) importAnalyzing = false; }
  }

  function scheduleImportAnalysis(): void {
    window.clearTimeout(importAnalysisTimer);
    importHasAnalyzed = false;
    importAnalysisTimer = window.setTimeout(() => void analyzeImports(), 260);
  }

  function toggleImport(input: string): void {
    const next = new Set(selectedImports);
    if (next.has(input)) next.delete(input); else next.add(input);
    selectedImports = next;
  }

  async function startImports(): Promise<void> {
    if (selectedImports.size === 0) return;
    const inputs = [...selectedImports];
    if (inputs.length > 10 && !window.confirm(`${t("confirmLargeImport")} (${inputs.length})`)) return;
    await run(async () => {
      if (!project) {
        await activateProject(await createTemporaryProject());
      }
      const activeProject = project;
      if (!activeProject) return;
      importQueue = await enqueueImports(activeProject.packagePath, inputs);
      importVisible = false; tasksVisible = false; importText = ""; importCandidates = []; selectedImports = new Set();
      await refreshImportJobs();
    });
  }

  async function refreshImportJobs(): Promise<void> {
    try {
      const previousCompleted = importQueue.filter((job) => job.state === "completed").length;
      const jobs = await importJobs();
      importQueue = jobs;
      for (const job of jobs) {
        if (job.state !== "completed" || importDismissTimers.has(job.id)) continue;
        const timer = window.setTimeout(() => void dismissCompletedImport(job.id), 2_500);
        importDismissTimers.set(job.id, timer);
      }
      const completed = importQueue.filter((job) => job.state === "completed").length;
      if (project && completed > previousCompleted) {
        project = await openProject(project.packagePath);
        if (currentTrack) currentTrack = project.tracks.find((track) => track.id === currentTrack?.id) ?? currentTrack;
        else {
          const track = preferredTrack(project.tracks, rememberedTrackId(window.localStorage, project.packagePath));
          if (track) selectTrack(track, false);
        }
      }
    } catch { /* Background status is best-effort during shutdown. */ }
  }

  async function inspectClipboard(force = false): Promise<void> {
    if (!preferences.smartClipboard) return;
    if (clipboardReadActive) {
      clipboardForcePending ||= force;
      return;
    }
    clipboardReadActive = true;
    let text = "";
    try {
      text = (await readClipboardText()).trim();
    } catch (error) {
      if (importVisible) {
        importCandidates = [];
        selectedImports = new Set();
        importHasAnalyzed = true;
        importAnalysisError = error instanceof Error ? error.message : String(error);
      }
      return;
    } finally {
      clipboardReadActive = false;
    }
    force ||= clipboardForcePending;
    clipboardForcePending = false;
    if (!text || (!force && text === lastClipboardText)) return;
    lastClipboardText = text;
    if (importVisible) {
      importText = text;
      clipboardDetected = 0;
      await analyzeImports();
      if (importVisible && importText === text && !importAnalyzing) clipboardDetected = importCandidates.length;
      return;
    }
    if (!project) return;
    try {
      const candidates = deduplicateImportCandidates(await analyzeImportText(text));
      if (!candidates.length) return;
      clipboardDetected = candidates.length;
      importText = text;
      if (preferences.searchMode === "automaticFirst") {
        if (candidates.length > 10 && !window.confirm(`${t("confirmLargeImport")} (${candidates.length})`)) return;
        await enqueueImports(project.packagePath, candidates.map((candidate) => candidate.input));
        await refreshImportJobs();
      }
    } catch { /* Clipboard inspection is optional and local. */ }
  }

  function showDiagnostics(): void {
    void run(async () => {
      diagnosticInfo = await diagnostics();
    });
  }

  function showPathInFileManager(path: string): void {
    void revealProject(path).catch((error) => {
      errorMessage = error instanceof Error ? error.message : String(error);
    });
  }

  function showProjectInFileManager(): void {
    if (project) showPathInFileManager(project.packagePath);
  }

  function openCommunityLink(target: "github" | "donate"): void {
    void openExternalLink(target).catch((error) => {
      errorMessage = error instanceof Error ? error.message : String(error);
    });
  }

  function selectTrack(track: TrackSummary, autoplay = true): void {
    if (!project) return;
    cancelPendingSeek();
    const packagePath = project.packagePath;
    const selectionGeneration = ++trackSelectionGeneration;
    window.clearTimeout(playbackRateTimer);
    window.clearTimeout(pitchTimer);
    playbackRateTimer = undefined;
    pitchTimer = undefined;
    void persistCurrentPracticeState();
    void audioPause();
    void stemDisable();
    stems = { state: "disabled", progress: 0, stage: "disabled", trackId: null, cached: false, error: null, computeBackend: null };
    isPlaying = false;
    masterPeak = 0;
    masterPeakLeft = 0;
    masterPeakRight = 0;
    audioLoading = true;
    loadingTrackId = track.id;
    currentTrack = track;
    rememberTrackSelection(window.localStorage, packagePath, track.id);
    currentSeconds = track.practice.positionSeconds;
    durationSeconds = track.durationSeconds ?? 0;
    playbackRate = track.practice.playbackRate;
    pitchSemitones = track.practice.pitchSemitones ?? 0;
    volume = preferences.masterVolume;
    volumeBeforeMute = volume > 0 ? volume : 0.8;
    loopEnabled = track.practice.loopEnabled ?? (track.practice.loopASeconds !== null && track.practice.loopBSeconds !== null);
    const loopBounds = defaultLoopBounds(track.practice.loopASeconds, track.practice.loopBSeconds, durationSeconds);
    loopA = loopBounds.a;
    loopB = loopBounds.b;
    usingDefaultLoopBounds = track.practice.loopASeconds === null && track.practice.loopBSeconds === null;
    gridBpm = track.practice.gridBpm ?? null;
    beatGridOffsetSeconds = track.practice.beatGridOffsetSeconds ?? 0;
    metronomeEnabled = track.practice.metronomeEnabled ?? false;
    metronomeVolume = preferences.metronomeVolume;
    trainerEnabled = track.practice.trainerEnabled ?? false;
    trainerRepetitions = track.practice.trainerRepetitions ?? 3;
    trainerIncrement = track.practice.trainerIncrement ?? 0.05;
    trainerTargetRate = track.practice.trainerTargetRate ?? 1;
    stemMix = track.practice.stemMix ?? Array.from({ length: 4 }, () => ({ gain: 1, muted: false, soloed: false }));
    stemMix.forEach((value, index) => void stemSetMix(index, value.gain, value.muted, value.soloed));
    trainerLoopCount = 0;
    spectrumBands = Array<number>(64).fill(0);
    tapTimes = [];
    void loadTrackWaveform(track, packagePath, selectionGeneration);
    void loadTrackTempo(track, packagePath, selectionGeneration);
    void loadSelectedAudio(track, packagePath, selectionGeneration, autoplay);
  }

  async function enableStems(): Promise<void> {
    if (!project || !currentTrack) return;
    stems = { state: "separating", progress: 0, stage: "checkingCache", trackId: currentTrack.id, cached: false, error: null, computeBackend: null };
    try { await stemStart(project.packagePath, currentTrack.id); schedulePracticeSave(); }
    catch (error) { errorMessage = `${t("stemFailed")}: ${error instanceof Error ? error.message : String(error)}`; }
  }

  async function disableStems(): Promise<void> {
    await stemDisable();
    stems = { state: "disabled", progress: 0, stage: "disabled", trackId: null, cached: false, error: null, computeBackend: null };
    schedulePracticeSave();
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
    schedulePracticeSave();
  }

  async function loadTrackTempo(track: TrackSummary, packagePath: string, selectionGeneration: number): Promise<void> {
    tempoLoading = true;
    detectedBpm = null;
    const cacheKey = `${packagePath}:${track.id}`;
    const stillSelected = (): boolean => selectionGeneration === trackSelectionGeneration
      && project?.packagePath === packagePath
      && currentTrack?.id === track.id;
    try {
      const analysis = tempoCache.get(cacheKey) ?? await analyzeTempo(packagePath, track.id);
      tempoCache.set(cacheKey, analysis);
      if (stillSelected()) {
        detectedBpm = analysis.bpm;
        if (gridBpm === null && analysis.bpm !== null) {
          gridBpm = analysis.bpm;
          applyBeatGridToEngine();
          schedulePracticeSave();
        }
      }
    } catch {
      if (stillSelected()) detectedBpm = null;
    } finally {
      if (stillSelected()) tempoLoading = false;
    }
  }

  async function loadSelectedAudio(track: TrackSummary, packagePath: string, selectionGeneration: number, autoplay: boolean): Promise<void> {
    const stillSelected = (): boolean => selectionGeneration === trackSelectionGeneration
      && project?.packagePath === packagePath
      && currentTrack?.id === track.id;
    try {
      const status = await audioLoad(packagePath, track.id);
      if (!stillSelected()) return;
      durationSeconds = status.durationSeconds;
      if (usingDefaultLoopBounds) loopB = durationSeconds;
      await audioSetVolume(volume);
      await audioSetPlaybackRate(playbackRate);
      await audioSetPitch(pitchSemitones);
      await audioSetBeatGrid(gridBpm, beatGridOffsetSeconds);
      await audioSetMetronome(metronomeEnabled, metronomeVolume);
      await audioSetLoopTrainer(trainerEnabled, trainerRepetitions, trainerIncrement, trainerTargetRate);
      await audioSetEndBehavior(endBehavior);
      if (!stillSelected()) return;
      endedGeneration = status.endedGeneration;
      applyLoopToEngine();
      await audioSeek(currentSeconds);
      if (!stillSelected()) return;
      audioLoading = false;
      loadingTrackId = null;
      if (track.practice.stemsEnabled) void enableStems();
      if (autoplay) await play();
      if (!stillSelected()) return;
      preloadNeighbour(track);
      void warmPlaylistCache(packagePath, track.id);
    } catch (error) {
      if (stillSelected()) {
        errorMessage = `${t("playbackError")}: ${error instanceof Error ? error.message : String(error)}`;
      }
    } finally {
      if (stillSelected()) {
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

  async function loadTrackWaveform(track: TrackSummary, packagePath: string, selectionGeneration: number): Promise<void> {
    waveformLoading = true;
    waveform = null;
    waveformZoom = 1;
    waveformStart = 0;
    const stillSelected = (): boolean => selectionGeneration === trackSelectionGeneration
      && project?.packagePath === packagePath
      && currentTrack?.id === track.id;
    try {
      const cacheKey = `${packagePath}:${track.id}`;
      const loaded = waveformCache.get(cacheKey) ?? await getWaveform(packagePath, track.id);
      waveformCache.set(cacheKey, loaded);
      if (stillSelected()) {
        waveform = loaded;
        if (loaded.durationSeconds > 0) durationSeconds = loaded.durationSeconds;
      }
    } catch (error) {
      if (stillSelected()) errorMessage = `${t("waveformError")}: ${error instanceof Error ? error.message : String(error)}`;
    } finally {
      if (stillSelected()) waveformLoading = false;
    }
  }

  async function play(): Promise<void> {
    if (!currentTrack && project?.tracks.length) selectTrack(project.tracks[0], false);
    if (!currentTrack || audioLoading) return;
    try {
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

  function seek(position: number): void {
    if (!Number.isFinite(position)) return;
    currentSeconds = Math.max(0, Math.min(position, durationSeconds));
    pendingSeekPosition = currentSeconds;
    if (seekAnimationFrame !== undefined) {
      window.cancelAnimationFrame(seekAnimationFrame);
      seekAnimationFrame = undefined;
    }
    void flushPendingSeek();
  }

  function scrub(position: number): void {
    if (!Number.isFinite(position)) return;
    currentSeconds = Math.max(0, Math.min(position, durationSeconds));
    pendingSeekPosition = currentSeconds;
    schedulePendingSeek();
  }

  function schedulePendingSeek(): void {
    if (seekAnimationFrame !== undefined || seekRequestActive) return;
    seekAnimationFrame = window.requestAnimationFrame(() => {
      seekAnimationFrame = undefined;
      void flushPendingSeek();
    });
  }

  async function flushPendingSeek(): Promise<void> {
    if (seekRequestActive || pendingSeekPosition === null) return;
    const position = pendingSeekPosition;
    pendingSeekPosition = null;
    seekRequestActive = true;
    try {
      await audioSeek(position);
    } catch (error) {
      errorMessage = `${t("playbackError")}: ${error instanceof Error ? error.message : String(error)}`;
    } finally {
      seekRequestActive = false;
      if (pendingSeekPosition !== null) schedulePendingSeek();
    }
  }

  function cancelPendingSeek(): void {
    pendingSeekPosition = null;
    if (seekAnimationFrame !== undefined) {
      window.cancelAnimationFrame(seekAnimationFrame);
      seekAnimationFrame = undefined;
    }
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

  function restoreDetectedBpm(): void {
    if (detectedBpm === null) return;
    changeGridBpm(detectedBpm);
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

  function resetBeatGridOrigin(): void {
    beatGridOffsetSeconds = 0;
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
    preferences = { ...preferences, metronomeVolume };
    void persistPreferences();
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
    return calculateBeatLines({
      bpm: gridBpm,
      durationSeconds,
      offsetSeconds: beatGridOffsetSeconds,
      detailed,
      zoom: waveformZoom,
      start: waveformStart,
    });
  }

  function setLoopA(): void {
    loopA = currentSeconds;
    if (usingDefaultLoopBounds || (loopB !== null && loopB <= loopA)) loopB = null;
    usingDefaultLoopBounds = false;
    loopEnabled = true;
    applyLoopToEngine();
    schedulePracticeSave();
  }

  function setLoopB(): void {
    if (loopA === null) loopA = 0;
    if (currentSeconds > loopA) loopB = currentSeconds;
    usingDefaultLoopBounds = false;
    loopEnabled = true;
    applyLoopToEngine();
    schedulePracticeSave();
  }

  function clearLoop(): void {
    loopA = null;
    loopB = null;
    usingDefaultLoopBounds = false;
    loopEnabled = false;
    void audioSetLoop(null, null);
    schedulePracticeSave();
  }

  function changeVolume(value: number): void {
    volume = Math.max(0, Math.min(1, value));
    if (volume > 0) volumeBeforeMute = volume;
    void audioSetVolume(volume);
    preferences = { ...preferences, masterVolume: volume };
    window.clearTimeout(volumePreferenceTimer);
    volumePreferenceTimer = window.setTimeout(() => void persistPreferences(), 180);
  }

  function setPlaybackRate(value: number): void {
    playbackRate = Math.round(Math.max(0.5, Math.min(2, value)) * 100) / 100;
    const target = playbackRate;
    window.clearTimeout(playbackRateTimer);
    playbackRateTimer = window.setTimeout(() => {
      playbackRateTimer = undefined;
      void audioSetPlaybackRate(target);
    }, 65);
    schedulePracticeSave(260);
  }

  function setPitch(value: number): void {
    pitchSemitones = Math.round(Math.max(-12, Math.min(12, value)) * 100) / 100;
    const target = pitchSemitones;
    window.clearTimeout(pitchTimer);
    pitchTimer = window.setTimeout(() => {
      pitchTimer = undefined;
      void audioSetPitch(target);
    }, 65);
    schedulePracticeSave(260);
  }

  function changePlaybackRate(delta: number): void {
    setPlaybackRate(playbackRate + delta);
  }

  function resetPlaybackRate(): void {
    setPlaybackRate(1);
  }

  function changePitch(delta: number): void {
    setPitch(pitchSemitones + delta);
  }

  function resetPitch(): void {
    setPitch(0);
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
    event.preventDefault();
    event.stopPropagation();
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    loopDrag = { mode, pointerId: event.pointerId, a: loopA, b: loopB };
  }

  function moveLoopDrag(event: PointerEvent, detailed: boolean): void {
    if (!loopDrag || loopDrag.pointerId !== event.pointerId || durationSeconds <= 0) return;
    const time = eventTime(event, detailed);
    const minimum = Math.min(0.05, durationSeconds);
    if (loopDrag.mode === "a") loopA = Math.max(0, Math.min(time, loopDrag.b - minimum));
    else loopB = Math.min(durationSeconds, Math.max(time, loopDrag.a + minimum));
    usingDefaultLoopBounds = false;
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
    const trackId = currentTrack.id;
    const selectionGeneration = trackSelectionGeneration;
    statusRequestActive = true;
    try {
      const status = await audioStatus();
      if (selectionGeneration !== trackSelectionGeneration || currentTrack?.id !== trackId) return;
      if (!seekRequestActive && pendingSeekPosition === null && seekAnimationFrame === undefined) {
        currentSeconds = status.positionSeconds;
      }
      durationSeconds = status.durationSeconds || durationSeconds;
      isPlaying = status.playing;
      masterPeak = status.outputPeak;
      masterPeakLeft = status.outputPeakLeft;
      masterPeakRight = status.outputPeakRight;
      if (status.endedGeneration !== endedGeneration) {
        endedGeneration = status.endedGeneration;
        if (endBehavior === "advance" && (project?.tracks.length ?? 0) > 1) {
          moveTrack(1);
          return;
        }
      }
      trainerLoopCount = status.trainerLoopCount;
      if (playbackRateTimer === undefined && Math.abs(playbackRate - status.playbackRate) > 0.0001) {
        playbackRate = status.playbackRate;
        schedulePracticeSave(300);
      }
      if (trainerEnabled !== status.trainerEnabled) {
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
    const trackId = currentTrack.id;
    const selectionGeneration = trackSelectionGeneration;
    spectrumRequestActive = true;
    try {
      const frame = await audioSpectrum();
      if (selectionGeneration === trackSelectionGeneration && currentTrack?.id === trackId) spectrumBands = frame.bands;
    } catch {
      // Spectrum visualization is optional and must never affect playback.
    } finally {
      spectrumRequestActive = false;
    }
  }

  async function refreshSystemMetrics(): Promise<void> {
    try {
      systemMetricsSnapshot = await systemMetrics();
    } catch {
      // System metrics are informational and must never affect playback.
    }
  }

  function toggleTheme(): void {
    const prefersLight = window.matchMedia("(prefers-color-scheme: light)").matches;
    const isDark = preferences.theme === "dark" || (preferences.theme === "system" && !prefersLight);
    preferences = { ...preferences, theme: isDark ? "light" : "dark" };
    applyTheme();
    void persistPreferences();
  }

  function schedulePracticeSave(delay = 700): void {
    window.clearTimeout(practiceSaveTimer);
    practiceSaveTimer = window.setTimeout(() => void persistCurrentPracticeState(), delay);
  }

  async function persistCurrentPracticeState(): Promise<boolean> {
    if (!project || !currentTrack) return true;
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
        stemsEnabled: stems.state !== "disabled",
        stemMix,
      });
      if (project?.packagePath === packagePath) project = updated;
      return true;
    } catch (error) {
      errorMessage = `${t("saveError")}: ${error instanceof Error ? error.message : String(error)}`;
      return false;
    }
  }

  function zoomWaveform(event: WheelEvent): void {
    event.preventDefault();
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const anchor = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    const factor = Math.exp(-event.deltaY * 0.002);
    const anchorPosition = waveformStart + anchor / waveformZoom;
    applyWaveformViewport(zoomWaveformViewport(waveformStart, waveformZoom, factor, anchorPosition));
  }

  function zoomOverviewWaveform(event: WheelEvent): void {
    event.preventDefault();
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const anchorPosition = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    applyWaveformViewport(zoomWaveformViewport(
      waveformStart,
      waveformZoom,
      Math.exp(-event.deltaY * 0.002),
      anchorPosition,
    ));
  }

  function applyWaveformViewport(viewport: WaveformViewport): void {
    waveformStart = viewport.start;
    waveformZoom = viewport.zoom;
  }

  function startWaveformDrag(event: PointerEvent): void {
    if (event.button !== 0) return;
    event.preventDefault();
    dragStartX = event.clientX;
    dragStartViewport = waveformStart;
    dragStartZoom = waveformZoom;
    dragMoved = false;
    waveformDragPointerId = event.pointerId;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function dragWaveform(event: PointerEvent): void {
    if (waveformDragPointerId !== event.pointerId || !(event.currentTarget as HTMLElement).hasPointerCapture(event.pointerId)) return;
    const width = (event.currentTarget as HTMLElement).clientWidth;
    const delta = event.clientX - dragStartX;
    if (Math.abs(delta) > 3) dragMoved = true;
    applyWaveformViewport(moveWaveformViewport(dragStartViewport, dragStartZoom, -delta / width / dragStartZoom));
  }

  function finishWaveformDrag(event: PointerEvent): void {
    if (waveformDragPointerId !== event.pointerId) return;
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    if (!dragMoved) {
      const bounds = target.getBoundingClientRect();
      const local = (event.clientX - bounds.left) / bounds.width;
      seek((waveformStart + local / waveformZoom) * durationSeconds);
    }
    waveformDragPointerId = null;
    dragMoved = false;
  }

  function cancelWaveformDrag(event: PointerEvent): void {
    if (waveformDragPointerId !== event.pointerId) return;
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    waveformDragPointerId = null;
    dragMoved = false;
  }

  function overviewRatio(event: PointerEvent): number {
    const overview = (event.currentTarget as HTMLElement).closest(".overview-wave") as HTMLElement | null;
    const bounds = (overview ?? event.currentTarget as HTMLElement).getBoundingClientRect();
    return Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
  }

  function startViewportDrag(event: PointerEvent, mode: ViewportDragMode): void {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    viewportDrag = {
      mode,
      pointerId: event.pointerId,
      originRatio: overviewRatio(event),
      originClientX: event.clientX,
      start: waveformStart,
      zoom: waveformZoom,
      moved: false,
    };
  }

  function moveViewportDrag(event: PointerEvent): void {
    if (!viewportDrag || viewportDrag.pointerId !== event.pointerId) return;
    const ratio = overviewRatio(event);
    if (Math.abs(event.clientX - viewportDrag.originClientX) > 3) viewportDrag.moved = true;
    const next = viewportDrag.mode === "move"
      ? moveWaveformViewport(viewportDrag.start, viewportDrag.zoom, ratio - viewportDrag.originRatio)
      : resizeWaveformViewport(viewportDrag.start, viewportDrag.zoom, viewportDrag.mode, ratio);
    applyWaveformViewport(next);
  }

  function finishViewportDrag(event: PointerEvent): void {
    if (!viewportDrag || viewportDrag.pointerId !== event.pointerId) return;
    const completedDrag = viewportDrag;
    const ratio = overviewRatio(event);
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    viewportDrag = null;
    if (completedDrag.mode === "move" && !completedDrag.moved) seekAndCenterOverview(ratio);
  }

  function cancelViewportDrag(event: PointerEvent): void {
    if (!viewportDrag || viewportDrag.pointerId !== event.pointerId) return;
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    viewportDrag = null;
  }

  function seekFromOverview(event: PointerEvent): void {
    if (event.button !== 0) return;
    seekAndCenterOverview(overviewRatio(event));
  }

  function seekAndCenterOverview(ratio: number): void {
    seek(ratio * durationSeconds);
    const span = 1 / waveformZoom;
    applyWaveformViewport(moveWaveformViewport(ratio - span / 2, waveformZoom, 0));
  }
</script>

<svelte:head><title>SonArcan</title></svelte:head>

<main class="shell" class:console-open={consoleVisible}>
  <header class="topbar">
    <div class="project-header">
      {#if project}
        <div class="project-name-wrap">
          {#if editingProjectName}
            <input class="project-name-input" bind:value={projectNameDraft} aria-label={t("projectName")} use:focusOnMount onblur={commitProjectName} onkeydown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); else if (event.key === "Escape") cancelProjectName(); }} />
          {:else}
            <button class="project-name" data-tooltip={t("projectName")} onclick={renameCurrentProject}>{project.name}</button>
          {/if}
        </div>
        <div class="project-path" aria-label={project.packagePath}>
          {#if projectHeaderPath}
            {#if projectHeaderPath.absolute}<span class="path-root">/</span>{/if}
            {#each projectHeaderPath.directoryParts as part, index}
              {#if index > 0}<span class="path-separator">/</span>{/if}
              <button class:project-path-ellipsis={part.ellipsis} class="project-path-part" title={part.ellipsis ? projectHeaderPath.directoryPath : part.path} onclick={() => showPathInFileManager(part.path)}>{part.label}</button>
            {/each}
            {#if projectHeaderPath.directoryParts.length > 0}<span class="path-separator">/</span>{/if}
          {/if}
          <button class="project-file" title={projectHeaderPath?.fullPath ?? project.packagePath} onclick={() => showPathInFileManager(project!.packagePath)}>
            <span class="project-file-stem">{projectHeaderPath?.fileStem ?? project.name}</span><span class="project-file-extension">{projectHeaderPath?.fileExtension}</span>
          </button>
        </div>
      {:else}
        <span class="project-empty">{t("noProject")}</span>
      {/if}
    </div>
    <div class="header-metrics" aria-label={t("systemMetrics")}>
      <span><small>{t("cpuUsage")}</small><strong>{systemMetricsSnapshot.cpuPercent === null ? "—" : `${systemMetricsSnapshot.cpuPercent.toFixed(1)}%`}</strong></span>
      <span><small>{t("memoryUsage")}</small><strong>{systemMetricsSnapshot.memoryMegabytes === null ? "—" : `${systemMetricsSnapshot.memoryMegabytes} MB`}</strong></span>
    </div>
    <div class="header-actions">
      <button class="header-icon-link" aria-label={t("toggleTheme")} data-tooltip={t("toggleTheme")} onclick={toggleTheme}>
        <Icon name={preferences.theme === "dark" ? "moon" : "sun"} size="15px" />
      </button>
      <button class="header-icon-link" aria-label={t("preferences")} data-tooltip={t("preferences")} onclick={() => preferencesVisible = true}><Icon name="gear" size="15px" /></button>
      <div class="master-output" aria-label={t("masterVolume")}>
        <button class="master-mute" class:muted={volume === 0} onclick={toggleMute} aria-label={volume > 0 ? t("mute") : t("unmute")} data-tooltip={volume > 0 ? t("mute") : t("unmute")}>
          {#if volume > 0}
            <Icon name="volume-high" size="15px" />
          {:else}
            <Icon name="volume-xmark" size="15px" />
          {/if}
        </button>
        <input aria-label={t("masterVolume")} type="range" min="0" max="1" step="0.01" value={volume} oninput={(event) => changeVolume(Number(event.currentTarget.value))} />
        <output>{Math.round(volume * 100)}%</output>
        <div class="master-meter" role="meter" aria-label={`${t("masterVolume")} ${Math.round(masterPeak * 100)}%`} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(masterPeak * 100)}>
          {#each masterMeterLevels as level}<i class:active={masterPeak * masterMeterLevels.length >= level}></i>{/each}
        </div>
      </div>
      <span class="header-separator" aria-hidden="true"></span>
      <button class="header-icon-link" aria-label={t("openGithub")} data-tooltip={t("openGithub")} onclick={() => openCommunityLink("github")}><Icon name="github" size="15px" /></button>
      <button class="header-icon-link donate" aria-label={t("supportProject")} data-tooltip={t("supportProject")} onclick={() => openCommunityLink("donate")}><Icon name="mug-hot" size="15px" /></button>
    </div>
  </header>

  {#if errorMessage}<div class="error" role="alert">{errorMessage}</div>{/if}

  <section class="workspace">
    <aside class="playlist panel">
      <div class="panel-title playlist-title"><h2>{t("playlist")}</h2><div><span class="count-badge">{project?.trackCount ?? 0}</span>{#if importQueue.length}<button class:failed={importQueue.some((job) => job.state === "failed")} class:complete={activeImports.length === 0 && !importQueue.some((job) => job.state === "failed")} class="playlist-task-orb" style={`--progress:${importProgress * 360}deg`} aria-label={t("importQueue")} data-tooltip={t("importQueue")} onclick={() => tasksVisible = true}><i></i><b>{importQueue.length}</b></button>{/if}<button class="playlist-add" aria-label={t("addSongs")} data-tooltip={t("addSongs")} onclick={openImportCenter}><Icon name="plus" size="13px" /></button></div></div>
      {#if project && project.tracks.length > 0}
        <ol>
          {#each project.tracks as track, index}
            <li
              class:active={track.id === currentTrack?.id}
              class:loading={track.id === loadingTrackId}
              class:drop-target={track.id === dropTrackId}
              onpointerenter={() => { if (draggedTrackId) { dropTrackId = track.id; dropTrackIndex = index; } }}
              oncontextmenu={(event) => openTrackContextMenu(event, track.id)}
            >
              <button class="drag-handle" aria-label={t("reorderSong")} data-tooltip={t("reorderSong")} onpointerdown={(event) => startTrackDrag(event, track.id)}><Icon name="grip-lines" size="12px" /></button>
              <button class="track-select" onclick={() => selectTrack(track)} aria-label={`${t("loadingTrack")} ${track.title}`}><span class="track-number">{#if track.id === loadingTrackId}<i class="mini-spinner" aria-label={t("loadingTrack")}></i>{:else}{String(index + 1).padStart(2, "0")}{/if}</span></button>
              <div class="track-info">
                {#if editingTrackId === track.id}
                  <input class="track-title-input" bind:value={editingTrackTitle} aria-label={t("trackName")} use:focusOnMount onblur={() => commitTrackRename(track)} onkeydown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); else if (event.key === "Escape") { editingTrackId = null; } }} />
                {:else}
                  <button class="track-title" onclick={() => startTrackRename(track)} data-tooltip={t("renameTrack")}>{track.title}</button>
                {/if}
                <button class="track-meta" onclick={() => selectTrack(track)}>{track.format.toUpperCase()} · {track.sampleRate ? `${track.sampleRate} Hz` : t("unknownRate")}</button>
              </div>
              <button class="track-remove" aria-label={t("removeTrack")} data-tooltip={t("removeTrack")} onclick={(event) => { event.stopPropagation(); void removeTrack(track); }}><Icon name="trash" size="13px" /></button>
            </li>
          {/each}
        </ol>
      {:else}
        <div class="empty">{t("emptyPlaylist")}</div>
      {/if}
    </aside>

    <section class="main-stage">
      {#if currentTrack}
      <div class="visualizer panel">
        <div class="panel-title"><h2>{t("waveform")}</h2><div class="load-states">{#if audioLoading}<span><i class="mini-spinner"></i>{t("loadingAudio")}</span>{/if}{#if waveformLoading}<span><i class="mini-spinner"></i>{t("waveformLoading")}</span>{/if}{#if tempoLoading}<span><i class="mini-spinner"></i>{t("bpmAnalyzing")}</span>{:else if detectedBpm !== null}<button class="bpm" data-tooltip={t("restoreDetectedBpm")} onclick={restoreDetectedBpm}><Icon name="rotate-left" size="11px" /> {detectedBpm.toFixed(1)} BPM</button>{/if}{#if currentTrack && !audioLoading && !waveformLoading}<span class="loaded"><Icon name="check" size="10px" /> {t("audioReady")}</span>{/if}</div></div>
        <div
          class="wave detailed-wave"
          class:dragging={waveformDragPointerId !== null}
          role="application"
          aria-label={t("waveform")}
          data-tooltip={t("seekHelp")}
          onwheel={zoomWaveform}
          onpointerdown={startWaveformDrag}
          onpointermove={dragWaveform}
          onpointerup={finishWaveformDrag}
          onpointercancel={cancelWaveformDrag}
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
            {#if loopEnabled && loopA !== null}
              <button class="loop-handle a" style={`left:${(loopA / durationSeconds - waveformStart) * waveformZoom * 100}%`} aria-label={t("moveStart")} data-tooltip={t("moveStart")} onpointerdown={(event) => startLoopDrag(event, "a", true)} onpointermove={(event) => moveLoopDrag(event, true)} onpointerup={finishLoopDrag} onpointercancel={finishLoopDrag}>A</button>
              {#if loopB !== null}
                <i
                  class="loop-region"
                  aria-hidden="true"
                  style={`left:${(loopA / durationSeconds - waveformStart) * waveformZoom * 100}%;width:${(loopB - loopA) / durationSeconds * waveformZoom * 100}%`}
                ></i>
                <button class="loop-handle b" style={`left:${(loopB / durationSeconds - waveformStart) * waveformZoom * 100}%`} aria-label={t("moveEnd")} data-tooltip={t("moveEnd")} onpointerdown={(event) => startLoopDrag(event, "b", true)} onpointermove={(event) => moveLoopDrag(event, true)} onpointerup={finishLoopDrag} onpointercancel={finishLoopDrag}>B</button>
              {/if}
            {/if}
            {#if playheadPercent >= 0 && playheadPercent <= 100}<i class="playhead" style={`left:${playheadPercent}%`}></i>{/if}
          {/if}
        </div>
        <div class="zoom-info"><span>{waveformZoom.toFixed(1)}×</span><span>{t("waveformHelp")}</span></div>
        <div class="overview-wave" role="application" aria-label={t("overviewHelp")} data-tooltip={t("overviewHelp")} onwheel={zoomOverviewWaveform} onpointerdown={seekFromOverview}>
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
            <button type="button" class="viewport" class:dragging={viewportDrag?.mode === "move"} aria-label={t("moveViewport")} style={`left:${waveformStart * 100}%;width:${100 / waveformZoom}%`} onpointerdown={(event) => startViewportDrag(event, "move")} onpointermove={moveViewportDrag} onpointerup={finishViewportDrag} onpointercancel={cancelViewportDrag}></button>
            <button type="button" class="viewport-handle start" aria-label={t("resizeViewportStart")} data-tooltip={t("resizeViewportStart")} style={`left:${waveformStart * 100}%`} onpointerdown={(event) => startViewportDrag(event, "start")} onpointermove={moveViewportDrag} onpointerup={finishViewportDrag} onpointercancel={cancelViewportDrag}></button>
            <button type="button" class="viewport-handle end" aria-label={t("resizeViewportEnd")} data-tooltip={t("resizeViewportEnd")} style={`left:${(waveformStart + 1 / waveformZoom) * 100}%`} onpointerdown={(event) => startViewportDrag(event, "end")} onpointermove={moveViewportDrag} onpointerup={finishViewportDrag} onpointercancel={cancelViewportDrag}></button>
            {#if loopEnabled && loopA !== null}
              <button class="loop-handle overview a" style={`left:${loopA / durationSeconds * 100}%`} aria-label={t("moveStart")} data-tooltip={t("moveStart")} onpointerdown={(event) => startLoopDrag(event, "a", false)} onpointermove={(event) => moveLoopDrag(event, false)} onpointerup={finishLoopDrag} onpointercancel={finishLoopDrag}>A</button>
              {#if loopB !== null}
                <i class="loop-region overview" aria-hidden="true" style={`left:${loopA / durationSeconds * 100}%;width:${(loopB - loopA) / durationSeconds * 100}%`}></i>
                <button class="loop-handle overview b" style={`left:${loopB / durationSeconds * 100}%`} aria-label={t("moveEnd")} data-tooltip={t("moveEnd")} onpointerdown={(event) => startLoopDrag(event, "b", false)} onpointermove={(event) => moveLoopDrag(event, false)} onpointerup={finishLoopDrag} onpointercancel={finishLoopDrag}>B</button>
              {/if}
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
          oninput={(event) => scrub(Number(event.currentTarget.value))}
          onchange={(event) => seek(Number(event.currentTarget.value))}
        />
        <div class="loop-status">
          <span>A {loopA === null ? "—" : formatTime(loopA)}</span>
          <strong class="playback-position" aria-label={t("playbackPosition")}>{formatTime(currentSeconds)}</strong>
          <span>B {loopB === null ? "—" : formatTime(loopB)}</span>
        </div>
      </div>

      <div class="transport panel">
        <div class="transport-parameters">
          <NumericControl label={t("tempo")} value={playbackRate} defaultValue={1} minimum={0.5} maximum={2} step={0.01} buttonStep={0.05} shiftButtonStep={0.01} display={(value) => `${Math.round(value * 100)}%`} onChange={setPlaybackRate} tooltip={t("numericHelp")} />
          <NumericControl label={t("pitch")} value={pitchSemitones} defaultValue={0} minimum={-12} maximum={12} step={0.01} buttonStep={1} shiftButtonStep={0.01} display={formatPitch} onChange={setPitch} tooltip={t("pitchFineHelp")} />
        </div>
        <div class="transport-center">
          <button disabled={audioLoading} aria-label={t("back5")} data-tooltip={t("back5")} onclick={() => jump(-5)}><Icon name="arrow-left" size="11px" />5s</button>
          <button disabled={audioLoading} class="round" aria-label={t("previous")} data-tooltip={t("previous")} onclick={() => moveTrack(-1)}><Icon name="backward-step" size="15px" /></button>
          <button disabled={audioLoading} class="play" class:loading={audioLoading} aria-label={audioLoading ? t("loadingAudio") : isPlaying ? t("pause") : t("play")} data-tooltip={audioLoading ? t("loadingAudio") : isPlaying ? t("pause") : t("play")} onclick={togglePlayback}>{#if audioLoading}<i class="button-spinner"></i>{:else}<Icon name={isPlaying ? "pause" : "play"} size="15px" />{/if}</button>
          <button disabled={audioLoading} class="round" aria-label={t("next")} data-tooltip={t("next")} onclick={() => moveTrack(1)}><Icon name="forward-step" size="15px" /></button>
          <button disabled={audioLoading} aria-label={t("forward5")} data-tooltip={t("forward5")} onclick={() => jump(5)}>5s<Icon name="arrow-right" size="11px" /></button>
        </div>
        <div class="end-behavior" role="group" aria-label={t("endBehavior")}>
          <button class:active={endBehavior === "restart"} aria-pressed={endBehavior === "restart"} aria-label={t("restartAtEnd")} data-tooltip={t("restartAtEnd")} onclick={() => changeEndBehavior("restart")}><Icon name="rotate-right" size="13px" /></button>
          <button class:active={endBehavior === "advance"} aria-pressed={endBehavior === "advance"} aria-label={t("advanceAtEnd")} data-tooltip={t("advanceAtEnd")} onclick={() => changeEndBehavior("advance")}><Icon name="forward-step" size="13px" /></button>
          <button class:active={endBehavior === "stop"} aria-pressed={endBehavior === "stop"} aria-label={t("stopAtEnd")} data-tooltip={t("stopAtEnd")} onclick={() => changeEndBehavior("stop")}><Icon name="stop" size="13px" /></button>
        </div>
      </div>

      <div class="practice panel">
        <div class="loop-controls">
          <div class="control-group loop-actions"><button class="loop-action-a" onclick={setLoopA} data-tooltip={t("moveA")}>A</button><button onclick={clearLoop} data-tooltip={t("resetAB")} aria-label={t("resetAB")}><Icon name="xmark" size="11px" /></button><button class="loop-action-b" onclick={setLoopB} data-tooltip={t("moveB")}>B</button><button class:active={loopEnabled} onclick={toggleLoop} aria-pressed={loopEnabled} data-tooltip={t("toggleLoop")}><Icon name="rotate-right" size="11px" /> {t("loop")}</button></div>
        </div>
        <div class="transport-trainer">
          <button class="trainer-toggle" class:active={trainerEnabled} aria-pressed={trainerEnabled} aria-label={t("loopTrainer")} data-tooltip={t("trainerHelp")} onclick={toggleLoopTrainer}><i class="stair-icon"><b></b><b></b><b></b></i><span>{trainerLoopCount}/{trainerRepetitions}</span></button>
          <NumericControl label={t("repetitions")} value={trainerRepetitions} defaultValue={3} minimum={1} maximum={99} step={1} display={(value) => String(value)} onChange={updateTrainerRepetitions} tooltip={t("numericHelp")} />
          <NumericControl label={t("increment")} value={trainerIncrement * 100} defaultValue={5} minimum={1} maximum={25} step={1} display={(value) => `+${value}%`} onChange={updateTrainerIncrement} tooltip={t("numericHelp")} />
          <NumericControl label={t("targetTempo")} value={trainerTargetRate * 100} defaultValue={100} minimum={50} maximum={200} step={5} display={(value) => `${value}%`} onChange={updateTrainerTarget} tooltip={t("numericHelp")} />
          <div class="transport-trainer-progress"><i style={`width:${Math.max(0, Math.min(100, trainerLoopCount / trainerRepetitions * 100))}%`}></i></div>
        </div>
      </div>

      <div class="tempo-grid panel">
        <div class="tempo-editor"><NumericControl label={t("gridTempo")} value={gridBpm ?? detectedBpm ?? 120} defaultValue={detectedBpm ?? 120} minimum={30} maximum={300} step={0.1} display={(value) => `${value.toFixed(1)} BPM`} onChange={changeGridBpm} onTap={tapTempo} tooltip={t("tapTempoHelp")} /><button class="reset-value" disabled={detectedBpm === null} data-tooltip={t("restoreDetectedBpm")} onclick={restoreDetectedBpm}><Icon name="rotate-left" size="12px" /><span><small>{t("detected")}</small><strong>{detectedBpm === null ? "—" : `${detectedBpm.toFixed(1)} BPM`}</strong></span></button></div>
        <div class="grid-anchor">
          <button class="align-first-beat" disabled={gridBpm === null} data-tooltip={t("setGridAnchorHelp")} onclick={setBeatGridAnchor}><i>Ⅰ</i><span>{t("setGridAnchor")}</span></button>
          <button class="reset-value" disabled={gridBpm === null || beatGridOffsetSeconds === 0} data-tooltip={t("resetGridOriginHelp")} onclick={resetBeatGridOrigin}><Icon name="rotate-left" size="12px" /><span><small>{t("gridAnchor")}</small><strong>{formatTimePrecise(0)}</strong></span></button>
          <div class="compact-controls"><button disabled={gridBpm === null} data-tooltip={t("nudgeGridBack")} onclick={() => nudgeBeatGrid(-10)}>−10 ms</button><button disabled={gridBpm === null} data-tooltip={t("nudgeGridForward")} onclick={() => nudgeBeatGrid(10)}>+10 ms</button></div>
          <output>{t("gridAnchor")}: {formatTimePrecise(beatGridOffsetSeconds)}</output>
        </div>
        <div class="metronome-control">
          <button class:active={metronomeEnabled} disabled={gridBpm === null} aria-pressed={metronomeEnabled} data-tooltip={t("metronomeHelp")} onclick={toggleMetronome}><Icon name="music" size="12px" /> {t("metronome")}</button>
          <NumericControl label={t("metronomeVolume")} value={metronomeVolume} defaultValue={0.55} minimum={0} maximum={1} step={0.05} display={(value) => `${Math.round(value * 100)}%`} onChange={changeMetronomeVolume} tooltip={t("numericHelp")} />
        </div>
      </div>

      <div class="visualization-row">
        <div class="spectrum panel">
          <div class="panel-title"><h2>{t("spectrum")}</h2><span>30 Hz — 20 kHz · FFT 2048</span></div>
          <div class="spectrum-bars" aria-label={t("spectrum")}>
            {#each spectrumBands as magnitude, index}<i style={`height:${Math.max(1, magnitude * 100)}%;--band:${index}`}></i>{/each}
          </div>
          <div class="spectrum-scale"><span>30</span><span>100</span><span>1k</span><span>10k</span><span>20k Hz</span></div>
        </div>
        <div class="stereo-meter panel">
          <div class="panel-title"><h2>{t("stereoMeter")}</h2><span>L / R</span></div>
          <div class="stereo-meter-channels">
            <div class="stereo-channel"><span>L</span><div class="stereo-track" role="meter" aria-label={`${t("leftChannel")} ${Math.round(masterPeakLeft * 100)}%`} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(masterPeakLeft * 100)}><i style={`width:${Math.min(100, Math.max(0, masterPeakLeft * 100))}%`}></i></div><output>{Math.round(masterPeakLeft * 100)}%</output></div>
            <div class="stereo-channel"><span>R</span><div class="stereo-track" role="meter" aria-label={`${t("rightChannel")} ${Math.round(masterPeakRight * 100)}%`} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(masterPeakRight * 100)}><i style={`width:${Math.min(100, Math.max(0, masterPeakRight * 100))}%`}></i></div><output>{Math.round(masterPeakRight * 100)}%</output></div>
          </div>
        </div>
      </div>

      <div class="lower-grid">
        <div class="panel stem-panel">
          <div class="panel-title"><h2>{t("stems")}</h2><div class="stem-heading-status">{#if stems.computeBackend}<span class:cpu={stems.computeBackend === "CPU"} class="stem-backend">{stems.computeBackend}</span>{/if}<span>{stems.state === "ready" ? t("stemsReady") : stems.state === "failed" ? t("stemFailed") : t("idle")}</span></div></div>
          {#if stems.state === "disabled"}
            <div class="stem-empty"><button class="primary" data-tooltip={t("stemHelp")} disabled={!currentTrack} onclick={enableStems}>{t("enableStems")}</button><small>HTDemucs · 4 stems · local</small></div>
          {:else if stems.state === "downloading" || stems.state === "separating"}
            <div class="stem-progress"><div class="stem-progress-label"><span class="mini-spinner"></span><span>{stems.state === "downloading" ? t("downloadingModel") : stems.stage === "cpuFallback" ? t("cpuStemFallback") : t("separatingStems")}</span><b>{Math.round(stems.progress * 100)}%</b></div><i><b style={`width:${Math.max(1, stems.progress * 100)}%`}></b></i><button onclick={disableStems}>{t("disableStems")}</button></div>
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
      {:else}
        <div class="no-track-stage panel" role="region" aria-labelledby="no-track-title">
          <span class="no-track-icon" aria-hidden="true"><Icon name="music" size="42px" /></span>
          <h2 id="no-track-title">{t("noTrackTitle")}</h2>
          <p>{t("noTrackMessage")}</p>
          <button class="primary no-track-import" onclick={openImportCenter}><Icon name="plus" size="13px" /> {t("importFirstTrack")}</button>
        </div>
      {/if}
    </section>
  </section>

  <footer>
    <span>{busy ? t("working") : t("ready")}</span>
    <div class="help-strip" aria-live="polite"><Icon name="lightbulb" size="12px" /><span>{helpMessage || t("helpHover")}</span></div>
    <button class="link" onclick={showDiagnostics}>{t("diagnostics")}</button>
  </footer>

  {#if consoleVisible}
    <section class="app-console" aria-label={t("applicationConsole")}>
      <header><div><strong>{t("applicationConsole")}</strong><span>{appLogs.length} {t("logEntries")}</span></div><button aria-label={t("hideConsole")} data-tooltip={t("hideConsole")} onclick={toggleConsole}><Icon name="xmark" size="12px" /></button></header>
      <div class="console-output">
        {#if appLogs.length === 0}<p class="console-empty">{t("noLogs")}</p>{/if}
        {#each appLogs as entry}
          <div class={`console-line ${entry.level}`}><time>{new Date(entry.timestampMs).toLocaleTimeString(language, { hour12: false })}</time><b>{entry.origin === "rust" ? "RUST" : "WEB"}</b><em>{entry.level.toUpperCase()}</em><pre>{entry.message}</pre></div>
        {/each}
      </div>
    </section>
  {/if}

  {#if closePromptVisible}
    <Modal title={t("saveTemporaryTitle")} close={() => closePromptVisible = false}>
      <p>{t("saveTemporaryPrompt")}</p>
      <div class="modal-actions">
        <button onclick={() => closePromptVisible = false}>{t("cancel")}</button>
        <button onclick={closeWithoutSavingElsewhere}>{t("quitWithoutSaving")}</button>
        <button class="primary" disabled={busy} onclick={() => void saveTemporaryAndClose()}>{t("saveAndQuit")}</button>
      </div>
    </Modal>
  {/if}

  {#if diagnosticInfo}
    <Modal title={t("diagnostics")} close={() => diagnosticInfo = null}><dl><dt>{t("version")}</dt><dd>{diagnosticInfo.appVersion}</dd><dt>OS</dt><dd>{diagnosticInfo.os}</dd><dt>{t("architecture")}</dt><dd>{diagnosticInfo.architecture}</dd><dt>{t("logging")}</dt><dd>{diagnosticInfo.rustLog}</dd></dl><button onclick={() => diagnosticInfo = null}>{t("close")}</button></Modal>
  {/if}

  {#if preferencesVisible}
    <Modal title={t("preferences")} wide close={() => preferencesVisible = false}>
      <div class="preferences-grid">
        <section><h3>{t("appearance")}</h3><label>{t("language")}<select bind:value={preferences.language}><option value="fr">{t("french")}</option><option value="en">{t("english")}</option></select></label><label>{t("theme")}<select bind:value={preferences.theme}><option value="system">{t("system")}</option><option value="dark">{t("dark")}</option><option value="light">{t("light")}</option></select></label></section>
        <section><h3>{t("importSettings")}</h3><label class="check"><input type="checkbox" bind:checked={preferences.smartClipboard} /> {t("smartClipboard")}</label><label>{t("searchMode")}<select bind:value={preferences.searchMode}><option value="automaticFirst">{t("automaticFirst")}</option><option value="chooseFive">{t("chooseFive")}</option></select></label><label>{t("simultaneousDownloads")}<input type="number" min="1" max="8" bind:value={preferences.concurrentDownloads} /></label><label>{t("conversionFormat")}<select bind:value={preferences.conversionFormat}><option value="keep">{t("keepSupported")}</option><option value="mp3">MP3</option><option value="wav">WAV</option><option value="flac">FLAC</option></select></label><label>{t("sampleRate")}<select bind:value={preferences.sampleRate}><option value="preserve">{t("preserve")}</option><option value="hz44100">44.1 kHz</option><option value="hz48000">48 kHz</option></select></label><label>{t("channels")}<select bind:value={preferences.channels}><option value="preserve">{t("preserve")}</option><option value="stereo">{t("stereo")}</option><option value="mono">{t("mono")}</option></select></label></section>
        <section><h3>{t("practiceDefaults")}</h3><label>{t("repetitions")}<input type="number" min="1" max="99" bind:value={preferences.defaultTrainerRepetitions} /></label><label>{t("increment")}<input type="number" min="1" max="25" value={preferences.defaultTrainerIncrement * 100} onchange={(event) => preferences.defaultTrainerIncrement = Number(event.currentTarget.value) / 100} /></label><label>{t("targetTempo")}<input type="number" min="50" max="200" value={preferences.defaultTrainerTargetRate * 100} onchange={(event) => preferences.defaultTrainerTargetRate = Number(event.currentTarget.value) / 100} /></label></section>
        <section><h3>Audio</h3><label>{t("masterVolume")}<input type="range" min="0" max="1" step="0.01" bind:value={preferences.masterVolume} /></label><label>{t("metronomeVolume")}<input type="range" min="0" max="1" step="0.01" bind:value={preferences.metronomeVolume} /></label></section>
      </div><div class="modal-actions"><button onclick={() => preferencesVisible = false}>{t("close")}</button><button class="primary" onclick={() => { void persistPreferences(); preferencesVisible = false; }}>{t("savePreferences")}</button></div>
    </Modal>
  {/if}

  {#if shortcutsVisible}
    <Modal title={t("shortcuts")} close={() => shortcutsVisible = false}><dl><dt>{t("playPause")}</dt><dd>Space</dd><dt>{t("jump")}</dt><dd>← / →</dd><dt>{t("loopAB")}</dt><dd>A / B</dd><dt>{t("clearLoop")}</dt><dd>Escape</dd><dt>{t("tempo")}</dt><dd>− / +</dd><dt>{t("tapTempo")}</dt><dd>T</dd><dt>{t("metronome")}</dt><dd>M</dd></dl><button onclick={() => shortcutsVisible = false}>{t("close")}</button></Modal>
  {/if}

  {#if importVisible}
    <Modal title={t("importCenter")} wide close={() => importVisible = false}>
      <div class="import-center" role="region" aria-label={t("importCenter")}>
        <div class="import-toolbar">
          <div class="import-toolbar-actions">
            <button onclick={chooseImportFiles}>{t("addFiles")}</button>
            <button
              class:active={preferences.smartClipboard}
              class="clipboard-toggle"
              type="button"
              aria-pressed={preferences.smartClipboard}
              aria-label={t("smartClipboard")}
              data-tooltip={t("smartClipboard")}
              onclick={toggleSmartClipboard}
            >
              <Icon name="clipboard" size="12px" />
              <span>{t("smartClipboard")}</span>
              {#if clipboardDetected}<b>{clipboardDetected}</b>{/if}
            </button>
          </div>
          <span>{t("importLimit")}</span>
        </div>
        <textarea bind:this={importTextarea} class:drop-active={importDropActive} bind:value={importText} oninput={scheduleImportAnalysis} ondragover={(event) => { event.preventDefault(); importDropActive = true; }} ondragleave={() => importDropActive = false} ondrop={(event) => { event.preventDefault(); importDropActive = false; const text = event.dataTransfer?.getData("text/plain"); if (text) { importText = [importText, text].filter(Boolean).join("\n"); void analyzeImports(); } }} placeholder={t("importPlaceholder")}></textarea>
        <div class="import-analysis-state">
          {#if importAnalyzing}<span><i class="mini-spinner"></i>{t("analyzingSources")}</span>
          {:else if importAnalysisError}<span class="failed">{importAnalysisError}</span>
          {:else if importHasAnalyzed}<span>{importCandidates.length} {t("sourcesFound")}</span>
          {:else}<span>{t("dropToAnalyze")}</span>{/if}
        </div>
        {#if importCandidates.length}
          <div class="candidate-list">{#each importCandidates as candidate}<button class:selected={selectedImports.has(candidate.input)} onclick={() => toggleImport(candidate.input)}><i>{selectedImports.has(candidate.input) ? "✓" : ""}</i><span><strong>{candidate.title}</strong><small>{candidate.detail}</small></span></button>{/each}</div>
        {:else if importHasAnalyzed && !importAnalysisError}
          <div class="import-empty">{t("noSourcesFound")}</div>
        {/if}
        <small class="authorized-note">{t("authorizedOnly")}</small>
        <div class="modal-actions"><button onclick={() => importVisible = false}>{t("close")}</button><button class="primary" disabled={selectedImports.size === 0 || importAnalyzing} onclick={startImports}>{t("startImport")} ({selectedImports.size})</button></div>
      </div>
    </Modal>
  {/if}

  {#if tasksVisible}
    <Modal title={t("importQueue")} wide close={() => tasksVisible = false}>{#if !importQueue.length}<p>{t("noTasks")}</p>{:else}<div class="job-list">{#each [...importQueue].reverse() as job}<article class:failed={job.state === "failed"}><div class="job-heading"><span><strong>{job.label}</strong><span>{t(job.state as MessageKey)} · {Math.round(job.progress * 100)}%</span></span><button class="job-remove" aria-label={t("cancelImport")} data-tooltip={t("cancelImport")} onclick={() => void cancelImportJob(job.id)}><Icon name="xmark" size="11px" /></button></div><i><b style={`width:${job.progress * 100}%`}></b></i>{#if job.error}<p>{job.error}</p>{/if}{#if job.suggestion}<small>{job.suggestion}</small>{/if}{#if job.diagnostic}<details><summary>{t("technicalDetails")}</summary><pre>{job.diagnostic}</pre></details>{/if}</article>{/each}</div>{/if}<button onclick={() => tasksVisible = false}>{t("close")}</button></Modal>
  {/if}

  {#if trackContextMenu}
    <div class="context-menu" role="menu" tabindex="-1" style={`left:${trackContextMenu.x}px;top:${trackContextMenu.y}px`} onpointerdown={(event) => event.stopPropagation()}>
      <button onclick={() => { const track = project?.tracks.find((item) => item.id === trackContextMenu?.trackId); if (track) void removeTrack(track); }}>{t("removeTrack")}</button>
    </div>
  {/if}
</main>
