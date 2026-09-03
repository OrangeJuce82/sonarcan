<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { handleWindowCloseRequest, projectOpenDialogOptions } from "./lib/applicationLifecycle";
  import { analyzeChords, analyzeImportText, audioLoad, audioPause, audioPlay, audioPreload, audioSeek, audioSetBeatTimeline, audioSetEndBehavior, audioSetLoop, audioSetLoopTrainer, audioSetLoudnessNormalization, audioSetMetronome, audioSetMusicVolume, audioSetPitch, audioSetPlaybackRate, audioSetVolume, audioSpectrum, audioStatus, beginYoutubeSearches, cancelChordAnalysis, cancelImport, confirmApplicationExit, createTemporaryProject, deleteTrack as deleteTrackFromProject, diagnostics, enqueueImports, exportChords, exportPlaylist, exportStems, getPreferences, getWaveform, importJobs, initializeProject, listRecentProjects, logsSnapshot, openExternalLink, openProject, openYoutubeVideo, pushFrontendLog, readImportTextFiles, removeImportJob, renameProject, renameTrack, reorderTrack, requestApplicationExit, resolveYoutubeSearch, revealProject, savePreferences, saveProjectAs, setApplicationLanguage, stemDisable, stemSetEnabled, stemSetMix, stemStart, stemStatus, systemMetrics, takeOpenProjectRequest, updatePracticeState, verifyProjectAccess, verifyProjectDestinationAccess } from "./lib/backend";
  import { languageDirection, languageOptions, systemLanguage, translate, type Language, type MessageKey } from "./lib/i18n";
  import { deduplicateImportCandidates, importRelevanceLevel, importRelevancePercent, normalizeImportQuery, reconcileImportSelection } from "./lib/importCandidates";
  import type { ImportCandidateGroup } from "./lib/importCandidates";
  import { shouldConfirmDialogOnEnter } from "./lib/dialogKeyboard";
  import { droppedAudioPaths } from "./lib/importPaths";
  import { ImportSearchCache } from "./lib/importSearchCache";
  import { completedImportBatch } from "./lib/importCompletion";
  import { BackgroundTaskScheduler } from "./lib/backgroundTaskScheduler";
  import { filterLogs, logOrigins, type LogLevel } from "./lib/logFilters";
  import { metronomeShortcutAction, parameterShortcutAction, parameterShortcutForKey, shortcutKeyLabels, shortcutPlatformFor, shouldBlurFocusedSelect, shouldHandleGlobalShortcut, shouldHandleParameterShortcut, shouldHandlePlayPauseShortcut, shouldToggleChordEditModeShortcut, shouldToggleMetronomeOnRelease, type ParameterShortcut, type ParameterShortcutAction } from "./lib/globalShortcuts";
  import { activeChordIndexAt, adjacentChordGridIndex, chordColor, chordDisplayLabel, chordRepertoire, chordTimeline, chordViewportBlocks, chordsForMode, isNoChordLabel, presentChordLabel, presentChordSequence, visibleChords, type ChordAccidentalMode, type ChordColorMode } from "./lib/chordViews";
  import { applyChordEdits, centeredChordOptionScrollTop, chordEditKey, chordEditKeyboardAction, chordEditOptions, chordEditPointerAction, chordGridKeyboardAction, chordSuggestions, shouldSeekChordFromClick, updateChordEdits, validateChordEntry } from "./lib/chordEditing";
  import Icon from "./lib/Icon.svelte";
  import FretboardChord from "./lib/FretboardChord.svelte";
  import PianoChord from "./lib/PianoChord.svelte";
  import NumericControl from "./lib/NumericControl.svelte";
  import Modal from "./lib/Modal.svelte";
  import Toaster from "./lib/Toaster.svelte";
  import { appendToast, type ToastLevel, type ToastMessage } from "./lib/toasts";
  import { buildProjectPath, calculateDetectedBeatLines, defaultLoopBounds, formatPitch, formatProjectHeaderPath, formatTime, formatTimePrecise, isDetectedBeatActive, moveWaveformViewport, panWaveformViewportFromWheel, resizeWaveformViewport, shouldApplyAudioStatus, shouldApplyAudioStatusPosition, trackLoadPosition, visiblePeaks, waveformShowsChords, waveformShowsDetail, waveformViewportForWindow, waveformWheelAxis, zoomWaveformViewport, type WaveformViewport, type WaveformViewportEdge, type WaveformWheelAxis } from "./lib/presentation";
  import { effectiveNavigationMode, navigationPosition, snappedNavigationPosition } from "./lib/navigation";
  import { forgetTrackSelection, preferredTrack, rememberedTrackId, rememberTrackSelection } from "./lib/projectSelection";
  import { shouldResumeStemPlayback, stemPlaybackResumeRequest, type StemPlaybackResumeRequest } from "./lib/stemPlayback";
  import { chordSegmentsForJams } from "./lib/chordExport";
  import { trackTitleBounceMetrics } from "./lib/trackTitleMotion";
  import type { AppLogEntry, ChordAnalysis, ChordEdit, ChordMode, DiagnosticsSnapshot, EndBehavior, ImportCandidate, ImportJob, ImportJobState, MetronomeSound, NavigationMode, ProjectSummary, StemMix, StemStatus, SystemMetrics, TimedChord, TrackSummary, UserPreferences, WaveformData } from "./lib/types";

  let project: ProjectSummary | null = null;
  let diagnosticInfo: DiagnosticsSnapshot | null = null;
  let runtimeOs = "macos";
  let toasts: ToastMessage[] = [];
  let nextToastId = 1;
  let busy = false;
  let currentTrack: TrackSummary | null = null;
  let isPlaying = false;
  let playRequestActive: Promise<void> | null = null;
  let currentSeconds = 0;
  let durationSeconds = 0;
  let playbackRate = 1;
  let pitchSemitones = 0;
  let volume = 1;
  let musicVolume = 1;
  let volumeBeforeMute = 1;
  let masterPeak = 0;
  let masterPeakLeft = 0;
  let masterPeakRight = 0;
  let limiterReduction = 0;
  let normalizationGain = 1;
  let integratedLufs: number | null = null;
  let loopEnabled = false;
  let loopA: number | null = null;
  let loopB: number | null = null;
  let usingDefaultLoopBounds = false;
  let loopCommandGeneration = 0;
  let preferencesVisible = false;
  let importVisible = false;
  let tasksVisible = false;
  let consoleVisible = false;
  let helpVisible = true;
  let appLogs: AppLogEntry[] = [];
  let consoleMinimumLevel: LogLevel = "debug";
  let consoleOrigin: string | null = null;
  let shortcutsVisible = false;
  let closePromptVisible = false;
  let waveform: WaveformData | null = null;
  let waveformLoading = false;
  let audioLoading = false;
  let loadingTrackId: string | null = null;
  let tempoLoading = false;
  let detectedBpm: number | null = null;
  let chordAnalysis: ChordAnalysis | null = null;
  let chordsLoading = false;
  let chordAnalysisError = "";
  let chordAutoScrollEnabled = true;
  let chordScrollSuspended = false;
  let chordPointerInside = false;
  let chordFocusWithin = false;
  let chordFocusRestorePending = false;
  let chordProgrammaticScroll = false;
  let chordMode: ChordMode = "standard";
  let chordMinimumStrength = 0;
  let chordColorMode: ChordColorMode = "root";
  let chordAccidentalMode: ChordAccidentalMode = "flat";
  let chordView: "timeline" | "repertoire" = "timeline";
  let chordSettingsVisible = false;
  let chordEditMode = false;
  let chordEdits: ChordEdit[] = [];
  let selectedChordKey: string | null = null;
  let editingChordKey: string | null = null;
  let chordEditValue = "";
  let chordEditInvalid = false;
  let chordEditSuggestionOptions: string[] = [];
  let chordEditSuggestionIndex = -1;
  let chordEditContainer: HTMLElement | undefined;
  let chordEditOverlay: HTMLElement | undefined;
  let chordEditSuggestionSelect: HTMLSelectElement | undefined;
  let chordEditWheelAccumulator = 0;
  let harmonyView: "piano" | "guitar" | "ukulele" = "piano";
  let harmonyLabelMode: "notes" | "degrees" = "notes";
  let repertoireKeyboardLabel: string | null = null;
  let lastRepertoirePlaybackLabel: string | null = null;
  let chordList: HTMLElement | undefined;
  let lastFollowedChordIndex = -1;
  let chordFocusLayoutFrame: number | undefined;
  let metronomeEnabled = false;
  let loopSnapEnabled = true;
  let metronomeVolume = 0.55;
  let metronomeSound: MetronomeSound = "electronic";
  let metronomeBeating = false;
  let trainerEnabled = false;
  let trainerStartRate = 0.5;
  let trainerRepetitions = 1;
  let trainerIncrement = 0.05;
  let trainerTargetRate = 1;
  let trainerLoopCount = 0;
  let trainingSettingsVisible = false;
  let trainingDraft = { startRate: 0.5, targetRate: 1, increment: 0.05, repetitions: 1 };
  let spectrumBands = Array<number>(64).fill(0);
  let spectrumRequestActive = false;
  const canonicalStemNames = ["vocals", "drums", "bass", "other", "guitar", "piano"] as const;
  const stemDisplayOrder = [0, 1, 2, 4, 5, 3] as const;
  const stemColors = ["#36c7ef", "#f05d5e", "#ffc857", "#9b7ede", "#53d18d", "#f08cc0"] as const;
  const stemMeterLevels = Array.from({ length: 14 }, (_, index) => index + 1);
  let stems: StemStatus = { state: "disabled", enabled: false, progress: 0, stage: "disabled", trackId: null, cached: false, error: null, computeBackend: null };
  let stemMix: StemMix[] = Array.from({ length: 6 }, () => ({ gain: 1, pan: 0, muted: false, soloed: false }));
  let stemNames: string[] = [...canonicalStemNames];
  let stemPeaks = Array<number>(6).fill(0);
  let stemStatusRequestActive = false;
  let stemPlaybackLocked = false;
  let stemPlaybackResume: StemPlaybackResumeRequest | null = null;
  let stemPlaybackLockGeneration = 0;
  let stemGenerationStarting = false;
  let stemExportVisible = false;
  let stemExportFormat: "wav" | "mp3" = "wav";
  const defaultUserPreferences: UserPreferences = { theme: "system", language: "en", timeDisplay: "simple", toastDurationSeconds: 3, concurrentDownloads: 3, youtubeAutoSelectBestMatch: true, conversionFormat: "mp3", sampleRate: "preserve", channels: "stereo", mp3Quality: "vbrHigh", masterVolume: 1, musicVolume: 1, loudnessNormalization: true, metronomeVolume: 0.55, metronomeSound: "electronic", defaultPlaybackRate: 1, defaultPitchSemitones: 0, loopLoadPosition: "beginning", loopSnapEnabled: true, navigationMode: "time", navigationTimeSeconds: 10, defaultTrainerStartRate: 0.5, defaultTrainerRepetitions: 1, defaultTrainerIncrement: 0.05, defaultTrainerTargetRate: 1 };
  let preferences: UserPreferences = { ...defaultUserPreferences };
  let importText = "";
  let importCandidates: ImportCandidate[] = [];
  let importCandidateGroups: ImportCandidateGroup[] = [];
  let selectedImports = new Set<string>();
  let importAnalyzing = false;
  let importAnalysisError = "";
  let importHasAnalyzed = false;
  let importDropActive = false;
  let playlistDropActive = false;
  let playlistPanel: HTMLElement | undefined;
  let importTextarea: HTMLTextAreaElement | undefined;
  let importAnalysisTimer: number | undefined;
  let importAnalysisGeneration = 0;
  const importSearchCache = new ImportSearchCache(resolveYoutubeSearch);
  let importSearchCompleted = 0;
  let importSearchTotal = 0;
  let importActiveGroupIds = new Set<string>();
  let importPendingGroupIds = new Set<string>();
  let importGroupErrors = new Map<string, string>();
  let importQueue: ImportJob[] = [];
  type ImportBatch = { jobIds: Set<string>; states: Map<string, ImportJobState> };
  let importBatches: ImportBatch[] = [];
  const importDismissTimers = new Map<string, number>();
  const masterMeterLevels = [8, 7, 6, 5, 4, 3, 2, 1] as const;
  const defaultMasterVolume = 1;
  const defaultMusicVolume = 1;
  const defaultMetronomeVolume = 0.55;
  let editingTrackId: string | null = null;
  let editingTrackTitle = "";
  let editingTrackLocation: "header" | "playlist" | null = null;
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
  let followPlayhead = false;
  let waveformPointerInside = false;
  let waveformFocusWithin = false;
  let waveformFollowNeedsSmooth = false;
  let waveformFollowAnimationFrame: number | undefined;
  let dragStartX = 0;
  let dragStartViewport = 0;
  let dragStartZoom = 1;
  let dragMoved = false;
  let waveformDragPointerId: number | null = null;
  let practiceSaveTimer: number | undefined;
  let preferencesSaveActive = false;
  let preferencesSavePending = false;
  let playbackRateTimer: number | undefined;
  let pitchTimer: number | undefined;
  let volumePreferenceTimer: number | undefined;
  let jumpHoldDelayTimer: number | undefined;
  let jumpHoldRepeatTimer: number | undefined;
  let seekAnimationFrame: number | undefined;
  let pendingSeekPosition: number | null = null;
  let seekRequestActive = false;
  let seekGeneration = 0;
  let activeWaveformWheelAxis: WaveformWheelAxis | null = null;
  let waveformWheelAxisTimer: number | undefined;
  let statusRequestActive = false;
  let systemMetricsSnapshot: SystemMetrics = { cpuPercent: null, memoryMegabytes: null };
  let trackSelectionGeneration = 0;
  const waveformCache = new Map<string, WaveformData>();
  const loadingWave = Array.from({ length: 72 }, (_, index) => Math.min(0.95, 0.12 + Math.abs(Math.sin(index * 0.71) * Math.cos(index * 0.17)) * 0.78));
  const warmedProjects = new Set<string>();
  const backgroundTaskScheduler = new BackgroundTaskScheduler();
  type LoopDragMode = "a" | "b";
  let loopDrag: { mode: LoopDragMode; pointerId: number; a: number; b: number } | null = null;
  type ViewportDragMode = "move" | WaveformViewportEdge;
  let viewportDrag: { mode: ViewportDragMode; pointerId: number; originRatio: number; originClientX: number; start: number; zoom: number; moved: boolean } | null = null;
  let language: Language = systemLanguage();
  let t: (key: MessageKey) => string;
  $: t = (key: MessageKey): string => translate(language, key);
  const displayTime = (value: number): string => preferences.timeDisplay === "precise" ? formatTimePrecise(value) : formatTime(value);
  const shortcutPlatform = shortcutPlatformFor(navigator.platform, navigator.userAgent);
  const shortcutKeys = shortcutKeyLabels(shortcutPlatform);
  let activeNavigationMode: NavigationMode;
  let navigationAnalysisPending: boolean;
  let loopSnapAvailable: boolean;
  $: activeNavigationMode = effectiveNavigationMode(preferences.navigationMode, chordAnalysis?.beats ?? [], timelineChords);
  $: navigationAnalysisPending = activeNavigationMode !== preferences.navigationMode;
  $: loopSnapAvailable = preferences.navigationMode === "chord"
    ? Boolean(timelineChords.length || chordAnalysis?.beats.length)
    : Boolean(chordAnalysis?.beats.length);

  function errorText(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function notify(level: ToastLevel, title: string, detail?: string): void {
    toasts = appendToast(toasts, { id: nextToastId++, level, title, detail });
  }

  function dismissToast(id: number): void {
    toasts = toasts.filter((toast) => toast.id !== id);
  }

  function applyParameterShortcut(parameter: ParameterShortcut, action: ParameterShortcutAction, key: string): void {
    const direction = action === "increment" ? 1 : -1;
    if (parameter === "tempo") {
      if (action === "reset") resetPlaybackRate();
      else changePlaybackRate(direction * 0.05);
    } else if (parameter === "pitch") {
      if (action === "reset") resetPitch();
      else changePitch(direction);
    } else if (parameter === "zoom") {
      if (action === "reset") fitThirtySecondWaveform();
      else setWaveformZoom(waveformZoom * (direction > 0 ? 1.5 : 1 / 1.5));
    } else {
      const metronomeAction = metronomeShortcutAction(key);
      if (metronomeAction === "nextSound" || metronomeAction === "previousSound") {
        const sounds: readonly MetronomeSound[] = ["electronic", "woodblock", "metallic"];
        const currentIndex = Math.max(0, sounds.indexOf(metronomeSound));
        const offset = metronomeAction === "nextSound" ? 1 : -1;
        changeMetronomeSound(sounds[(currentIndex + offset + sounds.length) % sounds.length]!);
      } else if (metronomeAction === "resetVolume") {
        changeMetronomeVolume(defaultMetronomeVolume);
      } else if (metronomeAction === "incrementVolume" || metronomeAction === "decrementVolume") {
        changeMetronomeVolume(metronomeVolume + (metronomeAction === "incrementVolume" ? 0.05 : -0.05));
      }
    }
  }

  function importSummary(completed: number, failed: number): string {
    const imported = `${completed} ${t(completed === 1 ? "tracksImportedSingular" : "tracksImportedPlural")}`;
    if (failed === 0) return imported;
    return `${imported}, ${failed} ${t(failed === 1 ? "importFailureSingular" : "importFailurePlural")}`;
  }

  $: projectHeaderPath = project ? formatProjectHeaderPath(project.packagePath) : null;

  function focusOnMount(node: HTMLInputElement): void {
    queueMicrotask(() => {
      node.focus();
      node.select();
    });
  }

  function disableTextareaAutocorrect(node: HTMLTextAreaElement): void {
    // WebKit supports this attribute on textareas even though it is absent from
    // Svelte's current textarea typings.
    node.setAttribute("autocorrect", "off");
  }

  function bounceTrackTitle(node: HTMLButtonElement): { destroy: () => void } {
    const title = node.querySelector<HTMLElement>("span");
    const measure = (): void => {
      if (!title) return;
      const style = getComputedStyle(node);
      const availableWidth = node.clientWidth
        - Number.parseFloat(style.paddingLeft)
        - Number.parseFloat(style.paddingRight);
      const metrics = trackTitleBounceMetrics(availableWidth, title.scrollWidth);
      node.classList.toggle("has-overflow", Boolean(metrics));
      if (!metrics) {
        node.style.removeProperty("--track-title-shift");
        node.style.removeProperty("--track-title-duration");
        return;
      }
      node.style.setProperty("--track-title-shift", `${-metrics.overflowPixels}px`);
      node.style.setProperty("--track-title-duration", `${metrics.durationSeconds}s`);
    };
    const observer = new ResizeObserver(measure);
    const contentObserver = new MutationObserver(measure);
    observer.observe(node);
    if (title) {
      observer.observe(title);
      contentObserver.observe(title, { childList: true, characterData: true, subtree: true });
    }
    queueMicrotask(measure);
    return { destroy: () => { observer.disconnect(); contentObserver.disconnect(); } };
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
  $: waveformFollowSuspended = waveformPointerInside
    || waveformFocusWithin
    || waveformDragPointerId !== null
    || viewportDrag !== null;
  $: if (waveformFollowSuspended) {
    waveformFollowNeedsSmooth = true;
    cancelWaveformFollowAnimation();
  }
  $: if (followPlayhead && !waveformFollowSuspended && waveformZoom > 1 && durationSeconds > 0) {
    const centeredStart = moveWaveformViewport(
      currentSeconds / durationSeconds - 0.5 / waveformZoom,
      waveformZoom,
      0,
    ).start;
    if (waveformFollowNeedsSmooth) {
      waveformFollowNeedsSmooth = false;
      smoothWaveformFollow();
    } else if (waveformFollowAnimationFrame === undefined) {
      waveformStart = centeredStart;
    }
  }
  $: detailedBeatLines = calculateDetectedBeatLines(
    chordAnalysis?.beats ?? [],
    chordAnalysis?.downbeats ?? [],
    durationSeconds,
    true,
    waveformZoom,
    waveformStart,
  );
  $: metronomeBeating = metronomeEnabled && isPlaying && isDetectedBeatActive(currentSeconds, chordAnalysis?.beats ?? [], playbackRate);
  $: activeImports = importQueue.filter((job) => !["completed", "failed"].includes(job.state));
  $: backgroundTaskScheduler.setBlocked(
    busy
      || audioLoading
      || waveformLoading
      || tempoLoading
      || chordsLoading
      || stemGenerationStarting
      || stems.state === "separating"
      || importAnalyzing
      || importActiveGroupIds.size > 0
      || importPendingGroupIds.size > 0
      || activeImports.length > 0,
  );
  $: importProgress = importQueue.length ? importQueue.reduce((sum, job) => sum + job.progress, 0) / importQueue.length : 0;
  $: consoleOrigins = logOrigins(appLogs);
  $: filteredAppLogs = filterLogs(appLogs, consoleMinimumLevel, consoleOrigin);
  $: decodedChords = chordsForMode(chordAnalysis, chordMode);
  $: effectiveChords = applyChordEdits(decodedChords, chordEdits, chordMode);
  $: displayedChords = visibleChords(presentChordSequence(effectiveChords, pitchSemitones, chordAccidentalMode), chordMinimumStrength);
  $: timelineChords = chordTimeline(displayedChords);
  $: waveformChordBlocks = waveformShowsChords(durationSeconds, waveformZoom)
    ? chordViewportBlocks(timelineChords, durationSeconds, waveformZoom, waveformStart)
    : [];
  $: repertoireLabels = chordRepertoire(displayedChords);
  $: activeChordIndex = activeChordIndexAt(timelineChords, currentSeconds);
  $: activeChord = activeChordIndex >= 0 ? timelineChords[activeChordIndex] ?? null : null;
  $: activeChordLabel = repertoireKeyboardLabel ?? activeChord?.label ?? "N";
  $: activeHarmonyLabel = repertoireKeyboardLabel ?? (Math.round(pitchSemitones) === 0 ? activeChord?.sourceLabel ?? activeChordLabel : activeChordLabel);
  $: activeInstrumentColor = chordColor(activeChordLabel, activeChord?.strength ?? 1, chordColorMode);
  $: if (chordView === "repertoire" && (activeChord?.label ?? null) !== lastRepertoirePlaybackLabel) {
    lastRepertoirePlaybackLabel = activeChord?.label ?? null;
    repertoireKeyboardLabel = null;
  }
  $: if (chordView === "timeline" && chordAutoScrollEnabled && !editingChordKey && !chordScrollSuspended && activeChordIndex >= 0 && activeChordIndex !== lastFollowedChordIndex) {
    lastFollowedChordIndex = activeChordIndex;
    followChord(activeChordIndex);
  }

  function followChord(index: number): void {
    queueMicrotask(() => {
      const item = chordList?.querySelector<HTMLElement>(`[data-chord-index="${index}"]`);
      if (!chordList || !item) return;
      const top = chordList.scrollTop
        + item.getBoundingClientRect().top
        - chordList.getBoundingClientRect().top
        - (chordList.clientHeight - item.clientHeight) / 2;
      const targetTop = Math.max(0, top);
      if (Math.abs(chordList.scrollTop - targetTop) < 1) return;
      const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      chordProgrammaticScroll = true;
      chordList.scrollTo({
        top: targetTop,
        behavior: prefersReducedMotion ? "auto" : "smooth",
      });
      if (prefersReducedMotion) queueMicrotask(() => chordProgrammaticScroll = false);
    });
  }

  function suspendChordFollow(): void {
    if (!chordPointerInside) return;
    chordProgrammaticScroll = false;
    chordScrollSuspended = true;
  }

  function handleChordScroll(): void {
    if (chordPointerInside && !chordProgrammaticScroll) chordScrollSuspended = true;
  }

  function resumeChordFollow(): void {
    chordPointerInside = false;
    chordProgrammaticScroll = false;
    if (!chordAutoScrollEnabled || editingChordKey || chordFocusWithin) {
      chordScrollSuspended = true;
      return;
    }
    if (!chordScrollSuspended) return;
    chordScrollSuspended = false;
    if (activeChordIndex < 0) return;
    lastFollowedChordIndex = activeChordIndex;
    followChord(activeChordIndex);
  }

  function toggleChordAutoScroll(): void {
    chordAutoScrollEnabled = !chordAutoScrollEnabled;
    chordProgrammaticScroll = false;
    if (!chordAutoScrollEnabled || editingChordKey || chordFocusWithin || chordPointerInside) {
      chordScrollSuspended = true;
      return;
    }
    chordScrollSuspended = false;
    lastFollowedChordIndex = -1;
  }

  function selectChord(chord: TimedChord): void {
    selectedChordKey = chordEditKey(chordMode, chord);
    repertoireKeyboardLabel = null;
  }

  function ensureChordFocusSpace(target: HTMLElement, requiredBelow = 142): void {
    if (chordFocusLayoutFrame !== undefined) cancelAnimationFrame(chordFocusLayoutFrame);
    chordFocusLayoutFrame = requestAnimationFrame(() => {
      chordFocusLayoutFrame = undefined;
      if (!chordList || !chordList.contains(target)) return;
      const viewport = chordList.getBoundingClientRect();
      const bounds = target.getBoundingClientRect();
      const padding = 4;
      const availableBelow = Math.max(0, viewport.height - bounds.height - padding * 2);
      const reservedBelow = Math.min(requiredBelow, availableBelow);
      const topOverflow = bounds.top - (viewport.top + padding);
      const bottomOverflow = bounds.bottom + reservedBelow - (viewport.bottom - padding);
      const scrollDelta = topOverflow < 0 ? topOverflow : bottomOverflow > 0 ? bottomOverflow : 0;
      if (Math.abs(scrollDelta) < 1) return;
      const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      chordProgrammaticScroll = true;
      chordList.scrollTo({
        top: chordList.scrollTop + scrollDelta,
        behavior: prefersReducedMotion ? "auto" : "smooth",
      });
      if (prefersReducedMotion) queueMicrotask(() => chordProgrammaticScroll = false);
    });
  }

  function focusChordFromButton(event: FocusEvent, chord: TimedChord): void {
    selectChord(chord);
    ensureChordFocusSpace(event.currentTarget as HTMLButtonElement);
  }

  function selectChordFromButton(event: MouseEvent, chord: TimedChord): void {
    (event.currentTarget as HTMLButtonElement).focus({ preventScroll: true });
    selectChord(chord);
    changeNavigationMode("chord");
    if (shouldSeekChordFromClick(chordEditMode, event.altKey)) seek(chord.startSeconds);
  }

  function prepareChordMiddleClick(event: PointerEvent): void {
    if (!chordEditMode || event.button !== 1) return;
    event.preventDefault();
    event.stopPropagation();
    (event.currentTarget as HTMLButtonElement).focus({ preventScroll: true });
  }

  function beginChordEditFromMiddleClick(event: MouseEvent, chord: TimedChord): void {
    if (!chordEditMode || event.button !== 1) return;
    event.preventDefault();
    event.stopPropagation();
    (event.currentTarget as HTMLButtonElement).focus({ preventScroll: true });
    beginChordEdit(chord);
  }

  function beginChordEdit(chord: TimedChord): void {
    if (!chordEditMode) return;
    selectChord(chord);
    chordView = "timeline";
    editingChordKey = chordEditKey(chordMode, chord);
    chordEditValue = chord.label;
    chordEditInvalid = false;
    const availableOptions = chordEditOptions(chordAccidentalMode);
    chordEditSuggestionOptions = availableOptions.includes(chord.label)
      ? availableOptions
      : [chord.label, ...availableOptions];
    chordEditSuggestionIndex = chordEditSuggestionOptions.findIndex((suggestion) => suggestion === chord.label);
    chordEditWheelAccumulator = 0;
    chordScrollSuspended = true;
  }

  function restoreSelectedChordFocus(key: string): void {
    chordFocusRestorePending = true;
    queueMicrotask(() => {
      if (!chordList || selectedChordKey !== key) {
        chordFocusRestorePending = false;
        return;
      }
      const index = timelineChords.findIndex((chord) => chordEditKey(chordMode, chord) === key);
      const target = chordList.querySelector<HTMLButtonElement>(`button[data-chord-index="${index}"]`);
      chordFocusRestorePending = false;
      target?.focus({ preventScroll: true });
    });
  }

  function cancelChordEdit(restoreFocus = false): void {
    const editedKey = editingChordKey;
    editingChordKey = null;
    chordEditInvalid = false;
    chordEditSuggestionOptions = [];
    chordEditSuggestionIndex = -1;
    chordEditWheelAccumulator = 0;
    if (restoreFocus && editedKey) restoreSelectedChordFocus(editedKey);
  }

  function commitChordEdit(replaceAllSimilar: boolean, cancelInvalid = false, restoreFocus = false): void {
    if (!editingChordKey) return;
    const editedKey = editingChordKey;
    const displayLabel = validateChordEntry(chordEditValue, chordAccidentalMode);
    if (!displayLabel) {
      if (cancelInvalid) cancelChordEdit(restoreFocus);
      else chordEditInvalid = true;
      return;
    }
    const baseLabel = presentChordLabel(displayLabel, -Math.round(pitchSemitones), "sharp");
    chordEdits = updateChordEdits(decodedChords, chordEdits, chordMode, editingChordKey, baseLabel, replaceAllSimilar);
    editingChordKey = null;
    chordEditInvalid = false;
    chordEditSuggestionOptions = [];
    chordEditSuggestionIndex = -1;
    chordEditWheelAccumulator = 0;
    schedulePracticeSave(0);
    if (restoreFocus) restoreSelectedChordFocus(editedKey);
  }

  function resetChordEdits(): void {
    if (!chordEdits.length) return;
    chordEdits = [];
    cancelChordEdit();
    schedulePracticeSave(0);
  }

  function setChordEditMode(enabled: boolean): void {
    if (chordEditMode === enabled) return;
    chordEditMode = enabled;
    if (enabled) {
      chordView = "timeline";
      repertoireKeyboardLabel = null;
      return;
    }
    cancelChordEdit(true);
  }

  function toggleChordEditMode(): void {
    if (!timelineChords.length) return;
    setChordEditMode(!chordEditMode);
  }

  function toggleChordRepertoire(): void {
    chordView = chordView === "timeline" ? "repertoire" : "timeline";
    if (chordView === "repertoire") setChordEditMode(false);
    repertoireKeyboardLabel = null;
    chordScrollSuspended = false;
    chordPointerInside = false;
    chordFocusWithin = false;
    chordProgrammaticScroll = false;
    lastFollowedChordIndex = -1;
  }

  function refreshChordEditSuggestions(): void {
    chordEditSuggestionOptions = chordSuggestions(chordEditValue);
    chordEditSuggestionIndex = -1;
    chordEditInvalid = false;
  }

  function stepChordEditSuggestion(direction: -1 | 1): void {
    if (!chordEditSuggestionOptions.length) refreshChordEditSuggestions();
    if (!chordEditSuggestionOptions.length) return;
    const currentIndex = chordEditSuggestionOptions.findIndex((suggestion) => suggestion === chordEditValue);
    const anchor = currentIndex >= 0 ? currentIndex : chordEditSuggestionIndex;
    chordEditSuggestionIndex = Math.max(0, Math.min(
      chordEditSuggestionOptions.length - 1,
      anchor < 0 ? (direction > 0 ? 0 : chordEditSuggestionOptions.length - 1) : anchor + direction,
    ));
    chordEditValue = chordEditSuggestionOptions[chordEditSuggestionIndex] ?? chordEditValue;
    chordEditInvalid = false;
    queueMicrotask(() => chordEditSuggestionSelect?.querySelector("option:checked")?.scrollIntoView({ block: "nearest" }));
  }

  function handleChordEditKeydown(event: KeyboardEvent): void {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      stepChordEditSuggestion(event.key === "ArrowDown" ? 1 : -1);
      return;
    }
    const action = chordEditKeyboardAction(event.key);
    if (action === "commit") {
      event.preventDefault();
      event.stopPropagation();
      commitChordEdit(event.shiftKey, false, true);
    } else if (action === "cancel") {
      event.preventDefault();
      event.stopPropagation();
      cancelChordEdit(true);
    }
  }

  function handleChordEditWheel(event: WheelEvent): void {
    event.preventDefault();
    event.stopPropagation();
    if (event.shiftKey) {
      commitChordEdit(true, false, true);
      return;
    }
    const delta = event.deltaY !== 0 ? event.deltaY : event.deltaX;
    chordEditWheelAccumulator += delta;
    if (Math.abs(chordEditWheelAccumulator) < 16) return;
    stepChordEditSuggestion(chordEditWheelAccumulator > 0 ? 1 : -1);
    chordEditWheelAccumulator = 0;
  }

  function handleChordEditAuxClick(event: MouseEvent): void {
    if (event.button !== 1) return;
    event.preventDefault();
    event.stopPropagation();
    commitChordEdit(event.shiftKey, false, true);
  }

  function chooseChordEditSuggestion(event: Event): void {
    const select = event.currentTarget as HTMLSelectElement;
    chordEditValue = select.value;
    chordEditSuggestionIndex = select.selectedIndex;
    chordEditInvalid = false;
  }

  function validateChordEditSuggestion(event: MouseEvent): void {
    const action = chordEditPointerAction("option", event.button, event.shiftKey);
    if (action !== "commit" && action !== "commitAll") return;
    event.stopPropagation();
    chooseChordEditSuggestion(event);
    commitChordEdit(action === "commitAll", false, true);
  }

  function chordEditOptionsPortal(node: HTMLSelectElement): { destroy: () => void } {
    const overlay = document.createElement("div");
    let revealFrame: number | undefined;
    overlay.className = "chord-edit-overlay";
    overlay.appendChild(node);
    document.body.appendChild(overlay);
    chordEditOverlay = overlay;
    revealFrame = requestAnimationFrame(() => {
      revealFrame = undefined;
      if (chordEditSuggestionIndex < 0) return;
      node.selectedIndex = chordEditSuggestionIndex;
      node.scrollTop = centeredChordOptionScrollTop(
        chordEditSuggestionIndex,
        node.options.length,
        node.clientHeight,
        node.scrollHeight,
      );
    });
    const navigateWithWheel = (event: WheelEvent): void => handleChordEditWheel(event);
    const preventMiddleDefault = (event: PointerEvent): void => {
      if (event.button !== 1) return;
      event.preventDefault();
      event.stopPropagation();
    };
    const finishWithMiddleClick = (event: MouseEvent): void => handleChordEditAuxClick(event);
    node.addEventListener("wheel", navigateWithWheel, { passive: false });
    node.addEventListener("pointerdown", preventMiddleDefault);
    node.addEventListener("auxclick", finishWithMiddleClick);
    return {
      destroy: () => {
        if (revealFrame !== undefined) cancelAnimationFrame(revealFrame);
        node.removeEventListener("wheel", navigateWithWheel);
        node.removeEventListener("pointerdown", preventMiddleDefault);
        node.removeEventListener("auxclick", finishWithMiddleClick);
        if (chordEditOverlay === overlay) chordEditOverlay = undefined;
        overlay.remove();
      },
    };
  }

  function chordEditInteractions(node: HTMLElement): { destroy: () => void } {
    let positionFrame: number | undefined;
    let editingSpaceReserved = false;
    const scrollContainer = node.closest<HTMLElement>(".chords");
    const positionOptions = (): void => {
      if (positionFrame !== undefined) return;
      positionFrame = requestAnimationFrame(() => {
        positionFrame = undefined;
        const select = chordEditSuggestionSelect;
        const overlay = chordEditOverlay;
        if (!select || !overlay || !scrollContainer) return;
        if (!editingSpaceReserved) {
          editingSpaceReserved = true;
          ensureChordFocusSpace(node, select.offsetHeight + 10);
        }
        const bounds = node.getBoundingClientRect();
        const viewport = scrollContainer.getBoundingClientRect();
        const menuLeft = bounds.left;
        const menuTop = bounds.bottom + 6;
        const menuRight = menuLeft + bounds.width;
        const menuBottom = menuTop + select.offsetHeight;
        const visibleLeft = Math.max(viewport.left, menuLeft);
        const visibleTop = Math.max(viewport.top, menuTop);
        const visibleRight = Math.min(viewport.right, menuRight);
        const visibleBottom = Math.min(viewport.bottom, menuBottom);
        if (visibleRight <= visibleLeft || visibleBottom <= visibleTop) {
          overlay.style.display = "none";
          return;
        }
        overlay.style.display = "block";
        overlay.style.left = `${visibleLeft}px`;
        overlay.style.top = `${visibleTop}px`;
        overlay.style.width = `${visibleRight - visibleLeft}px`;
        overlay.style.height = `${visibleBottom - visibleTop}px`;
        select.style.left = `${menuLeft - visibleLeft}px`;
        select.style.top = `${menuTop - visibleTop}px`;
        select.style.width = `${bounds.width}px`;
      });
    };
    const preventMiddleDefault = (event: PointerEvent): void => {
      if (event.button !== 1) return;
      event.preventDefault();
      event.stopPropagation();
    };
    const finishWithMiddleClick = (event: MouseEvent): void => handleChordEditAuxClick(event);
    const navigateWithWheel = (event: WheelEvent): void => handleChordEditWheel(event);
    const cancelWithOutsideClick = (event: PointerEvent): void => {
      const target = event.target;
      if (!(target instanceof Node)
        || node.contains(target)
        || chordEditSuggestionSelect?.contains(target)
        || chordEditOverlay?.contains(target)) return;
      if (chordEditPointerAction("outside", event.button, event.shiftKey) === "cancel") cancelChordEdit();
    };
    node.addEventListener("wheel", navigateWithWheel, { passive: false });
    node.addEventListener("pointerdown", preventMiddleDefault);
    node.addEventListener("auxclick", finishWithMiddleClick);
    document.addEventListener("pointerdown", cancelWithOutsideClick, true);
    window.addEventListener("resize", positionOptions);
    scrollContainer?.addEventListener("scroll", positionOptions);
    positionOptions();
    return {
      destroy: () => {
        if (positionFrame !== undefined) cancelAnimationFrame(positionFrame);
        node.removeEventListener("wheel", navigateWithWheel);
        node.removeEventListener("pointerdown", preventMiddleDefault);
        node.removeEventListener("auxclick", finishWithMiddleClick);
        document.removeEventListener("pointerdown", cancelWithOutsideClick, true);
        window.removeEventListener("resize", positionOptions);
        scrollContainer?.removeEventListener("scroll", positionOptions);
      },
    };
  }

  function handleChordKeydown(event: KeyboardEvent, chord: TimedChord): void {
    const action = chordGridKeyboardAction(chordEditMode, event.key);
    if (!action) return;
    if (action === "beginEdit") {
      event.preventDefault();
      event.stopPropagation();
      beginChordEdit(chord);
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (action === "up" || action === "down") {
      moveChordGridSelection(action === "up" ? -1 : 1);
    } else {
      moveChordTimelineSelection(action === "previous" ? -1 : 1, !chordEditMode);
    }
  }

  function changeNavigationMode(mode: NavigationMode): void {
    if (preferences.navigationMode === mode) return;
    preferences = { ...preferences, navigationMode: mode };
    void persistPreferences();
  }

  function cycleNavigationMode(): void {
    const modes: NavigationMode[] = ["time", "beat", "chord"];
    const index = modes.indexOf(preferences.navigationMode);
    changeNavigationMode(modes[(index + 1) % modes.length] ?? "time");
  }

  function navigationModeLabel(mode: NavigationMode): string {
    if (mode === "beat") return t("navigationBeat");
    if (mode === "chord") return t("navigationChord");
    return t("navigationTime");
  }

  function navigationDirectionLabel(direction: -1 | 1): string {
    if (activeNavigationMode === "beat") return direction < 0 ? t("previousBeat") : t("nextBeat");
    if (activeNavigationMode === "chord") return direction < 0 ? t("previousChord") : t("nextChord");
    const template = direction < 0 ? t("navigationBackTime") : t("navigationForwardTime");
    return template.replace("{seconds}", String(preferences.navigationTimeSeconds));
  }

  function cycleHarmonyView(): void {
    const views: Array<typeof harmonyView> = ["piano", "guitar", "ukulele"];
    const index = views.indexOf(harmonyView);
    harmonyView = views[(index + 1) % views.length] ?? "piano";
  }

  function selectedChordIndex(): number {
    if (!selectedChordKey) return -1;
    return timelineChords.findIndex((chord) => chordEditKey(chordMode, chord) === selectedChordKey);
  }

  function focusChordAtIndex(targetIndex: number, seekPlayback = false): void {
    if (chordView !== "timeline" || !chordList || !timelineChords.length) return;
    const buttons = [...chordList.querySelectorAll<HTMLButtonElement>("button[data-chord-index]")];
    const target = buttons.find((button) => Number(button.dataset.chordIndex) === targetIndex);
    const chord = timelineChords[targetIndex];
    if (!target || !chord) return;

    target.focus({ preventScroll: true });
    selectChord(chord);
    if (seekPlayback) seek(chord.startSeconds);
  }

  function moveChordTimelineSelection(direction: -1 | 1, seekPlayback = false): void {
    if (chordView !== "timeline" || !timelineChords.length) return;
    const selectedIndex = selectedChordIndex();
    const anchorIndex = selectedIndex >= 0 ? selectedIndex : activeChordIndex;
    const targetIndex = anchorIndex < 0
      ? (direction > 0 ? 0 : timelineChords.length - 1)
      : Math.max(0, Math.min(timelineChords.length - 1, anchorIndex + direction));
    focusChordAtIndex(targetIndex, seekPlayback);
  }

  function moveChordGridSelection(direction: -1 | 1): void {
    if (chordView !== "timeline" || !chordList || !timelineChords.length) return;
    const buttons = [...chordList.querySelectorAll<HTMLButtonElement>("button[data-chord-index]")];
    const selectedIndex = selectedChordIndex();
    const currentIndex = selectedIndex >= 0 ? selectedIndex : Math.max(0, activeChordIndex);
    const targetIndex = adjacentChordGridIndex(
      buttons.map((button) => {
        const bounds = button.getBoundingClientRect();
        return {
          index: Number(button.dataset.chordIndex),
          left: bounds.left,
          top: bounds.top,
          width: bounds.width,
          height: bounds.height,
        };
      }),
      currentIndex,
      direction,
    );
    focusChordAtIndex(targetIndex);
  }

  function leaveChordGridFocus(event: FocusEvent): void {
    if (chordList?.contains(event.relatedTarget as Node | null)) return;
    if (chordFocusRestorePending) return;
    chordFocusWithin = false;
    if (!chordPointerInside) resumeChordFollow();
  }

  function changeChordMode(mode: ChordMode): void {
    chordMode = mode;
    repertoireKeyboardLabel = null;
    selectedChordKey = null;
    cancelChordEdit();
    lastFollowedChordIndex = -1;
  }

  onMount(() => {
    let unlisten: UnlistenFn | undefined;
    let unlistenDrag: (() => void) | undefined;
    let unlistenClose: (() => void) | undefined;
    let unlistenExit: UnlistenFn | undefined;
    let unlistenProjectOpen: UnlistenFn | undefined;
    const appWindow = getCurrentWindow();
    void diagnostics().then((value) => runtimeOs = value.os).catch(() => undefined);
    let activeParameterShortcut: ParameterShortcut | null = null;
    let parameterShortcutActionUsed = false;
    void listen<string>("native-menu", (event) => handleNativeMenu(event.payload)).then((stop) => unlisten = stop);
    void listen<void>("application-exit-requested", () => closePromptVisible = true).then((stop) => unlistenExit = stop);
    void listen<void>("project-open-requested", () => void openRequestedProject()).then((stop) => {
      unlistenProjectOpen = stop;
      void openRequestedProject();
    });
    const handleKeydown = (event: KeyboardEvent): void => {
      if (shouldHandlePlayPauseShortcut(event)) {
        event.preventDefault();
        event.stopPropagation();
        if (project && !event.repeat) togglePlayback();
        return;
      }
      const parameter = parameterShortcutForKey(event.key);
      if (parameter && shouldHandleParameterShortcut(event)) {
        event.preventDefault();
        event.stopPropagation();
        if (!event.repeat) {
          activeParameterShortcut = parameter;
          parameterShortcutActionUsed = false;
        }
        return;
      }
      const parameterAction = parameterShortcutAction(event.key);
      if (activeParameterShortcut && parameterAction && shouldHandleParameterShortcut(event)) {
        event.preventDefault();
        event.stopPropagation();
        parameterShortcutActionUsed = true;
        applyParameterShortcut(activeParameterShortcut, parameterAction, event.key);
        return;
      }
      if (document.querySelector("dialog[open]")) return;
      if (shouldToggleChordEditModeShortcut(event)) {
        event.preventDefault();
        event.stopPropagation();
        if (!event.repeat) toggleChordEditMode();
        return;
      }
      if (!shouldHandleGlobalShortcut(event)) return;
      const target = event.target as HTMLElement | null;
      const key = event.key.toLowerCase();
      if (key === "c") {
        event.preventDefault();
        if (!event.repeat) toggleConsole();
      }
      else if (key === "h") {
        event.preventDefault();
        if (!event.repeat) toggleHelp();
      }
      else if (key === "n") {
        event.preventDefault();
        if (!event.repeat) cycleNavigationMode();
      }
      else if (key === "i") {
        event.preventDefault();
        if (!event.repeat) cycleHarmonyView();
      }
      else if (target?.closest("button, a[href]") || !project) return;
      else if (key === "a") { event.preventDefault(); setLoopA(); }
      else if (key === "b") { event.preventDefault(); setLoopB(); }
      else if (key === "l") { event.preventDefault(); toggleLoop(); }
      else if (event.key === "Escape") { event.preventDefault(); clearLoop(); }
      else if (event.key === "ArrowLeft") { event.preventDefault(); navigate(-1); }
      else if (event.key === "ArrowRight") { event.preventDefault(); navigate(1); }
    };
    const handleKeyup = (event: KeyboardEvent): void => {
      if (parameterShortcutForKey(event.key) !== activeParameterShortcut) return;
      if (shouldToggleMetronomeOnRelease(event, activeParameterShortcut, parameterShortcutActionUsed)) {
        event.preventDefault();
        event.stopPropagation();
        toggleMetronome();
      }
      activeParameterShortcut = null;
      parameterShortcutActionUsed = false;
    };
    const clearParameterShortcut = (): void => {
      activeParameterShortcut = null;
      parameterShortcutActionUsed = false;
    };
    window.addEventListener("keydown", handleKeydown, { capture: true });
    window.addEventListener("keyup", handleKeyup, { capture: true });
    window.addEventListener("blur", clearParameterShortcut);
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
    const consoleTimer = window.setInterval(() => { if (consoleVisible) void refreshConsole(); }, 350);
    void refreshSystemMetrics();
    const originalConsole = installConsoleForwarding();
    const handleWindowError = (event: ErrorEvent): void => console.error(event.error ?? event.message);
    const handleUnhandledRejection = (event: PromiseRejectionEvent): void => console.error("Unhandled promise rejection", event.reason);
    window.addEventListener("error", handleWindowError);
    window.addEventListener("unhandledrejection", handleUnhandledRejection);
    void loadUserPreferences().finally(() => void restoreLastProject());
    const finishPlaylistDrag = (): void => finishTrackDrag();
    const handleWindowPointerDown = (event: PointerEvent): void => {
      trackContextMenu = null;
      const activeElement = document.activeElement;
      const focusRegion = activeElement?.closest("label") ?? activeElement;
      const pointerTarget = event.target instanceof Node ? event.target : null;
      const pointerInsideFocusRegion = Boolean(pointerTarget && focusRegion?.contains(pointerTarget));
      if (shouldBlurFocusedSelect(activeElement, pointerInsideFocusRegion)) {
        (activeElement as HTMLSelectElement).blur();
      }
    };
    window.addEventListener("pointerup", finishPlaylistDrag);
    window.addEventListener("pointercancel", finishPlaylistDrag);
    window.addEventListener("pointerdown", handleWindowPointerDown);
    void appWindow.onCloseRequested((event) => {
      handleWindowCloseRequest(event, () => void requestApplicationExit());
    }).then((stop) => unlistenClose = stop);
    void appWindow.onDragDropEvent((event) => {
      const position = "position" in event.payload ? event.payload.position : undefined;
      if (event.payload.type === "enter" || event.payload.type === "over") {
        importDropActive = importVisible && isImportDropTarget(position);
        playlistDropActive = !importVisible && isPlaylistDropTarget(position);
      }
      else if (event.payload.type === "leave") {
        importDropActive = false;
        playlistDropActive = false;
      }
      else if (event.payload.type === "drop") {
        const importTarget = importVisible && isImportDropTarget(position);
        const playlistTarget = !importVisible && isPlaylistDropTarget(position);
        importDropActive = false;
        playlistDropActive = false;
        if (importTarget) void acceptDroppedPaths(event.payload.paths);
        else if (playlistTarget) void importDroppedAudio(event.payload.paths);
      }
    }).then((stop) => unlistenDrag = stop);
    const savedEndBehavior = localStorage.getItem("sonarcan.endBehavior");
    if (savedEndBehavior === "restart" || savedEndBehavior === "advance" || savedEndBehavior === "stop") endBehavior = savedEndBehavior;
    void audioSetEndBehavior(endBehavior);
    return () => {
      window.removeEventListener("keydown", handleKeydown, { capture: true });
      window.removeEventListener("keyup", handleKeyup, { capture: true });
      window.removeEventListener("blur", clearParameterShortcut);
      window.removeEventListener("pointerover", handleHelpOver);
      window.removeEventListener("pointerout", handleHelpOut);
      window.removeEventListener("focusin", handleHelpFocus);
      window.removeEventListener("focusout", handleHelpBlur);
      window.removeEventListener("pointerdown", handleWindowPointerDown);
      window.clearInterval(statusTimer);
      window.clearInterval(spectrumTimer);
      window.clearInterval(stemTimer);
      window.clearInterval(metricsTimer);
      window.clearInterval(importTimer);
      window.clearInterval(consoleTimer);
      window.clearTimeout(playbackRateTimer);
      window.clearTimeout(pitchTimer);
      window.clearTimeout(volumePreferenceTimer);
      window.clearTimeout(importAnalysisTimer);
      stopJumpHold();
      cancelPendingSeek();
      cancelWaveformFollowAnimation();
      backgroundTaskScheduler.cancelAll();
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
      unlistenDrag?.();
      unlistenClose?.();
      unlistenExit?.();
      unlistenProjectOpen?.();
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

  function toggleHelp(): void {
    helpVisible = !helpVisible;
  }

  function logOriginLabel(origin: string): string {
    if (origin === "rust") return "RUST";
    if (origin === "mlx") return "MLX";
    if (origin === "webview") return "WEB";
    return origin.toUpperCase();
  }

  async function loadUserPreferences(): Promise<void> {
    try {
      preferences = await getPreferences();
      language = preferences.language;
      volume = preferences.masterVolume;
      musicVolume = preferences.musicVolume;
      metronomeVolume = preferences.metronomeVolume;
      metronomeSound = preferences.metronomeSound;
      loopSnapEnabled = preferences.loopSnapEnabled;
      applyTheme();
      document.documentElement.lang = language;
      document.documentElement.dir = languageDirection(language);
      await setApplicationLanguage(language);
      await audioSetVolume(volume);
      await audioSetMusicVolume(musicVolume);
      await audioSetLoudnessNormalization(preferences.loudnessNormalization);
      await audioSetMetronome(false, metronomeVolume, metronomeSound);
    } catch {
      applyTheme();
      notify("warn", t("preferencesLoadFallback"));
    }
  }

  function applyTheme(): void { document.documentElement.dataset.theme = preferences.theme; }

  function applyPreferencesImmediately(): void {
    language = preferences.language;
    volume = preferences.masterVolume;
    musicVolume = preferences.musicVolume;
    metronomeVolume = preferences.metronomeVolume;
    metronomeSound = preferences.metronomeSound;
    loopSnapEnabled = preferences.loopSnapEnabled;
    applyTheme();
    document.documentElement.lang = language;
    document.documentElement.dir = languageDirection(language);
    void setApplicationLanguage(language);
    void audioSetVolume(volume);
    void audioSetMusicVolume(musicVolume);
    void audioSetLoudnessNormalization(preferences.loudnessNormalization);
    void audioSetMetronome(metronomeEnabled, metronomeVolume, metronomeSound);
  }

  function autosavePreferences(): void {
    applyPreferencesImmediately();
    void persistPreferences();
  }

  function resetUserPreferences(): void {
    preferences = { ...defaultUserPreferences };
    autosavePreferences();
  }

  function resetPreferenceVolume(key: "masterVolume" | "musicVolume" | "metronomeVolume"): void {
    preferences = { ...preferences, [key]: defaultUserPreferences[key] };
    autosavePreferences();
  }

  async function persistPreferences(): Promise<boolean> {
    if (preferencesSaveActive) {
      preferencesSavePending = true;
      return true;
    }
    preferencesSaveActive = true;
    try {
      do {
        preferencesSavePending = false;
        const saved = await savePreferences({ ...preferences });
        if (!preferencesSavePending) {
          preferences = saved;
          applyPreferencesImmediately();
        }
      } while (preferencesSavePending);
      return true;
    } catch (error) {
      notify("error", t("preferencesSaveError"), errorText(error));
      return false;
    } finally {
      preferencesSaveActive = false;
    }
  }

  function changeLanguage(nextLanguage: Language): void {
    preferences = { ...preferences, language: nextLanguage };
    applyPreferencesImmediately();
    void persistPreferences();
  }

  async function restoreLastProject(): Promise<void> {
    if (project) return;
    try {
      const initialized = await initializeProject();
      if (project) return;
      if (await ensureProjectAccess(initialized.project.packagePath)) {
        await activateProject(initialized.project);
      } else {
        await activateProject(await createTemporaryProject());
      }
      if (initialized.unavailableProjectPath) {
        notify("warn", t("previousProjectUnavailable"), t("temporaryProjectCreated"));
      }
    } catch (error) {
      notify("error", t("operationFailed"), errorText(error));
    }
  }

  async function openRequestedProject(): Promise<void> {
    const packagePath = await takeOpenProjectRequest();
    if (!packagePath) return;
    await run(async () => {
      window.clearTimeout(practiceSaveTimer);
      if (!await persistCurrentPracticeState()) return;
      if (!await ensureProjectAccess(packagePath)) return;
      await activateProject(await openProject(packagePath));
    });
  }

  async function run(action: () => Promise<void>): Promise<void> {
    busy = true;
    try {
      await action();
    } catch (error) {
      notify("error", t("operationFailed"), errorText(error));
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
      const packagePath = await open(projectOpenDialogOptions(t("openProject"), runtimeOs === "macos"));
      if (!packagePath) return;
      if (!await ensureProjectAccess(packagePath)) return;
      window.clearTimeout(practiceSaveTimer);
      if (!await persistCurrentPracticeState()) return;
      await activateProject(await openProject(packagePath));
    });
  }

  function openRecent(packagePath: string): void {
    void run(async () => {
      if (!await ensureProjectAccess(packagePath)) return;
      window.clearTimeout(practiceSaveTimer);
      if (!await persistCurrentPracticeState()) return;
      await activateProject(await openProject(packagePath));
    });
  }

  async function ensureProjectAccess(packagePath: string): Promise<boolean> {
    try {
      await verifyProjectAccess(packagePath);
      return true;
    } catch (error) {
      notify("error", t("operationFailed"), errorText(error));
      return false;
    }
  }

  async function ensureProjectDestinationAccess(destination: string): Promise<boolean> {
    try {
      await verifyProjectDestinationAccess(destination);
      return true;
    } catch (error) {
      notify("error", t("operationFailed"), errorText(error));
      return false;
    }
  }

  async function activateProject(nextProject: ProjectSummary): Promise<void> {
    await resetTrackState();
    project = nextProject;
    const rememberedId = rememberedTrackId(window.localStorage, nextProject.packagePath);
    const track = preferredTrack(nextProject.tracks, rememberedId);
    if (track) selectTrack(track, { autoplay: false });
    else forgetTrackSelection(window.localStorage, nextProject.packagePath);
  }

  async function resetTrackState(): Promise<void> {
    ++trackSelectionGeneration;
    backgroundTaskScheduler.cancelAll();
    await cancelChordAnalysis();
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
    chordsLoading = false;
    chordAnalysis = null;
    chordAnalysisError = "";
    chordEdits = [];
    selectedChordKey = null;
    cancelChordEdit();
    lastFollowedChordIndex = -1;
    chordScrollSuspended = false;
    chordPointerInside = false;
    chordFocusWithin = false;
    chordFocusRestorePending = false;
    chordProgrammaticScroll = false;
    currentSeconds = 0;
    durationSeconds = 0;
    playbackRate = preferences.defaultPlaybackRate;
    pitchSemitones = preferences.defaultPitchSemitones;
    volume = preferences.masterVolume;
    volumeBeforeMute = volume > 0 ? volume : 1;
    masterPeak = 0;
    masterPeakLeft = 0;
    masterPeakRight = 0;
    loopEnabled = false;
    loopA = null;
    loopB = null;
    usingDefaultLoopBounds = false;
    loopCommandGeneration += 1;
    loopDrag = null;
    waveform = null;
    waveformZoom = 1;
    waveformStart = 0;
    waveformDragPointerId = null;
    waveformPointerInside = false;
    waveformFocusWithin = false;
    waveformFollowNeedsSmooth = false;
    cancelWaveformFollowAnimation();
    viewportDrag = null;
    detectedBpm = null;
    metronomeEnabled = false;
    metronomeVolume = preferences.metronomeVolume;
    metronomeSound = preferences.metronomeSound;
    trainerEnabled = false;
    trainerStartRate = preferences.defaultTrainerStartRate;
    trainerRepetitions = preferences.defaultTrainerRepetitions;
    trainerIncrement = preferences.defaultTrainerIncrement;
    trainerTargetRate = preferences.defaultTrainerTargetRate;
    trainerLoopCount = 0;
    endedGeneration = 0;
    spectrumBands = Array<number>(64).fill(0);
    stems = { state: "disabled", enabled: false, progress: 0, stage: "disabled", trackId: null, cached: false, error: null, computeBackend: null };
    stemMix = Array.from({ length: 6 }, () => ({ gain: 1, pan: 0, muted: false, soloed: false }));
    stemNames = [...canonicalStemNames];
    stemPeaks = Array<number>(6).fill(0);
    stemPlaybackLockGeneration += 1;
    stemPlaybackLocked = false;
    stemPlaybackResume = null;
    stemGenerationStarting = false;
    editingTrackId = null;
    editingTrackLocation = null;
    draggedTrackId = null;
    dropTrackId = null;
    dropTrackIndex = null;
    trackContextMenu = null;
    await audioPause();
    await stemDisable();
    await audioSetLoop(null, null);
    await audioSetMetronome(false, metronomeVolume, metronomeSound);
    await audioSetLoopTrainer(false, trainerStartRate, trainerRepetitions, trainerIncrement, trainerTargetRate, null, null);
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

  function startTrackRename(track: TrackSummary, location: "header" | "playlist" = "playlist"): void {
    editingTrackId = track.id;
    editingTrackTitle = track.title;
    editingTrackLocation = location;
  }

  function commitTrackRename(track: TrackSummary): void {
    if (editingTrackId !== track.id) return;
    const name = editingTrackTitle.trim();
    editingTrackId = null;
    editingTrackLocation = null;
    if (!project || !name || name === track.title) return;
    void run(async () => {
      project = await renameTrack(project!.packagePath, track.id, name);
      if (currentTrack?.id === track.id) currentTrack = project.tracks.find((item) => item.id === track.id) ?? null;
    });
  }

  function cancelTrackRename(): void {
    editingTrackId = null;
    editingTrackLocation = null;
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
      if (wasCurrent) {
        await resetTrackState();
        const replacement = project.tracks[deletedIndex] ?? project.tracks[project.tracks.length - 1];
        if (replacement) selectTrack(replacement, { autoplay: false });
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
      importBatches = importBatches
        .map((batch) => {
          batch.jobIds.delete(jobId);
          batch.states.delete(jobId);
          return batch;
        })
        .filter((batch) => batch.jobIds.size > 0);
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
      filters: [{ name: t("openProject"), extensions: ["sac"] }],
    });
    if (!destination) return false;
    if (!await ensureProjectDestinationAccess(destination)) return false;
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
      if (project?.temporary) {
        if (!await saveProjectToChosenLocation()) return;
      }
      else {
        window.clearTimeout(practiceSaveTimer);
        if (!await persistCurrentPracticeState()) return;
      }
      notify("success", t("projectSaved"));
    });
  }

  function saveAs(): void {
    if (!project) return;
    void run(async () => {
      if (await saveProjectToChosenLocation()) notify("success", t("projectSaved"));
    });
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
    try {
      if (!await saveProjectToChosenLocation()) return;
      closePromptVisible = false;
      await confirmApplicationExit();
    } catch (error) {
      notify("error", t("operationFailed"), errorText(error));
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
    else if (id === "playlist:export_stems") {
      if (stems.state === "ready" && stems.trackId === currentTrack?.id) openStemExport();
      else notify("warn", t("exportStemsUnavailable"));
    }
    else if (id === "playlist:export_chords") void exportCurrentChords();
    else if (id.startsWith("recent:")) {
      const index = Number(id.slice("recent:".length));
      const recent = await listRecentProjects();
      if (Number.isInteger(index) && recent[index]) openRecent(recent[index]);
    } else if (id === "view:zoom_in") setWaveformZoom(waveformZoom * 1.5);
    else if (id === "view:console") toggleConsole();
    else if (id === "view:zoom_out") setWaveformZoom(waveformZoom / 1.5);
    else if (id === "view:zoom_reset") fitEntireWaveform();
    else if (id === "playback:toggle") togglePlayback();
    else if (id === "playback:back") navigate(-1);
    else if (id === "playback:forward") navigate(1);
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

  function fitEntireWaveform(): void {
    applyWaveformViewport({ start: 0, zoom: 1 });
  }

  function fitThirtySecondWaveform(): void {
    applyWaveformViewport(waveformViewportForWindow(durationSeconds, 30, currentSeconds));
  }

  function addTracks(): void {
    openImportCenter();
  }

  function openImportCenter(): void {
    importVisible = true;
  }

  function positionIsInside(position: { x: number; y: number } | undefined, element: HTMLElement | undefined): boolean {
    if (!position || !element) return false;
    const rect = element.getBoundingClientRect();
    const scale = window.devicePixelRatio || 1;
    const contains = (x: number, y: number): boolean => x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
    return contains(position.x, position.y) || contains(position.x / scale, position.y / scale);
  }

  function isImportDropTarget(position: { x: number; y: number } | undefined): boolean {
    return positionIsInside(position, importTextarea);
  }

  function isPlaylistDropTarget(position: { x: number; y: number } | undefined): boolean {
    return positionIsInside(position, playlistPanel);
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
    await run(async () => {
      await exportPlaylist(project!.packagePath, destination, format);
      notify("success", t("playlistExported"));
    });
  }

  async function chooseImportFiles(): Promise<void> {
    const selected = await open({
        multiple: true,
        title: t("importAudio"), filters: [{ name: t("importAudio"), extensions: ["wav", "mp3", "flac", "txt", "md"] }],
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
    importSearchCompleted = 0;
    importSearchTotal = 0;
    importActiveGroupIds = new Set();
    try {
      const backendSearchGeneration = await beginYoutubeSearches();
      if (generation !== importAnalysisGeneration) return;
      const parsed = deduplicateImportCandidates(await analyzeImportText(importText));
      if (generation !== importAnalysisGeneration) return;
      const previousGroups = new Map(importCandidateGroups.map((group) => [group.id, group]));
      const groups: ImportCandidateGroup[] = [];
      const direct = parsed.filter((candidate) => candidate.kind !== "search");
      if (direct.length) {
        groups.push({ id: "direct", query: null, searchIndex: null, candidates: direct });
      }
      let searchIndex = 0;
      const unresolved: ImportCandidateGroup[] = [];
      for (const candidate of parsed) {
        if (candidate.kind !== "search") continue;
        searchIndex += 1;
        const normalizedQuery = normalizeImportQuery(candidate.input);
        const id = `search:${normalizedQuery}`;
        const cached = importSearchCache.peek(candidate.input);
        const previous = previousGroups.get(id)?.candidates;
        const group = {
          id,
          query: candidate.input,
          searchIndex,
          candidates: cached ?? previous ?? [],
        };
        groups.push(group);
        if (cached === undefined && previous === undefined) unresolved.push(group);
      }
      importSearchTotal = searchIndex;
      importSearchCompleted = searchIndex - unresolved.length;
      importPendingGroupIds = new Set(unresolved.map((group) => group.id));
      importGroupErrors = new Map();
      publishImportGroups(groups);

      let nextSearchIndex = 0;
      const resolveNextSearch = async (): Promise<void> => {
        while (generation === importAnalysisGeneration) {
          const group = unresolved[nextSearchIndex++];
          if (!group || group.query === null) return;
          importActiveGroupIds = new Set(importActiveGroupIds).add(group.id);
          try {
            group.candidates = deduplicateImportCandidates(await importSearchCache.resolve(group.query, backendSearchGeneration));
          } catch (error) {
            if (generation !== importAnalysisGeneration) return;
            importGroupErrors = new Map(importGroupErrors).set(group.id, error instanceof Error ? error.message : String(error));
          }
          if (generation !== importAnalysisGeneration) return;
          const pending = new Set(importPendingGroupIds);
          pending.delete(group.id);
          importPendingGroupIds = pending;
          const active = new Set(importActiveGroupIds);
          active.delete(group.id);
          importActiveGroupIds = active;
          importSearchCompleted += 1;
          publishImportGroups(groups);
        }
      };
      await Promise.all(Array.from({ length: Math.min(2, unresolved.length) }, () => resolveNextSearch()));
      importHasAnalyzed = true;
    } catch (error) {
      if (generation !== importAnalysisGeneration) return;
      importCandidates = [];
      importCandidateGroups = [];
      selectedImports = new Set();
      importHasAnalyzed = true;
      importAnalysisError = error instanceof Error ? error.message : String(error);
    } finally {
      if (generation === importAnalysisGeneration) {
        importAnalyzing = false;
        importActiveGroupIds = new Set();
        importPendingGroupIds = new Set();
      }
    }
  }

  function publishImportGroups(groups: ImportCandidateGroup[]): void {
    const previousGroups = importCandidateGroups;
    const previousSelection = selectedImports;
    const nextGroups = groups.map((group) => ({ ...group, candidates: [...group.candidates] }));
    importCandidateGroups = nextGroups;
    importCandidates = nextGroups.flatMap((group) => group.candidates);
    selectedImports = reconcileImportSelection(previousSelection, previousGroups, nextGroups, preferences.youtubeAutoSelectBestMatch);
  }

  function scheduleImportAnalysis(): void {
    window.clearTimeout(importAnalysisTimer);
    importHasAnalyzed = false;
    importAnalysisTimer = window.setTimeout(() => void analyzeImports(), 650);
  }

  function toggleImport(input: string): void {
    const next = new Set(selectedImports);
    if (next.has(input)) next.delete(input); else next.add(input);
    selectedImports = next;
  }

  async function startImports(): Promise<void> {
    if (selectedImports.size === 0) return;
    const inputs = [...selectedImports];
    await run(async () => {
      await enqueueImportInputs(inputs);
      importVisible = false; tasksVisible = false; importText = ""; importCandidates = []; importCandidateGroups = []; selectedImports = new Set();
    });
  }

  function handleImportDialogKeydown(event: KeyboardEvent): void {
    const canImport = selectedImports.size > 0 && !importAnalyzing && !busy;
    if (!shouldConfirmDialogOnEnter(event, canImport)) return;
    event.preventDefault();
    event.stopPropagation();
    void startImports();
  }

  async function enqueueImportInputs(inputs: string[]): Promise<void> {
    if (!project) await activateProject(await createTemporaryProject());
    const activeProject = project;
    if (!activeProject) return;
    const addedJobs = await enqueueImports(activeProject.packagePath, inputs);
    importQueue = [...importQueue, ...addedJobs];
    if (addedJobs.length > 0) {
      importBatches = [...importBatches, {
        jobIds: new Set(addedJobs.map((job) => job.id)),
        states: new Map(addedJobs.map((job) => [job.id, job.state])),
      }];
    }
    await refreshImportJobs();
  }

  async function importDroppedAudio(paths: string[]): Promise<void> {
    const audioPaths = droppedAudioPaths(paths);
    if (audioPaths.length === 0) {
      notify("warn", t("unsupportedAudioDrop"));
      return;
    }
    await run(() => enqueueImportInputs(audioPaths));
  }

  async function refreshImportJobs(): Promise<void> {
    try {
      const previousCompleted = importQueue.filter((job) => job.state === "completed").length;
      const jobs = await importJobs();
      importQueue = jobs;
      const jobsById = new Map(jobs.map((job) => [job.id, job]));
      const pendingBatches: ImportBatch[] = [];
      for (const batch of importBatches) {
        for (const jobId of batch.jobIds) {
          const state = jobsById.get(jobId)?.state;
          if (state) batch.states.set(jobId, state);
        }
        const completion = completedImportBatch(batch.jobIds.size, batch.states.values());
        if (!completion) {
          pendingBatches.push(batch);
          continue;
        }
        notify(completion.failed > 0 ? "warn" : "success", importSummary(completion.completed, completion.failed));
      }
      importBatches = pendingBatches;
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
          if (track) selectTrack(track, { autoplay: false });
        }
      }
    } catch { /* Background status is best-effort during shutdown. */ }
  }

  function showDiagnostics(): void {
    void run(async () => {
      diagnosticInfo = await diagnostics();
    });
  }

  function showPathInFileManager(path: string): void {
    void revealProject(path).catch((error) => {
      notify("error", t("revealError"), errorText(error));
    });
  }

  function showProjectInFileManager(): void {
    if (project) showPathInFileManager(project.packagePath);
  }

  function openCommunityLink(target: "github" | "donate"): void {
    void openExternalLink(target).catch((error) => {
      notify("error", t("linkOpenError"), errorText(error));
    });
  }

  function openImportVideo(videoId: string): void {
    void openYoutubeVideo(videoId).catch((error) => {
      notify("error", t("linkOpenError"), errorText(error));
    });
  }

  function hideBrokenThumbnail(event: Event): void {
    if (event.currentTarget instanceof HTMLImageElement) event.currentTarget.hidden = true;
  }

  function selectTrack(
    track: TrackSummary,
    options: { autoplay?: boolean } = {},
  ): void {
    if (!project) return;
    const { autoplay = true } = options;
    stemPlaybackLockGeneration += 1;
    stemPlaybackLocked = false;
    stemPlaybackResume = null;
    stemGenerationStarting = false;
    cancelPendingSeek();
    const packagePath = project.packagePath;
    const selectionGeneration = ++trackSelectionGeneration;
    window.clearTimeout(playbackRateTimer);
    window.clearTimeout(pitchTimer);
    playbackRateTimer = undefined;
    pitchTimer = undefined;
    void persistCurrentPracticeState();
    void audioPause();
    void cancelChordAnalysis();
    void stemDisable();
    stems = { state: "disabled", enabled: false, progress: 0, stage: "disabled", trackId: null, cached: false, error: null, computeBackend: null };
    stemPeaks = Array<number>(6).fill(0);
    isPlaying = false;
    masterPeak = 0;
    masterPeakLeft = 0;
    masterPeakRight = 0;
    audioLoading = true;
    loadingTrackId = track.id;
    currentTrack = track;
    rememberTrackSelection(window.localStorage, packagePath, track.id);
    durationSeconds = track.durationSeconds ?? 0;
    playbackRate = track.practice.playbackRate;
    pitchSemitones = track.practice.pitchSemitones ?? 0;
    volume = preferences.masterVolume;
    volumeBeforeMute = volume > 0 ? volume : 1;
    loopEnabled = track.practice.loopEnabled ?? (track.practice.loopASeconds !== null && track.practice.loopBSeconds !== null);
    const loopBounds = defaultLoopBounds(track.practice.loopASeconds, track.practice.loopBSeconds, durationSeconds);
    loopA = loopBounds.a;
    loopB = loopBounds.b;
    currentSeconds = trackLoadPosition(loopEnabled, loopA, preferences.loopLoadPosition);
    usingDefaultLoopBounds = track.practice.loopASeconds === null && track.practice.loopBSeconds === null;
    metronomeEnabled = track.practice.metronomeEnabled ?? false;
    metronomeVolume = preferences.metronomeVolume;
    metronomeSound = preferences.metronomeSound;
    trainerEnabled = track.practice.trainerEnabled ?? false;
    trainerStartRate = track.practice.trainerStartRate;
    trainerRepetitions = track.practice.trainerRepetitions ?? 1;
    trainerIncrement = track.practice.trainerIncrement ?? 0.05;
    trainerTargetRate = track.practice.trainerTargetRate ?? 1;
    stemMix = track.practice.stemMix;
    stemNames = track.practice.stemNames;
    chordEdits = track.practice.chordEdits ?? [];
    selectedChordKey = null;
    cancelChordEdit();
    stemMix.forEach((value, index) => void stemSetMix(index, value.gain, value.pan, value.muted, value.soloed));
    trainerLoopCount = 0;
    spectrumBands = Array<number>(64).fill(0);
    tempoLoading = true;
    detectedBpm = null;
    chordAnalysis = null;
    chordsLoading = false;
    chordAnalysisError = "";
    lastFollowedChordIndex = -1;
    chordScrollSuspended = false;
    chordPointerInside = false;
    chordFocusWithin = false;
    chordFocusRestorePending = false;
    chordProgrammaticScroll = false;
    void loadTrackWaveform(track, packagePath, selectionGeneration);
    void loadSelectedAudio(track, packagePath, selectionGeneration, autoplay);
  }

  function finishStemPlaybackLock(trackId: string | null): void {
    if (!stemPlaybackLocked) return;
    const resumeRequest = stemPlaybackResume;
    stemPlaybackLockGeneration += 1;
    stemPlaybackLocked = false;
    stemPlaybackResume = null;
    stemGenerationStarting = false;
    if (trackId !== null
      && resumeRequest?.trackId !== trackId) return;
    if (shouldResumeStemPlayback(
      resumeRequest,
      currentTrack?.id,
      trackSelectionGeneration,
    )) void play();
  }

  async function enableStems(options: { resumePlayback?: boolean } = {}): Promise<void> {
    if (!project || !currentTrack) return;
    if (stemPlaybackLocked || stems.state === "separating") return;
    const trackId = currentTrack.id;
    const packagePath = project.packagePath;
    const selectionGeneration = trackSelectionGeneration;
    const lockGeneration = ++stemPlaybackLockGeneration;
    stemPlaybackLocked = true;
    stemGenerationStarting = true;
    stemPlaybackResume = stemPlaybackResumeRequest(
      options.resumePlayback ?? (isPlaying || playRequestActive !== null),
      trackId,
      selectionGeneration,
    );
    const pendingPlay = playRequestActive;
    if (pendingPlay) {
      try {
        await pendingPlay;
      } catch {
        // The play request reports its own error; separation still requires an explicit pause.
      }
    }
    try {
      await audioPause();
    } catch (error) {
      if (lockGeneration === stemPlaybackLockGeneration) {
        stemPlaybackLocked = false;
        stemPlaybackResume = null;
        stemGenerationStarting = false;
        notify("error", t("playbackError"), errorText(error));
      }
      return;
    }
    if (lockGeneration !== stemPlaybackLockGeneration
      || currentTrack?.id !== trackId
      || trackSelectionGeneration !== selectionGeneration) return;
    if (isPlaying) {
      isPlaying = false;
      schedulePracticeSave(0);
    }
    stems = { state: "separating", enabled: true, progress: 0, stage: "checkingCache", trackId, cached: false, error: null, computeBackend: null };
    try {
      await stemStart(packagePath, trackId);
      if (lockGeneration !== stemPlaybackLockGeneration) return;
      stemGenerationStarting = false;
      schedulePracticeSave();
    } catch (error) {
      if (lockGeneration !== stemPlaybackLockGeneration) return;
      stemGenerationStarting = false;
      stems = { state: "failed", enabled: false, progress: 0, stage: "failed", trackId, cached: false, error: errorText(error), computeBackend: null };
      notify("error", t("stemFailed"), errorText(error));
      finishStemPlaybackLock(trackId);
    }
  }

  async function disableStems(): Promise<void> {
    const generatedTrackId = stems.trackId;
    await stemDisable();
    stems = { state: "disabled", enabled: false, progress: 0, stage: "disabled", trackId: null, cached: false, error: null, computeBackend: null };
    stemPeaks = Array<number>(6).fill(0);
    schedulePracticeSave();
    finishStemPlaybackLock(generatedTrackId);
  }

  async function toggleStemMode(event: Event): Promise<void> {
    const enabled = (event.currentTarget as HTMLInputElement).checked;
    if (enabled) {
      if (stems.state === "ready") {
        stems = { ...stems, enabled: await stemSetEnabled(true) };
        schedulePracticeSave();
      } else {
        await enableStems();
      }
    } else if (stems.state === "separating") {
      await disableStems();
    } else if (stems.state === "ready") {
      await stemSetEnabled(false);
      stems = { ...stems, enabled: false };
      stemPeaks = Array<number>(6).fill(0);
      schedulePracticeSave();
    }
  }

  async function refreshStemStatus(): Promise<void> {
    if (stemStatusRequestActive || stemGenerationStarting || (stems.state === "disabled" && !stemPlaybackLocked)) return;
    const requestedLockGeneration = stemPlaybackLockGeneration;
    stemStatusRequestActive = true;
    try {
      const previous = stems;
      const next = await stemStatus();
      if (requestedLockGeneration !== stemPlaybackLockGeneration) return;
      if (!next.trackId || next.trackId === currentTrack?.id) {
        stems = next;
        if (next.state === "ready"
          && next.trackId === currentTrack?.id
          && (previous.state !== "ready" || previous.trackId !== next.trackId)
          && project
          && currentTrack) {
          const track = currentTrack;
          const packagePath = project.packagePath;
          void loadTrackChords(track, packagePath, trackSelectionGeneration);
        }
        if (stemPlaybackLocked && next.state !== "separating") {
          finishStemPlaybackLock(previous.trackId);
        }
      }
    } finally { stemStatusRequestActive = false; }
  }

  function updateStem(index: number, change: Partial<StemMix>): void {
    stemMix = stemMix.map((value, candidate) => candidate === index ? { ...value, ...change } : value);
    const value = stemMix[index];
    void stemSetMix(index, value.gain, value.pan, value.muted, value.soloed);
    schedulePracticeSave();
  }

  function renameStem(index: number, value: string): void {
    const name = value.trim().slice(0, 40);
    if (!name) return;
    stemNames = stemNames.map((current, candidate) => candidate === index ? name : current);
    schedulePracticeSave(0);
  }

  function stemDisplayName(index: number): string {
    const name = stemNames[index] ?? canonicalStemNames[index];
    return name === canonicalStemNames[index] ? t(canonicalStemNames[index]) : name;
  }

  function formatStemGain(gain: number): string {
    if (gain <= 0.0001) return "−∞ dB";
    const decibels = 20 * Math.log10(gain);
    return `${decibels > 0 ? "+" : ""}${decibels.toFixed(1)} dB`;
  }

  function formatPan(pan: number): string {
    if (Math.abs(pan) < 0.005) return "C";
    return `${pan < 0 ? "L" : "R"} ${Math.round(Math.abs(pan) * 100)}`;
  }

  function stemMeterLevel(peak: number): number {
    if (peak <= 0.001) return 0;
    return Math.min(1, Math.max(0, (20 * Math.log10(peak) + 60) / 60));
  }

  function openStemExport(): void {
    if (stems.state !== "ready" || stems.trackId !== currentTrack?.id) return;
    stemExportFormat = preferences.conversionFormat === "mp3" ? "mp3" : "wav";
    stemExportVisible = true;
  }

  function safeStemExportFolderName(value: string): string {
    return value.replace(/[\\/:*?"<>|]/g, "_").trim().slice(0, 80) || "Stems";
  }

  async function exportCurrentStems(): Promise<void> {
    if (!project || !currentTrack || stems.state !== "ready" || stems.trackId !== currentTrack.id) return;
    const track = currentTrack;
    const packagePath = project.packagePath;
    const destination = await save({
      title: t("exportStemsDestination"),
      defaultPath: `${safeStemExportFolderName(track.title)} - Stems`,
    });
    if (!destination) return;
    await run(async () => {
      await exportStems(packagePath, track.id, destination, stemExportFormat, stemNames.map((_, index) => stemDisplayName(index)));
      stemExportVisible = false;
      notify("success", t("stemExportComplete"));
    });
  }

  async function exportCurrentChords(): Promise<void> {
    if (!currentTrack || !effectiveChords.length) {
      notify("warn", t("exportChordsUnavailable"));
      return;
    }
    const track = currentTrack;
    const destination = await save({
      title: t("exportChordsDestination"),
      defaultPath: `${safeStemExportFolderName(track.title)}.jams`,
      filters: [{ name: "JAMS", extensions: ["jams"] }],
    });
    if (!destination) return;
    await run(async () => {
      await exportChords(destination, track.title, durationSeconds, chordMode, chordSegmentsForJams(effectiveChords));
      notify("success", t("chordExportComplete"));
    });
  }

  async function loadTrackChords(track: TrackSummary, packagePath: string, selectionGeneration: number): Promise<void> {
    chordsLoading = true;
    tempoLoading = true;
    detectedBpm = null;
    chordAnalysisError = "";
    const stillSelected = (): boolean => selectionGeneration === trackSelectionGeneration
      && project?.packagePath === packagePath
      && currentTrack?.id === track.id;
    try {
      // Rust owns the versioned, source-aware cache. Always cross that boundary
      // so a newly available stem cache cannot be hidden by an older in-memory
      // mix result retained by the webview.
      const analysis = await analyzeChords(packagePath, track.id);
      if (stillSelected()) {
        chordAnalysis = analysis;
        detectedBpm = analysis.bpm;
        await audioSetBeatTimeline(analysis.beats, analysis.downbeats);
      }
    } catch (error) {
      if (stillSelected()) {
        chordAnalysisError = errorText(error);
        chordAnalysis = null;
        detectedBpm = null;
        await audioSetBeatTimeline([], []);
        notify("error", t("chordAnalysisFailed"), chordAnalysisError);
      }
    } finally {
      if (stillSelected()) {
        chordsLoading = false;
        tempoLoading = false;
      }
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
      await audioSetBeatTimeline([], []);
      await audioSetMetronome(metronomeEnabled, metronomeVolume, metronomeSound);
      await audioSetLoopTrainer(trainerEnabled, trainerStartRate, trainerRepetitions, trainerIncrement, trainerTargetRate, loopA, loopB);
      await audioSetEndBehavior(endBehavior);
      if (!stillSelected()) return;
      endedGeneration = status.endedGeneration;
      applyLoopToEngine();
      const loopWasDisabled = await audioSeek(currentSeconds);
      if (loopWasDisabled) disableLoopBeyondB(currentSeconds);
      if (!stillSelected()) return;
      audioLoading = false;
      loadingTrackId = null;
      void loadTrackChords(track, packagePath, selectionGeneration);
      if (track.practice.stemsEnabled) void enableStems({ resumePlayback: autoplay });
      else if (autoplay) await play();
      if (!stillSelected()) return;
      schedulePlaylistWarmup(packagePath, track.id);
    } catch (error) {
      if (stillSelected()) {
        notify("error", t("playbackError"), errorText(error));
      }
    } finally {
      if (stillSelected()) {
        audioLoading = false;
        loadingTrackId = null;
      }
    }
  }

  function schedulePlaylistWarmup(packagePath: string, selectedTrackId: string): void {
    if (!project || warmedProjects.has(packagePath)) return;
    backgroundTaskScheduler.cancelScope(packagePath);
    const selectedIndex = project.tracks.findIndex((track) => track.id === selectedTrackId);
    const tracks = project.tracks.filter((track) => track.id !== selectedTrackId);
    if (selectedIndex >= 0 && tracks.length > 1) {
      const nextId = project.tracks[(selectedIndex + 1) % project.tracks.length]?.id;
      tracks.sort((left, right) => Number(right.id === nextId) - Number(left.id === nextId));
    }
    for (const track of tracks) {
      backgroundTaskScheduler.enqueue({
        scope: packagePath,
        key: track.id,
        run: async () => {
          if (project?.packagePath !== packagePath) return;
          await audioPreload(packagePath, track.id);
        },
      });
    }
    backgroundTaskScheduler.enqueue({
      scope: packagePath,
      key: "playlist-warmed",
      run: async () => {
        if (project?.packagePath === packagePath) warmedProjects.add(packagePath);
      },
    });
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
        if (loaded.durationSeconds > 0) durationSeconds = loaded.durationSeconds;
        const initialViewport = waveformViewportForWindow(loaded.durationSeconds);
        waveformStart = initialViewport.start;
        waveformZoom = initialViewport.zoom;
        waveform = loaded;
      }
    } catch (error) {
      if (stillSelected()) notify("error", t("waveformError"), errorText(error));
    } finally {
      if (stillSelected()) waveformLoading = false;
    }
  }

  async function play(): Promise<void> {
    if (!currentTrack && project?.tracks.length) selectTrack(project.tracks[0], { autoplay: false });
    if (!currentTrack || audioLoading || stemPlaybackLocked) return;
    const request = audioPlay();
    playRequestActive = request;
    try {
      await request;
      if (stemPlaybackLocked) {
        await audioPause();
        isPlaying = false;
        return;
      }
      isPlaying = true;
    } catch (error) {
      notify("error", t("playbackError"), errorText(error));
    } finally {
      if (playRequestActive === request) playRequestActive = null;
    }
  }

  function togglePlayback(): void {
    if (stemPlaybackLocked) return;
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
    seekGeneration += 1;
    currentSeconds = Math.max(0, Math.min(position, durationSeconds));
    disableLoopBeyondB(currentSeconds);
    pendingSeekPosition = currentSeconds;
    if (seekAnimationFrame !== undefined) {
      window.cancelAnimationFrame(seekAnimationFrame);
      seekAnimationFrame = undefined;
    }
    void flushPendingSeek();
  }

  function scrub(position: number): void {
    if (!Number.isFinite(position)) return;
    seekGeneration += 1;
    currentSeconds = Math.max(0, Math.min(position, durationSeconds));
    disableLoopBeyondB(currentSeconds);
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
      const loopWasDisabled = await audioSeek(position);
      if (loopWasDisabled) disableLoopBeyondB(position);
    } catch (error) {
      notify("error", t("playbackError"), errorText(error));
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

  function disableLoopBeyondB(position: number): void {
    if (!loopEnabled || loopB === null || position < loopB) return;
    loopCommandGeneration += 1;
    loopEnabled = false;
    if (trainerEnabled) {
      trainerEnabled = false;
      trainerLoopCount = 0;
    }
    schedulePracticeSave(0);
  }

  function navigate(direction: -1 | 1): void {
    seek(navigationPosition(
      preferences.navigationMode,
      currentSeconds,
      direction,
      preferences.navigationTimeSeconds,
      chordAnalysis?.beats ?? [],
      timelineChords,
    ));
  }

  function stopJumpHold(): void {
    window.clearTimeout(jumpHoldDelayTimer);
    window.clearInterval(jumpHoldRepeatTimer);
    jumpHoldDelayTimer = undefined;
    jumpHoldRepeatTimer = undefined;
  }

  function startJumpHold(event: PointerEvent, direction: -1 | 1): void {
    if (event.button !== 0) return;
    event.preventDefault();
    stopJumpHold();
    const target = event.currentTarget as HTMLButtonElement;
    target.focus();
    target.setPointerCapture(event.pointerId);
    navigate(direction);
    jumpHoldDelayTimer = window.setTimeout(() => {
      jumpHoldDelayTimer = undefined;
      jumpHoldRepeatTimer = window.setInterval(() => navigate(direction), 140);
    }, 400);
  }

  function finishJumpHold(event: PointerEvent): void {
    const target = event.currentTarget as HTMLButtonElement;
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    stopJumpHold();
  }

  function keyboardJump(event: MouseEvent, direction: -1 | 1): void {
    if (event.detail === 0) navigate(direction);
  }

  function toggleMetronome(): void {
    if (!chordAnalysis?.beats.length) return;
    metronomeEnabled = !metronomeEnabled;
    void audioSetMetronome(metronomeEnabled, metronomeVolume, metronomeSound);
    schedulePracticeSave();
  }

  function changeMetronomeVolume(value: number): void {
    metronomeVolume = Math.max(0, Math.min(1, value));
    void audioSetMetronome(metronomeEnabled, metronomeVolume, metronomeSound);
    preferences = { ...preferences, metronomeVolume };
    void persistPreferences();
  }

  function changeMetronomeSound(value: string): void {
    if (value !== "electronic" && value !== "woodblock" && value !== "metallic") return;
    metronomeSound = value;
    void audioSetMetronome(metronomeEnabled, metronomeVolume, metronomeSound);
    preferences = { ...preferences, metronomeSound };
    void persistPreferences();
  }

  function applyLoopTrainer(): void {
    void audioSetLoopTrainer(trainerEnabled, trainerStartRate, trainerRepetitions, trainerIncrement, trainerTargetRate, loopA, loopB);
    schedulePracticeSave();
  }

  function toggleLoopTrainer(): void {
    trainerEnabled = !trainerEnabled;
    if (trainerEnabled) {
      ensureValidLoopBounds();
      loopEnabled = true;
      playbackRate = trainerStartRate;
      window.clearTimeout(playbackRateTimer);
      playbackRateTimer = undefined;
      const generation = ++loopCommandGeneration;
      void activateTrainingAtA(generation);
      schedulePracticeSave();
      return;
    }
    loopCommandGeneration += 1;
    applyLoopTrainer();
  }

  async function activateTrainingAtA(generation: number): Promise<void> {
    if (loopA === null || loopB === null) return;
    cancelPendingSeek();
    currentSeconds = loopA;
    try {
      await audioSetLoopTrainer(true, trainerStartRate, trainerRepetitions, trainerIncrement, trainerTargetRate, loopA, loopB);
      if (generation !== loopCommandGeneration || !trainerEnabled) return;
      await audioSeek(loopA);
    } catch (error) {
      notify("error", t("playbackError"), errorText(error));
    }
  }

  function openTrainingSettings(): void {
    trainingDraft = { startRate: trainerStartRate, targetRate: trainerTargetRate, increment: trainerIncrement, repetitions: trainerRepetitions };
    trainingSettingsVisible = true;
  }

  function resetTrainingDraft(): void {
    trainingDraft = {
      startRate: preferences.defaultTrainerStartRate,
      targetRate: preferences.defaultTrainerTargetRate,
      increment: preferences.defaultTrainerIncrement,
      repetitions: preferences.defaultTrainerRepetitions,
    };
    saveTrainingSettings();
  }

  function saveTrainingSettings(): void {
    trainerStartRate = Math.max(0.5, Math.min(1.99, trainingDraft.startRate));
    trainerTargetRate = Math.max(trainerStartRate + 0.01, Math.min(2, trainingDraft.targetRate));
    trainerIncrement = Math.max(0.01, Math.min(0.25, trainingDraft.increment));
    trainerRepetitions = Math.max(1, Math.min(99, Math.round(trainingDraft.repetitions)));
    trainingDraft = { startRate: trainerStartRate, targetRate: trainerTargetRate, increment: trainerIncrement, repetitions: trainerRepetitions };
    applyLoopTrainer();
  }

  function setLoopA(): void {
    loopA = snappedLoopTime(currentSeconds);
    if (usingDefaultLoopBounds || (loopB !== null && loopB <= loopA)) loopB = null;
    usingDefaultLoopBounds = false;
    loopEnabled = true;
    applyLoopToEngine();
    schedulePracticeSave();
  }

  function setLoopB(): void {
    if (loopA === null) {
      loopA = 0;
    }
    const nextLoopB = snappedLoopTime(currentSeconds);
    if (nextLoopB > loopA) loopB = nextLoopB;
    usingDefaultLoopBounds = false;
    loopEnabled = true;
    applyLoopToEngine();
    schedulePracticeSave();
  }

  function clearLoop(): void {
    loopCommandGeneration += 1;
    loopA = null;
    loopB = null;
    usingDefaultLoopBounds = false;
    loopEnabled = false;
    if (trainerEnabled) {
      trainerEnabled = false;
      trainerLoopCount = 0;
      applyLoopTrainer();
    }
    void audioSetLoop(null, null);
    schedulePracticeSave();
  }

  function snappedLoopTime(seconds: number): number {
    return loopSnapEnabled
      ? snappedNavigationPosition(preferences.navigationMode, seconds, chordAnalysis?.beats ?? [], timelineChords)
      : seconds;
  }

  function toggleLoopSnap(): void {
    if (!loopSnapAvailable) return;
    loopSnapEnabled = !loopSnapEnabled;
    preferences = { ...preferences, loopSnapEnabled };
    void persistPreferences();
  }

  function resetLoopBoundary(event: MouseEvent, boundary: "a" | "b"): void {
    event.preventDefault();
    event.stopPropagation();
    if (durationSeconds <= 0) return;
    if (boundary === "a") {
      loopA = 0;
    } else {
      loopB = durationSeconds;
    }
    usingDefaultLoopBounds = loopA === 0 && loopB === durationSeconds;
    loopEnabled = true;
    applyLoopToEngine();
    schedulePracticeSave(0);
  }

  function masterVolumeColor(value: number): string {
    if (value <= 1) return "var(--accent)";
    const dangerPercent = Math.round(Math.max(0, Math.min(1, value - 1)) * 100);
    return `color-mix(in srgb, var(--gold) ${100 - dangerPercent}%, var(--danger) ${dangerPercent}%)`;
  }

  function changeVolume(value: number): void {
    volume = Math.max(0, Math.min(2, value));
    if (volume > 0) volumeBeforeMute = volume;
    void audioSetVolume(volume);
    preferences = { ...preferences, masterVolume: volume };
    window.clearTimeout(volumePreferenceTimer);
    volumePreferenceTimer = window.setTimeout(() => void persistPreferences(), 180);
  }

  function changeMusicVolume(value: number): void {
    musicVolume = Math.max(0, Math.min(1, value));
    void audioSetMusicVolume(musicVolume);
    preferences = { ...preferences, musicVolume };
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
    repertoireKeyboardLabel = null;
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
    if (loopEnabled) {
      loopCommandGeneration += 1;
      loopEnabled = false;
      if (trainerEnabled) {
        trainerEnabled = false;
        trainerLoopCount = 0;
        applyLoopTrainer();
      }
      applyLoopToEngine();
      schedulePracticeSave();
      return;
    }
    ensureValidLoopBounds();
    if (loopA === null || loopB === null || loopB <= loopA) return;
    loopEnabled = true;
    const generation = ++loopCommandGeneration;
    void activateLoopAtA(generation);
    schedulePracticeSave();
  }

  function ensureValidLoopBounds(): void {
    if (durationSeconds <= 0) return;
    if (loopA === null || loopA >= durationSeconds) loopA = 0;
    if (loopB === null || loopB <= loopA) loopB = durationSeconds;
    usingDefaultLoopBounds = loopA === 0 && loopB === durationSeconds;
  }

  async function activateLoopAtA(generation: number): Promise<void> {
    if (loopA === null || loopB === null) return;
    const a = loopA;
    const b = loopB;
    cancelPendingSeek();
    currentSeconds = a;
    try {
      await audioSetLoop(a, b);
      if (generation !== loopCommandGeneration || !loopEnabled) return;
      await audioSeek(a);
    } catch (error) {
      notify("error", t("playbackError"), errorText(error));
    }
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
    const time = snappedLoopTime(eventTime(event, detailed));
    const minimum = Math.min(0.05, durationSeconds);
    if (loopDrag.mode === "a") {
      loopA = Math.max(0, Math.min(time, loopDrag.b - minimum));
    }
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
    const requestSeekGeneration = seekGeneration;
    const seekPendingAtRequest = seekRequestActive || pendingSeekPosition !== null || seekAnimationFrame !== undefined;
    statusRequestActive = true;
    try {
      const status = await audioStatus();
      if (!shouldApplyAudioStatus(
        audioLoading,
        selectionGeneration,
        trackSelectionGeneration,
        trackId,
        currentTrack?.id,
      )) return;
      if (shouldApplyAudioStatusPosition(
        requestSeekGeneration,
        seekGeneration,
        seekPendingAtRequest,
        seekRequestActive || pendingSeekPosition !== null || seekAnimationFrame !== undefined,
      )) {
        currentSeconds = status.positionSeconds;
      }
      durationSeconds = status.durationSeconds || durationSeconds;
      if (stemPlaybackLocked) {
        if (status.playing) void audioPause();
        isPlaying = false;
      } else isPlaying = status.playing;
      masterPeak = status.outputPeak;
      masterPeakLeft = status.outputPeakLeft;
      masterPeakRight = status.outputPeakRight;
      limiterReduction = status.limiterReduction;
      normalizationGain = status.normalizationGain;
      integratedLufs = status.integratedLufs;
      stemPeaks = status.stemPeaks;
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
        metronomeEnabled,
        metronomeVolume,
        trainerEnabled,
        trainerStartRate,
        trainerRepetitions,
        trainerIncrement,
        trainerTargetRate,
        stemsEnabled: stems.enabled,
        stemMix,
        stemNames,
        chordEdits,
      });
      if (project?.packagePath === packagePath) project = updated;
      return true;
    } catch (error) {
      notify("error", t("saveError"), errorText(error));
      return false;
    }
  }

  function navigateWaveformWithWheel(event: WheelEvent, overview: boolean): void {
    event.preventDefault();
    const target = event.currentTarget as HTMLElement;
    const bounds = target.getBoundingClientRect();
    const axis = event.ctrlKey
      ? "vertical"
      : waveformWheelAxis(event.deltaX, event.deltaY, activeWaveformWheelAxis);
    activeWaveformWheelAxis = axis;
    window.clearTimeout(waveformWheelAxisTimer);
    waveformWheelAxisTimer = window.setTimeout(() => {
      activeWaveformWheelAxis = null;
      waveformWheelAxisTimer = undefined;
    }, 140);
    if (axis === "horizontal") {
      applyWaveformViewport(panWaveformViewportFromWheel(
        waveformStart,
        waveformZoom,
        event.deltaX,
        bounds.width,
      ));
      return;
    }
    const pointerRatio = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    const anchorPosition = overview
      ? pointerRatio
      : waveformStart + pointerRatio / waveformZoom;
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

  function cancelWaveformFollowAnimation(): void {
    if (waveformFollowAnimationFrame === undefined) return;
    window.cancelAnimationFrame(waveformFollowAnimationFrame);
    waveformFollowAnimationFrame = undefined;
  }

  function smoothWaveformFollow(): void {
    cancelWaveformFollowAnimation();
    const initialStart = waveformStart;
    const animationStart = performance.now();
    const duration = 180;
    const animate = (now: number): void => {
      if (!followPlayhead || waveformFollowSuspended || waveformZoom <= 1 || durationSeconds <= 0) {
        waveformFollowAnimationFrame = undefined;
        return;
      }
      const progress = Math.min(1, (now - animationStart) / duration);
      const eased = 1 - (1 - progress) ** 3;
      const target = moveWaveformViewport(
        currentSeconds / durationSeconds - 0.5 / waveformZoom,
        waveformZoom,
        0,
      ).start;
      waveformStart = initialStart + (target - initialStart) * eased;
      if (progress < 1) {
        waveformFollowAnimationFrame = window.requestAnimationFrame(animate);
      } else {
        waveformFollowAnimationFrame = undefined;
        waveformStart = target;
      }
    };
    waveformFollowAnimationFrame = window.requestAnimationFrame(animate);
  }

  function toggleWaveformFollow(): void {
    followPlayhead = !followPlayhead;
    cancelWaveformFollowAnimation();
    waveformFollowNeedsSmooth = followPlayhead;
  }

  function enterDetailedWaveform(event: PointerEvent): void {
    if (event.pointerType !== "touch") waveformPointerInside = true;
  }

  function leaveDetailedWaveform(event: PointerEvent): void {
    if (event.pointerType !== "touch") waveformPointerInside = false;
  }

  function leaveDetailedWaveformFocus(event: FocusEvent): void {
    waveformFocusWithin = event.currentTarget instanceof HTMLElement
      && event.relatedTarget instanceof Node
      && event.currentTarget.contains(event.relatedTarget);
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
      const position = (waveformStart + local / waveformZoom) * durationSeconds;
      seek(navigationSnappedSeekPosition(position));
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

  function navigationSnappedSeekPosition(position: number): number {
    if (activeNavigationMode === "time") return position;
    return snappedNavigationPosition(preferences.navigationMode, position, chordAnalysis?.beats ?? [], timelineChords);
  }

  function seekAndCenterOverview(ratio: number): void {
    const position = navigationSnappedSeekPosition(ratio * durationSeconds);
    seek(position);
    const span = 1 / waveformZoom;
    const positionRatio = durationSeconds > 0 ? position / durationSeconds : ratio;
    applyWaveformViewport(moveWaveformViewport(positionRatio - span / 2, waveformZoom, 0));
  }
</script>

<svelte:head><title>SonArcan</title></svelte:head>

<main class="shell" class:console-open={consoleVisible} class:help-open={helpVisible} spellcheck="false">
  <header class="topbar">
    <div class="project-header">
      {#if project}
        <div class="project-name-wrap">
          {#if editingProjectName}
            <input class="project-name-input" bind:value={projectNameDraft} aria-label={t("projectName")} autocorrect="off" use:focusOnMount onblur={commitProjectName} onkeydown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); else if (event.key === "Escape") cancelProjectName(); }} />
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
      <button class="header-icon-link" aria-label={t("shortcuts")} data-tooltip={t("shortcuts")} onclick={() => shortcutsVisible = true}><Icon name="keyboard" size="15px" /></button>
      <button class="header-icon-link" class:active={helpVisible} aria-pressed={helpVisible} aria-label={helpVisible ? t("hideHelp") : t("showHelp")} data-tooltip={helpVisible ? t("hideHelp") : t("showHelp")} onclick={toggleHelp}><Icon name="lightbulb" size="15px" /></button>
      <button class="header-icon-link" class:active={consoleVisible} aria-pressed={consoleVisible} aria-label={consoleVisible ? t("hideConsole") : t("showConsole")} data-tooltip={consoleVisible ? t("hideConsole") : t("showConsole")} onclick={toggleConsole}><Icon name="terminal" size="15px" /></button>
      <button class="header-icon-link" aria-label={t("toggleTheme")} data-tooltip={t("toggleTheme")} onclick={toggleTheme}>
        <Icon name={preferences.theme === "dark" ? "moon" : "sun"} size="15px" />
      </button>
      <button class="header-icon-link" aria-label={t("preferences")} data-tooltip={t("preferences")} onclick={() => preferencesVisible = true}><Icon name="gear" size="15px" /></button>
      <span class="header-separator" aria-hidden="true"></span>
      <div class="master-output" class:boosted={volume > 1} style={`--master-volume-color: ${masterVolumeColor(volume)}`} aria-label={t("masterVolume")}>
        <button class="master-mute" class:muted={volume === 0} onclick={toggleMute} aria-label={volume > 0 ? t("mute") : t("unmute")} data-tooltip={volume > 0 ? t("mute") : t("unmute")}>
          {#if volume > 0}
            <Icon name="volume-high" size="15px" />
          {:else}
            <Icon name="volume-xmark" size="15px" />
          {/if}
        </button>
        <input aria-label={t("masterVolume")} type="range" min="0" max="2" step="0.01" value={volume} oninput={(event) => changeVolume(Number(event.currentTarget.value))} ondblclick={() => changeVolume(defaultMasterVolume)} />
        <output>{Math.round(volume * 100)}%</output>
        <div class="master-meter" class:limiting={limiterReduction > 0.001} data-tooltip={limiterReduction > 0.001 ? `Limiter −${(-20 * Math.log10(1 - limiterReduction)).toFixed(1)} dB` : preferences.loudnessNormalization && integratedLufs !== null ? `${t("loudnessNormalization")} ${20 * Math.log10(normalizationGain) >= 0 ? "+" : ""}${(20 * Math.log10(normalizationGain)).toFixed(1)} dB · ${integratedLufs.toFixed(1)} LUFS` : undefined} role="meter" aria-label={`${t("masterVolume")} ${Math.round(masterPeak * 100)}%`} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(masterPeak * 100)}>
          {#each masterMeterLevels as level}<i class:active={masterPeak * masterMeterLevels.length >= level}></i>{/each}
        </div>
      </div>
      <span class="header-separator" aria-hidden="true"></span>
      <button class="header-icon-link" aria-label={t("openGithub")} data-tooltip={t("openGithub")} onclick={() => openCommunityLink("github")}><Icon name="github" size="15px" /></button>
      <button class="header-icon-link donate" aria-label={t("supportProject")} data-tooltip={t("supportProject")} onclick={() => openCommunityLink("donate")}><Icon name="mug-hot" size="15px" /></button>
    </div>
  </header>

  <Toaster {toasts} durationMs={preferences.toastDurationSeconds * 1_000} closeLabel={t("closeNotification")} notificationsLabel={t("notifications")} dismiss={dismissToast} />

  <section class="workspace">
    <aside bind:this={playlistPanel} class="playlist panel" class:audio-drop-active={playlistDropActive}>
      {#if playlistDropActive}<div class="playlist-drop-overlay" aria-label={t("dropAudioHere")}><Icon name="cloud-arrow-down" size="22px" /><strong>{t("dropAudioHere")}</strong></div>{/if}
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
                {#if editingTrackId === track.id && editingTrackLocation === "playlist"}
                  <input class="track-title-input" bind:value={editingTrackTitle} aria-label={t("trackName")} autocorrect="off" use:focusOnMount onblur={() => commitTrackRename(track)} onkeydown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); else if (event.key === "Escape") { event.preventDefault(); cancelTrackRename(); } }} />
                {:else}
                  <button class="track-title" use:bounceTrackTitle onclick={() => startTrackRename(track, "playlist")} data-tooltip={t("renameTrack")}><span>{track.title}</span></button>
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
        <div class="current-track-title-row">
          {#if editingTrackId === currentTrack.id && editingTrackLocation === "header"}
            <input class="current-track-title-input" bind:value={editingTrackTitle} aria-label={t("trackName")} autocorrect="off" use:focusOnMount onblur={() => commitTrackRename(currentTrack!)} onkeydown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); else if (event.key === "Escape") { event.preventDefault(); cancelTrackRename(); } }} />
          {:else}
            <button class="current-track-title" onclick={() => startTrackRename(currentTrack!, "header")} data-tooltip={t("renameTrack")}>{currentTrack.title}</button>
          {/if}
        </div>
        <div class="panel-title waveform-panel-title">
          <h2>{t("waveform")}</h2>
          <div class="waveform-header-center">
            <div class="navigation-controls">
              <label class="navigation-mode"><span>{t("navigation")}</span><select value={preferences.navigationMode} onchange={(event) => changeNavigationMode(event.currentTarget.value as NavigationMode)} aria-keyshortcuts="N"><option value="time">{t("navigationTime")} · {preferences.navigationTimeSeconds} {t("secondsShort")}</option><option value="beat">{t("navigationBeat")}</option><option value="chord">{t("navigationChord")}</option></select></label>
              <button
                type="button"
                class="follow-playhead"
                class:active={followPlayhead}
                class:suspended={followPlayhead && waveformFollowSuspended}
                aria-label={t("followPlayhead")}
                aria-pressed={followPlayhead}
                data-tooltip={t("followPlayhead")}
                onclick={toggleWaveformFollow}
              ><Icon name="arrows-to-dot" size="13px" /></button>
            </div>
            {#if navigationAnalysisPending}<small>{navigationModeLabel(preferences.navigationMode)} · {t("navigationPending")}</small>{/if}
            <div class="load-states">{#if audioLoading}<span><i class="mini-spinner"></i>{t("loadingAudio")}</span>{/if}{#if waveformLoading}<span><i class="mini-spinner"></i>{t("waveformLoading")}</span>{/if}</div>
          </div>
          <span class="zoom-status">
            <span>{t("zoom")}: <strong>{waveformZoom.toFixed(1)}×</strong></span>
            <button type="button" class="zoom-preset fit-all" aria-label={t("fitEntireTrack")} data-tooltip={t("fitEntireTrack")} onclick={fitEntireWaveform}><Icon name="arrows-left-right-to-line" size="14px" /></button>
            <i class="zoom-separator" aria-hidden="true"></i>
            <button type="button" class="zoom-preset fit-thirty" aria-label={t("fitThirtySeconds")} data-tooltip={t("fitThirtySeconds")} onclick={fitThirtySecondWaveform}><Icon name="stopwatch" size="12px" /><small>30</small></button>
          </span>
        </div>
        {#if waveformChordBlocks.length}
          <div
            class="waveform-chord-lane"
            role="group"
            aria-label={t("chords")}
            onwheel={(event) => navigateWaveformWithWheel(event, false)}
            onpointerenter={enterDetailedWaveform}
            onpointerleave={leaveDetailedWaveform}
            onfocusin={() => waveformFocusWithin = true}
            onfocusout={leaveDetailedWaveformFocus}
          >
            {#each waveformChordBlocks as block}
              <button
                type="button"
                class:active={block.index === activeChordIndex}
                class:edited={block.chord.edited}
                class:no-chord={isNoChordLabel(block.chord.label)}
                style={`--chord-color:${chordColor(block.chord.label, block.chord.strength, chordColorMode)};left:${block.leftPercent}%;width:${block.widthPercent}%`}
                aria-label={`${chordDisplayLabel(block.chord.label)}, ${displayTime(block.chord.startSeconds)}, ${t("chordSeekHelp")}`}
                aria-current={block.index === activeChordIndex ? "true" : undefined}
                title={`${chordDisplayLabel(block.chord.label)} · ${displayTime(block.chord.startSeconds)}–${displayTime(block.chord.endSeconds)}`}
                onclick={() => { changeNavigationMode("chord"); seek(block.chord.startSeconds); }}
              >{chordDisplayLabel(block.chord.label)}</button>
            {/each}
          </div>
        {/if}
        <div
          class="wave detailed-wave"
          class:dragging={waveformDragPointerId !== null}
          role="application"
          aria-label={t("waveform")}
          data-tooltip={t("seekHelp")}
          onwheel={(event) => navigateWaveformWithWheel(event, false)}
          onpointerdown={startWaveformDrag}
          onpointerenter={enterDetailedWaveform}
          onpointerleave={leaveDetailedWaveform}
          onpointermove={dragWaveform}
          onpointerup={finishWaveformDrag}
          onpointercancel={cancelWaveformDrag}
          onfocusin={() => waveformFocusWithin = true}
          onfocusout={leaveDetailedWaveformFocus}
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
            {#if loopA !== null}
              <button class="loop-handle a" style={`left:${(loopA / durationSeconds - waveformStart) * waveformZoom * 100}%`} aria-label={`${t("moveStart")}. ${t("doubleClickResetA")}`} data-tooltip={`${t("moveStart")} · ${t("doubleClickResetA")}`} onpointerdown={(event) => startLoopDrag(event, "a", true)} onpointermove={(event) => moveLoopDrag(event, true)} onpointerup={finishLoopDrag} onpointercancel={finishLoopDrag} ondblclick={(event) => resetLoopBoundary(event, "a")}>A</button>
              {#if loopB !== null}
                <i
                  class="loop-region"
                  class:disabled={!loopEnabled}
                  aria-hidden="true"
                  style={`left:${(loopA / durationSeconds - waveformStart) * waveformZoom * 100}%;width:${(loopB - loopA) / durationSeconds * waveformZoom * 100}%`}
                ></i>
                <button class="loop-handle b" style={`left:${(loopB / durationSeconds - waveformStart) * waveformZoom * 100}%`} aria-label={`${t("moveEnd")}. ${t("doubleClickResetB")}`} data-tooltip={`${t("moveEnd")} · ${t("doubleClickResetB")}`} onpointerdown={(event) => startLoopDrag(event, "b", true)} onpointermove={(event) => moveLoopDrag(event, true)} onpointerup={finishLoopDrag} onpointercancel={finishLoopDrag} ondblclick={(event) => resetLoopBoundary(event, "b")}>B</button>
              {/if}
            {/if}
            {#if playheadPercent >= 0 && playheadPercent <= 100}<i class="playhead" style={`left:${playheadPercent}%`}></i>{/if}
          {/if}
        </div>
        <div class="waveform-help">{t("waveformHelp")} · {t("waveformNavigationHelp")}</div>
        <div class="overview-wave" role="application" aria-label={t("overviewHelp")} data-tooltip={t("overviewHelp")} onwheel={(event) => navigateWaveformWithWheel(event, true)} onpointerdown={seekFromOverview}>
          {#if waveformLoading}<div class="overview-skeleton"><svg viewBox={`0 0 ${loadingWave.length} 60`} preserveAspectRatio="none" aria-hidden="true">{#each loadingWave as height, index}<line x1={index} x2={index} y1={30 - height * 27} y2={30 + height * 27}></line>{/each}</svg><i></i></div>
          {:else if overviewPeaks.length > 0}
            <svg viewBox={`0 0 ${overviewPeaks.length} 60`} preserveAspectRatio="none" aria-hidden="true">
              {#each overviewPeaks as peak, index}
                <line x1={index} x2={index} y1={30 - peak.max * 28} y2={30 - peak.min * 28} />
              {/each}
            </svg>
            <button type="button" class="viewport" class:dragging={viewportDrag?.mode === "move"} aria-label={t("moveViewport")} style={`left:${waveformStart * 100}%;width:${100 / waveformZoom}%`} onpointerdown={(event) => startViewportDrag(event, "move")} onpointermove={moveViewportDrag} onpointerup={finishViewportDrag} onpointercancel={cancelViewportDrag}></button>
            <button type="button" class="viewport-handle start" aria-label={t("resizeViewportStart")} data-tooltip={t("resizeViewportStart")} style={`left:${waveformStart * 100}%`} onpointerdown={(event) => startViewportDrag(event, "start")} onpointermove={moveViewportDrag} onpointerup={finishViewportDrag} onpointercancel={cancelViewportDrag}></button>
            <button type="button" class="viewport-handle end" aria-label={t("resizeViewportEnd")} data-tooltip={t("resizeViewportEnd")} style={`left:${(waveformStart + 1 / waveformZoom) * 100}%`} onpointerdown={(event) => startViewportDrag(event, "end")} onpointermove={moveViewportDrag} onpointerup={finishViewportDrag} onpointercancel={cancelViewportDrag}></button>
            {#if loopA !== null}
              <button class="loop-handle overview a" style={`left:${loopA / durationSeconds * 100}%`} aria-label={`${t("moveStart")}. ${t("doubleClickResetA")}`} data-tooltip={`${t("moveStart")} · ${t("doubleClickResetA")}`} onpointerdown={(event) => startLoopDrag(event, "a", false)} onpointermove={(event) => moveLoopDrag(event, false)} onpointerup={finishLoopDrag} onpointercancel={finishLoopDrag} ondblclick={(event) => resetLoopBoundary(event, "a")}>A</button>
              {#if loopB !== null}
                <i class="loop-region overview" class:disabled={!loopEnabled} aria-hidden="true" style={`left:${loopA / durationSeconds * 100}%;width:${(loopB - loopA) / durationSeconds * 100}%`}></i>
                <button class="loop-handle overview b" style={`left:${loopB / durationSeconds * 100}%`} aria-label={`${t("moveEnd")}. ${t("doubleClickResetB")}`} data-tooltip={`${t("moveEnd")} · ${t("doubleClickResetB")}`} onpointerdown={(event) => startLoopDrag(event, "b", false)} onpointermove={(event) => moveLoopDrag(event, false)} onpointerup={finishLoopDrag} onpointercancel={finishLoopDrag} ondblclick={(event) => resetLoopBoundary(event, "b")}>B</button>
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
          <span>A {loopA === null ? "—" : displayTime(loopA)}</span>
          <strong class="playback-position" aria-label={t("playbackPosition")}>{displayTime(currentSeconds)}</strong>
          <span>B {loopB === null ? "—" : displayTime(loopB)}</span>
        </div>
        <div class="waveform-transport-row">
          <span aria-hidden="true"></span>
          <div class="transport-center">
            <button class="seek-button" disabled={audioLoading} aria-label={navigationDirectionLabel(-1)} data-tooltip={`${navigationDirectionLabel(-1)} · ${t("holdToRepeat")}`} onpointerdown={(event) => startJumpHold(event, -1)} onpointerup={finishJumpHold} onpointercancel={finishJumpHold} onlostpointercapture={stopJumpHold} onclick={(event) => keyboardJump(event, -1)}><Icon name="backward" size="14px" /></button>
            <button disabled={audioLoading} class="round" aria-label={t("previous")} data-tooltip={t("previous")} onclick={() => moveTrack(-1)}><Icon name="backward-step" size="15px" /></button>
            <button disabled={audioLoading || stemPlaybackLocked} class="play" class:loading={audioLoading || stemPlaybackLocked} aria-label={audioLoading ? t("loadingAudio") : stemPlaybackLocked ? t("separatingStems") : isPlaying ? t("pause") : t("play")} data-tooltip={audioLoading ? t("loadingAudio") : stemPlaybackLocked ? t("separatingStems") : isPlaying ? t("pause") : t("play")} onclick={togglePlayback}>{#if audioLoading || stemPlaybackLocked}<i class="button-spinner"></i>{:else}<Icon name={isPlaying ? "pause" : "play"} size="15px" />{/if}</button>
            <button disabled={audioLoading} class="round" aria-label={t("next")} data-tooltip={t("next")} onclick={() => moveTrack(1)}><Icon name="forward-step" size="15px" /></button>
            <button class="seek-button" disabled={audioLoading} aria-label={navigationDirectionLabel(1)} data-tooltip={`${navigationDirectionLabel(1)} · ${t("holdToRepeat")}`} onpointerdown={(event) => startJumpHold(event, 1)} onpointerup={finishJumpHold} onpointercancel={finishJumpHold} onlostpointercapture={stopJumpHold} onclick={(event) => keyboardJump(event, 1)}><Icon name="forward" size="14px" /></button>
          </div>
          <div class="transport-right">
            <div class="end-behavior" role="group" aria-label={t("endBehavior")}>
              <button class:active={endBehavior === "restart"} aria-pressed={endBehavior === "restart"} aria-label={t("restartAtEnd")} data-tooltip={t("restartAtEnd")} onclick={() => changeEndBehavior("restart")}><Icon name="rotate-left" size="13px" /></button>
              <button class:active={endBehavior === "advance"} aria-pressed={endBehavior === "advance"} aria-label={t("advanceAtEnd")} data-tooltip={t("advanceAtEnd")} onclick={() => changeEndBehavior("advance")}><Icon name="forward-step" size="13px" /></button>
              <button class:active={endBehavior === "stop"} aria-pressed={endBehavior === "stop"} aria-label={t("stopAtEnd")} data-tooltip={t("stopAtEnd")} onclick={() => changeEndBehavior("stop")}><Icon name="stop" size="13px" /></button>
            </div>
            <label class="metronome-volume transport-volume" data-tooltip={t("musicVolumeHelp")}><Icon name="volume-high" size="11px" /><input aria-label={t("musicVolume")} type="range" min="0" max="1" step="0.01" value={musicVolume} oninput={(event) => changeMusicVolume(Number(event.currentTarget.value))} ondblclick={() => changeMusicVolume(defaultMusicVolume)} /></label>
          </div>
        </div>
      </div>

      <div class="practice panel">
        <div class="control-block loop-controls">
          <span class="control-block-label">{t("loop")}</span>
          <div class="control-group loop-actions">
            <button class="loop-action-a" onclick={setLoopA} ondblclick={(event) => resetLoopBoundary(event, "a")} aria-label={`${t("moveA")}. ${t("doubleClickResetA")}`} data-tooltip={`${t("moveA")} · ${t("doubleClickResetA")}`}>A</button>
            <button class="loop-action-b" onclick={setLoopB} ondblclick={(event) => resetLoopBoundary(event, "b")} aria-label={`${t("moveB")}. ${t("doubleClickResetB")}`} data-tooltip={`${t("moveB")} · ${t("doubleClickResetB")}`}>B</button>
            <i class="control-separator" aria-hidden="true"></i>
            <button class:active={loopEnabled} onclick={toggleLoop} aria-pressed={loopEnabled} aria-label={t("toggleLoop")} data-tooltip={t("toggleLoop")}><Icon name="rotate-left" size="11px" /></button>
            <button class:active={loopSnapEnabled} disabled={!loopSnapAvailable} onclick={toggleLoopSnap} aria-pressed={loopSnapEnabled} aria-label={t("loopSnap")} data-tooltip={preferences.navigationMode === "chord" ? t("loopSnapChordHelp") : t("loopSnapBeatHelp")}><Icon name="magnet" size="12px" /></button>
          </div>
        </div>
        <div class="practice-center-controls">
          <div class="control-block trainer-control">
            <span class="control-block-label">{t("training")}</span>
            <div class="trainer-actions">
              <button class="trainer-toggle" class:active={trainerEnabled} aria-pressed={trainerEnabled} aria-label={t("training")} data-tooltip={t("trainerHelp")} onclick={toggleLoopTrainer}><Icon name="stairs" size="15px" /><span>{trainerLoopCount}/{trainerRepetitions}</span></button>
              <button aria-label={t("trainingSettings")} data-tooltip={t("trainingSettings")} onclick={openTrainingSettings}><Icon name="sliders" size="13px" /></button>
            </div>
          </div>
          <NumericControl label={t("tempo")} value={playbackRate} defaultValue={1} minimum={0.5} maximum={2} step={0.01} buttonStep={0.05} shiftButtonStep={0.01} display={(value) => `${Math.round(value * 100)}%`} onChange={setPlaybackRate} tooltip={t("numericHelp")} />
          <NumericControl label={t("pitch")} value={pitchSemitones} defaultValue={0} minimum={-12} maximum={12} step={0.01} buttonStep={1} shiftButtonStep={0.01} display={formatPitch} onChange={setPitch} tooltip={t("pitchFineHelp")} />
        </div>
        <div class="practice-right-controls">
          <div class="control-block bpm-indicator" aria-live="polite" data-tooltip={t("bpmEstimateHelp")}>
            <span class="control-block-label">{t("gridTempo")}</span>
            {#if tempoLoading}
              <span class="bpm-indicator-value"><i class="mini-spinner"></i><span class="sr-only">{t("bpmAnalyzing")}</span></span>
            {:else}
              <strong>{detectedBpm === null ? "—" : detectedBpm.toFixed(1)}</strong>
            {/if}
          </div>
          <div class="control-block metronome-block">
            <span class="control-block-label">{t("metronome")}</span>
            <div class="metronome-control">
              <button class:active={metronomeEnabled} class:beating={metronomeBeating} disabled={!chordAnalysis?.beats.length} aria-pressed={metronomeEnabled} aria-label={t("metronome")} data-tooltip={t("metronomeHelp")} onclick={toggleMetronome}><Icon name="metronome" size="14px" /></button>
              <select class="metronome-sound" aria-label={t("metronomeSound")} data-tooltip={t("metronomeSound")} value={metronomeSound} onchange={(event) => changeMetronomeSound(event.currentTarget.value)}><option value="electronic">{t("metronomeElectronic")}</option><option value="woodblock">{t("metronomeWoodblock")}</option><option value="metallic">{t("metronomeMetallic")}</option></select>
              <label class="metronome-volume" data-tooltip={t("metronomeVolume")}><Icon name="volume-high" size="11px" /><input aria-label={t("metronomeVolume")} type="range" min="0" max="1" step="0.01" value={metronomeVolume} oninput={(event) => changeMetronomeVolume(Number(event.currentTarget.value))} ondblclick={() => changeMetronomeVolume(defaultMetronomeVolume)} /></label>
            </div>
          </div>
        </div>
        <div class="transport-trainer-progress"><i style={`width:${Math.max(0, Math.min(100, trainerLoopCount / trainerRepetitions * 100))}%`}></i></div>
      </div>

      <div class="analysis-grid">
        <div class="panel stem-panel" class:stem-bypassed={stems.state === "ready" && !stems.enabled}>
          <div class="panel-title stem-panel-title">
            <label class="stem-switch" data-tooltip={t("stemSwitchHelp")}>
              <input type="checkbox" role="switch" checked={stems.enabled} disabled={!currentTrack || stems.state === "failed"} onchange={(event) => void toggleStemMode(event)} />
              <i aria-hidden="true"><b></b></i><strong>STEMS</strong>
            </label>
            <div class="stem-header-actions">
              <div class="stem-heading-status">{#if stems.computeBackend}<span class="stem-backend">{stems.computeBackend}</span>{/if}<span>{stems.state === "ready" ? stems.enabled ? t("stemsReady") : t("stemsBypassed") : stems.state === "failed" ? t("stemFailed") : t("idle")}</span></div>
              <button class="stem-export-button" disabled={stems.state !== "ready" || stems.trackId !== currentTrack?.id || busy} aria-label={t("exportStems")} data-tooltip={stems.state === "ready" ? t("exportStems") : t("exportStemsUnavailable")} onclick={openStemExport}><Icon name="cloud-arrow-down" size="13px" /></button>
            </div>
          </div>
          {#if stems.state === "disabled"}
            <div class="stem-empty"><button class="primary" data-tooltip={t("stemHelp")} disabled={!currentTrack} onclick={() => void enableStems()}>{t("enableStems")}</button><small>HTDemucs 6s · 6 stems · MLX</small></div>
          {:else if stems.state === "separating"}
            <div class="stem-progress"><div class="stem-progress-label"><span class="mini-spinner"></span><span>{stems.stage === "checkingCache" ? t("loadingAvailableStems") : stems.stage === "loadingModel" ? t("loadingStemModel") : stems.stage === "loadingAudio" ? t("loadingStemAudio") : stems.stage === "writingStems" || stems.stage === "validatingStems" || stems.stage === "cachingStems" ? t("writingStems") : t("separatingStems")}</span><b>{Math.round(stems.progress * 100)}%</b></div><i><b style={`width:${Math.max(1, stems.progress * 100)}%`}></b></i><button onclick={disableStems}>{t("disableStems")}</button></div>
          {:else if stems.state === "failed"}
            <div class="stem-empty"><p>{stems.error ?? t("stemFailed")}</p><button onclick={() => void enableStems()}>{t("enableStems")}</button></div>
          {:else}
            <div class="stem-mixer" aria-label={t("stemMixer")}>
              {#each stemDisplayOrder as index, position}
                <section class="stem-strip" style={`--stem-color:${stemColors[index]}`}>
                  <div class="stem-pan">
                    <span>{t("pan")}</span>
                    <div class="pan-knob" style={`--pan-angle:${stemMix[index].pan * 135}deg`}>
                      <i aria-hidden="true"></i>
                      <input disabled={!stems.enabled} aria-label={`${stemDisplayName(index)} ${t("pan")}`} aria-valuetext={formatPan(stemMix[index].pan)} type="range" min="-1" max="1" step="0.01" value={stemMix[index].pan} oninput={(event) => updateStem(index, { pan: Number(event.currentTarget.value) })} ondblclick={() => updateStem(index, { pan: 0 })} />
                    </div>
                    <output>{formatPan(stemMix[index].pan)}</output>
                  </div>
                  <div class="stem-level-section">
                    <div class="stem-fader">
                      <output>{formatStemGain(stemMix[index].gain)}</output>
                      <input disabled={!stems.enabled} aria-label={`${stemDisplayName(index)} ${t("volume")}`} aria-valuetext={formatStemGain(stemMix[index].gain)} type="range" min="0" max="2" step="0.01" value={stemMix[index].gain} oninput={(event) => updateStem(index, { gain: Number(event.currentTarget.value) })} ondblclick={() => updateStem(index, { gain: 1 })} />
                    </div>
                    <div class="stem-vu" role="meter" aria-label={`${stemDisplayName(index)} ${t("level")}`} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(stemMeterLevel(stemPeaks[index]) * 100)}>
                      {#each stemMeterLevels as level}<i class:active={stemMeterLevel(stemPeaks[index]) * stemMeterLevels.length >= level} class:hot={level > 11}></i>{/each}
                    </div>
                  </div>
                  <div class="stem-buttons">
                    <button disabled={!stems.enabled} class:muted={stemMix[index].muted} aria-pressed={stemMix[index].muted} aria-label={`${t("mute")} ${stemDisplayName(index)}`} onclick={() => updateStem(index, { muted: !stemMix[index].muted })}>M</button>
                    <button disabled={!stems.enabled} class:soloed={stemMix[index].soloed} aria-pressed={stemMix[index].soloed} aria-label={`${t("solo")} ${stemDisplayName(index)}`} onclick={() => updateStem(index, { soloed: !stemMix[index].soloed })}>S</button>
                  </div>
                  <label class="stem-channel-label"><span>{String(position + 1).padStart(2, "0")} ·</span><input disabled={!stems.enabled} aria-label={t("stemName")} title={t("renameStem")} maxlength="40" autocorrect="off" value={stemDisplayName(index)} onchange={(event) => { renameStem(index, event.currentTarget.value); event.currentTarget.value = stemDisplayName(index); }} onkeydown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} /></label>
                </section>
              {/each}
            </div>
          {/if}
        </div>
        <div class="analysis-visuals">
          <div class="spectrum panel">
            <div class="panel-title"><h2>{t("spectrum")}</h2><span>30 Hz — 20 kHz · FFT 2048</span></div>
            <div class="spectrum-bars" aria-label={t("spectrum")}>
              {#each spectrumBands as magnitude, index}<i style={`height:${Math.max(1, magnitude * 100)}%;--band:${index}`}></i>{/each}
            </div>
            <div class="spectrum-scale"><span>30</span><span>100</span><span>1k</span><span>10k</span><span>20k Hz</span></div>
          </div>
          <div class="stereo-meter panel">
            <div class="panel-title"><h2>{t("stereoMeter")}</h2></div>
            <div class="stereo-meter-channels">
              <div class="stereo-channel"><span>L</span><div class="stereo-track" role="meter" aria-label={`${t("leftChannel")} ${Math.round(masterPeakLeft * 100)}%`} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(masterPeakLeft * 100)}><i style={`width:${Math.min(100, Math.max(0, masterPeakLeft * 100))}%`}></i></div><output>{Math.round(masterPeakLeft * 100)}%</output></div>
              <div class="stereo-channel"><span>R</span><div class="stereo-track" role="meter" aria-label={`${t("rightChannel")} ${Math.round(masterPeakRight * 100)}%`} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(masterPeakRight * 100)}><i style={`width:${Math.min(100, Math.max(0, masterPeakRight * 100))}%`}></i></div><output>{Math.round(masterPeakRight * 100)}%</output></div>
            </div>
          </div>
        </div>
      </div>

      <div class="harmony-grid">
        <div class="panel chord-panel">
          <div class="panel-title chord-panel-title">
            <div class="chord-title-row">
              <div class="chord-title-label">
                <h2>{t("chords")}</h2>
                {#if chordsLoading}<span class="chord-title-loader" aria-label={t("analyzingChords")} data-tooltip={t("analyzingChords")}><i class="mini-spinner"></i></span>{/if}
              </div>
              <div class="chord-panel-actions">
                <button disabled={!timelineChords.length} class:active={chordEditMode} aria-pressed={chordEditMode} aria-keyshortcuts="E" aria-label={t("chordEditMode")} data-tooltip={t("chordEditModeHelp")} onclick={toggleChordEditMode}><Icon name="pen" size="13px" /></button>
                <button disabled={!chordEdits.length} aria-label={t("resetChordEdits")} data-tooltip={t("resetChordEdits")} onclick={resetChordEdits}><Icon name="rotate-left" size="13px" /></button>
                <i class="chord-action-separator" aria-hidden="true"></i>
                <button class:active={chordView === "repertoire"} aria-pressed={chordView === "repertoire"} aria-label={t("chordRepertoire")} data-tooltip={t("chordRepertoireHelp")} onclick={toggleChordRepertoire}><Icon name="book-open" size="13px" /></button>
                <button class:active={chordAutoScrollEnabled} aria-pressed={chordAutoScrollEnabled} aria-label={t("chordAutoScroll")} data-tooltip={t("chordAutoScrollHelp")} onclick={toggleChordAutoScroll}><Icon name="arrow-down" size="13px" /></button>
                <i class="chord-action-separator" aria-hidden="true"></i>
                <button class:active={chordSettingsVisible} aria-expanded={chordSettingsVisible} aria-controls="chord-settings-panel" aria-label={t("chordSettings")} data-tooltip={t("chordSettings")} onclick={() => chordSettingsVisible = !chordSettingsVisible}><Icon name="sliders" size="13px" /></button>
              </div>
            </div>
            {#if chordSettingsVisible}
              <div class="chord-settings-panel" id="chord-settings-panel">
                <label><span>{t("chordAnalysisType")}</span><select aria-label={t("chordAnalysisType")} data-tooltip={t("chordModeHelp")} value={chordMode} onchange={(event) => changeChordMode(event.currentTarget.value as ChordMode)}>
                  <option value="essential">{t("chordEssential")}</option>
                  <option value="standard">{t("chordStandard")}</option>
                  <option value="complete">{t("chordComplete")}</option>
                </select></label>
                <label><span>{t("chordConfidence")}</span><select aria-label={t("chordConfidence")} data-tooltip={t("chordConfidenceHelp")} bind:value={chordMinimumStrength}>
                  <option value={0}>{t("chordConfidenceAll")}</option>
                  <option value={0.25}>≥ 25%</option><option value={0.5}>≥ 50%</option><option value={0.7}>≥ 70%</option><option value={0.85}>≥ 85%</option>
                </select></label>
                <label><span>{t("chordAccidentals")}</span><select aria-label={t("chordAccidentals")} data-tooltip={t("chordAccidentalsHelp")} value={chordAccidentalMode} onchange={(event) => chordAccidentalMode = event.currentTarget.value as ChordAccidentalMode}>
                  <option value="sharp">♯ · {t("chordSharps")}</option>
                  <option value="flat">♭ · {t("chordFlats")}</option>
                </select></label>
                <label><span>{t("chordColors")}</span><select aria-label={t("chordColors")} data-tooltip={t("chordColorsHelp")} value={chordColorMode} onchange={(event) => chordColorMode = event.currentTarget.value as ChordColorMode}>
                  <option value="root">{t("chordColorRoot")}</option>
                  <option value="score">{t("chordColorScore")}</option>
                </select></label>
              </div>
            {/if}
          </div>
          {#if chordAnalysisError}
            <p class="chord-state failed">{t("chordAnalysisFailed")}</p>
          {:else if !chordAnalysis || !chordAnalysis.modes.standard.length}
            <p class="chord-state">{chordsLoading ? t("analyzingChords") : t("noChords")}</p>
          {:else}
            {#if chordView === "timeline"}
              <div
                class="chords chord-timeline"
                role="region"
                aria-label={t("chords")}
                bind:this={chordList}
                onpointerenter={(event) => { if (event.pointerType !== "touch") chordPointerInside = true; }}
                onpointerleave={() => resumeChordFollow()}
                onpointerdown={(event) => { if (event.target === event.currentTarget) chordProgrammaticScroll = false; }}
                onwheel={suspendChordFollow}
                onscroll={handleChordScroll}
                onscrollend={() => chordProgrammaticScroll = false}
                onfocusin={() => { chordFocusWithin = true; chordScrollSuspended = true; }}
                onfocusout={leaveChordGridFocus}
              >
                {#each timelineChords as chord, chordIndex}
                  {@const editKey = chordEditKey(chordMode, chord)}
                  {#if editingChordKey === editKey}
                    <div
                      class="chord-card chord-editor"
                      class:active={chordIndex === activeChordIndex}
                      class:no-chord={isNoChordLabel(chord.label)}
                      style={`--chord-color:${chordColor(chord.label, chord.strength, chordColorMode)}`}
                      data-chord-index={chordIndex}
                      bind:this={chordEditContainer}
                      use:chordEditInteractions
                    >
                      <input
                        use:focusOnMount
                        bind:value={chordEditValue}
                        maxlength="96"
                        spellcheck="false"
                        autocorrect="off"
                        autocomplete="off"
                        aria-label={`${t("chords")}: ${chordDisplayLabel(chord.label)}`}
                        aria-invalid={chordEditInvalid}
                        oninput={refreshChordEditSuggestions}
                        onkeydown={handleChordEditKeydown}
                      />
                      {#if chordEditSuggestionOptions.length}
                        <select
                          class="chord-edit-options"
                          size={Math.min(7, chordEditSuggestionOptions.length)}
                          bind:this={chordEditSuggestionSelect}
                          use:chordEditOptionsPortal
                          value={chordEditValue}
                          aria-label={`${t("chords")}: ${chordDisplayLabel(chord.label)}`}
                          onchange={chooseChordEditSuggestion}
                          onclick={validateChordEditSuggestion}
                          onkeydown={handleChordEditKeydown}
                        >
                          {#each chordEditSuggestionOptions as suggestion}<option value={suggestion}>{isNoChordLabel(suggestion) ? `— (${t("noChord")})` : suggestion}</option>{/each}
                        </select>
                      {/if}
                      <small>{displayTime(chord.startSeconds)}</small>
                    </div>
                  {:else}
                    <button
                      class:active={chordIndex === activeChordIndex}
                      class:edited={chord.edited}
                      class:no-chord={isNoChordLabel(chord.label)}
                      style={`--chord-color:${chordColor(chord.label, chord.strength, chordColorMode)}`}
                      data-chord-index={chordIndex}
                      aria-pressed={selectedChordKey === editKey}
                      aria-keyshortcuts={chordEditMode ? "Enter" : undefined}
                      aria-label={`${chordDisplayLabel(chord.label)}, ${displayTime(chord.startSeconds)}, ${t(chordEditMode ? "showChordOnKeyboard" : "chordSeekHelp")}`}
                      data-tooltip={t(chordEditMode ? "showChordOnKeyboard" : "chordSeekHelp")}
                      title={`${displayTime(chord.startSeconds)}–${displayTime(chord.endSeconds)} · ${Math.round(chord.strength * 100)}%`}
                      onclick={(event) => selectChordFromButton(event, chord)}
                      onfocus={(event) => focusChordFromButton(event, chord)}
                      onpointerdown={prepareChordMiddleClick}
                      onauxclick={(event) => beginChordEditFromMiddleClick(event, chord)}
                      onkeydown={(event) => handleChordKeydown(event, chord)}
                    ><b>{chordDisplayLabel(chord.label)}</b><small>{displayTime(chord.startSeconds)}</small></button>
                  {/if}
                {/each}
              </div>
            {:else}
              <div class="chords chord-repertoire" aria-label={t("chordRepertoire")}>
                {#each repertoireLabels as label}
                  <button
                    class:active={activeChord?.label === label}
                    class:selected={repertoireKeyboardLabel === label && activeChord?.label !== label}
                    aria-current={activeChord?.label === label ? "true" : undefined}
                    style={`--chord-color:${chordColor(label, Math.max(...displayedChords.filter((chord) => chord.label === label).map((chord) => chord.strength)), chordColorMode)}`}
                    aria-label={`${label}, ${t("showChordOnKeyboard")}`}
                    data-tooltip={t("showChordOnKeyboard")}
                    onclick={() => { repertoireKeyboardLabel = label; changeNavigationMode("chord"); }}
                  ><b>{label}</b></button>
                {/each}
              </div>
            {/if}
          {/if}
        </div>
        <div class="panel keyboard-panel harmony-view-panel">
          <div class="panel-title harmony-view-title">
            <div class="harmony-view-tabs" role="group" aria-label={t("instrumentView")} aria-keyshortcuts="I">
              <button class:active={harmonyView === "piano"} aria-pressed={harmonyView === "piano"} onclick={() => harmonyView = "piano"}><Icon name="keyboard" size=".72rem" />{t("piano")}</button>
              <button class:active={harmonyView === "guitar"} aria-pressed={harmonyView === "guitar"} onclick={() => harmonyView = "guitar"}><Icon name="guitar" size=".72rem" />{t("guitar")}</button>
              <button class:active={harmonyView === "ukulele"} aria-pressed={harmonyView === "ukulele"} onclick={() => harmonyView = "ukulele"}><Icon name="guitar" size=".62rem" />{t("ukulele")}</button>
            </div>
            <strong
              class="keyboard-current-chord"
              class:no-chord={isNoChordLabel(activeChordLabel)}
              style={`--chord-color:${activeInstrumentColor}`}
            >{chordDisplayLabel(activeChordLabel)}</strong>
            <div class="harmony-label-mode" role="group" aria-label={t("instrumentLabels")}>
              <button class:active={harmonyLabelMode === "notes"} aria-pressed={harmonyLabelMode === "notes"} onclick={() => harmonyLabelMode = "notes"}>{t("noteNames")}</button>
              <button class:active={harmonyLabelMode === "degrees"} aria-pressed={harmonyLabelMode === "degrees"} onclick={() => harmonyLabelMode = "degrees"}>{t("chordDegrees")}</button>
            </div>
          </div>
          {#if harmonyView === "piano"}
            <PianoChord label={activeHarmonyLabel} accessibleLabel={t("chordKeyboard")} accidentals={chordAccidentalMode} positionLabel={t("voicingPosition")} unavailableLabel={t("noVoicing")} emptyLabel={t("noChords")} labelMode={harmonyLabelMode} chordColor={activeInstrumentColor} />
          {:else}
            <FretboardChord
              label={activeHarmonyLabel}
              instrument={harmonyView}
              accidentals={chordAccidentalMode}
              accessibleLabel={t(harmonyView)}
              positionLabel={t("voicingPosition")}
              exactLabel={t("exactVoicing")}
              adaptedLabel={t("adaptedVoicing")}
              unavailableLabel={t("noVoicing")}
              emptyLabel={t("noChords")}
              omittedLabel={t("omittedNotes")}
              bassOmittedLabel={t("bassOmitted")}
              labelMode={harmonyLabelMode}
              chordColor={activeInstrumentColor}
            />
          {/if}
        </div>
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

  {#if helpVisible}
    <footer class="app-help-footer">
      <span>{busy ? t("working") : t("ready")}</span>
      <div class="help-strip" aria-live="polite"><Icon name="lightbulb" size="12px" /><span>{helpMessage || t("helpHover")}</span></div>
      <button class="link" onclick={showDiagnostics}>{t("diagnostics")}</button>
    </footer>
  {/if}

  {#if consoleVisible}
    <section class="app-console" aria-label={t("applicationConsole")}>
      <header>
        <div class="console-title"><strong>{t("applicationConsole")}</strong><span>{filteredAppLogs.length}/{appLogs.length} {t("logEntries")}</span></div>
        <div class="console-filters">
          <label><span>{t("minimumLogLevel")}</span><select class={`level-${consoleMinimumLevel}`} bind:value={consoleMinimumLevel} aria-label={t("minimumLogLevel")}><option class="level-debug" value="debug">DEBUG</option><option class="level-info" value="info">INFO</option><option class="level-warn" value="warn">WARN</option><option class="level-error" value="error">ERROR</option></select></label>
          <label><span>{t("logFamily")}</span><select value={consoleOrigin ?? "all"} onchange={(event) => consoleOrigin = event.currentTarget.value === "all" ? null : event.currentTarget.value} aria-label={t("logFamily")}><option value="all">{t("allLogFamilies")}</option>{#each consoleOrigins as origin}<option value={origin}>{logOriginLabel(origin)}</option>{/each}</select></label>
          <button aria-label={t("hideConsole")} data-tooltip={t("hideConsole")} onclick={toggleConsole}><Icon name="xmark" size="12px" /></button>
        </div>
      </header>
      <div class="console-output">
        {#if appLogs.length === 0}<p class="console-empty">{t("noLogs")}</p>{:else if filteredAppLogs.length === 0}<p class="console-empty">{t("noMatchingLogs")}</p>{/if}
        {#each filteredAppLogs as entry}
          <div class={`console-line ${entry.level}`}><time>{new Date(entry.timestampMs).toLocaleTimeString(language, { hour12: false })}</time><b>{logOriginLabel(entry.origin)}</b><em>{entry.level.toUpperCase()}</em><pre>{entry.message}</pre></div>
        {/each}
      </div>
    </section>
  {/if}

  {#if closePromptVisible}
    <Modal title={t("saveTemporaryTitle")} closeLabel={t("close")} close={() => closePromptVisible = false}>
      <p>{t("saveTemporaryPrompt")}</p>
      <div class="modal-actions">
        <button onclick={() => closePromptVisible = false}>{t("cancel")}</button>
        <button onclick={closeWithoutSavingElsewhere}>{t("quitWithoutSaving")}</button>
        <button class="primary" disabled={busy} onclick={() => void saveTemporaryAndClose()}>{t("saveAndQuit")}</button>
      </div>
    </Modal>
  {/if}

  {#if diagnosticInfo}
    <Modal title={t("diagnostics")} closeLabel={t("close")} close={() => diagnosticInfo = null}><dl><dt>{t("version")}</dt><dd>{diagnosticInfo.appVersion}</dd><dt>OS</dt><dd>{diagnosticInfo.os}</dd><dt>{t("architecture")}</dt><dd>{diagnosticInfo.architecture}</dd><dt>{t("logging")}</dt><dd>{diagnosticInfo.rustLog}</dd></dl><button onclick={() => diagnosticInfo = null}>{t("close")}</button></Modal>
  {/if}

  {#if stemExportVisible}
    <Modal title={t("exportStems")} closeLabel={t("close")} close={() => stemExportVisible = false}>
      <p class="stem-export-description">{t("exportStemsHelp")}</p>
      <div class="stem-export-formats" role="radiogroup" aria-label={t("stemExportFormat")}>
        <button class:active={stemExportFormat === "wav"} role="radio" aria-checked={stemExportFormat === "wav"} onclick={() => stemExportFormat = "wav"}><strong>WAV</strong><small>{t("stemExportWavHelp")}</small></button>
        <button class:active={stemExportFormat === "mp3"} role="radio" aria-checked={stemExportFormat === "mp3"} onclick={() => stemExportFormat = "mp3"}><strong>MP3</strong><small>{t("stemExportMp3Help")}</small></button>
      </div>
      <div class="modal-actions"><button onclick={() => stemExportVisible = false}>{t("close")}</button><button class="primary" disabled={busy || stems.state !== "ready"} onclick={() => void exportCurrentStems()}>{busy ? t("working") : t("export")}</button></div>
    </Modal>
  {/if}

  {#if trainingSettingsVisible}
    <Modal title={t("trainingSettings")} closeLabel={t("close")} close={() => trainingSettingsVisible = false}>
      <div class="training-settings-form" onchange={saveTrainingSettings}>
        <label><span>{t("startSpeed")}</span><span><input type="number" min="50" max="199" step="1" value={Math.round(trainingDraft.startRate * 100)} oninput={(event) => trainingDraft = { ...trainingDraft, startRate: Number(event.currentTarget.value) / 100 }} /><b>%</b></span></label>
        <label><span>{t("endSpeed")}</span><span><input type="number" min="51" max="200" step="1" value={Math.round(trainingDraft.targetRate * 100)} oninput={(event) => trainingDraft = { ...trainingDraft, targetRate: Number(event.currentTarget.value) / 100 }} /><b>%</b></span></label>
        <label><span>{t("stepSize")}</span><span><input type="number" min="1" max="25" step="1" value={Math.round(trainingDraft.increment * 100)} oninput={(event) => trainingDraft = { ...trainingDraft, increment: Number(event.currentTarget.value) / 100 }} /><b>%</b></span></label>
        <label><span>{t("loopsPerStep")}</span><span><input type="number" min="1" max="99" step="1" bind:value={trainingDraft.repetitions} /></span></label>
      </div>
      <div class="modal-actions"><button onclick={resetTrainingDraft}>{t("resetTrainingDefaults")}</button></div>
    </Modal>
  {/if}

  {#if preferencesVisible}
    <Modal title={t("preferences")} closeLabel={t("close")} wide close={() => preferencesVisible = false}>
      <div class="preferences-grid" onchange={autosavePreferences}>
        <section><h3>{t("appearance")}</h3><label>{t("language")}<select value={preferences.language} onchange={(event) => { event.stopPropagation(); changeLanguage(event.currentTarget.value as Language); }}>{#each languageOptions as option}<option value={option.value}>{option.label}</option>{/each}</select></label><label>{t("theme")}<select bind:value={preferences.theme}><option value="system">{t("system")}</option><option value="dark">{t("dark")}</option><option value="light">{t("light")}</option></select></label><label>{t("timeDisplay")}<select bind:value={preferences.timeDisplay}><option value="simple">{t("timeDisplaySimple")}</option><option value="precise">{t("timeDisplayPrecise")}</option></select></label><label>{t("notificationDuration")}<span class="preference-number"><input type="number" min="1" max="10" bind:value={preferences.toastDurationSeconds} /><small>{t("seconds")}</small></span></label></section>
        <section><h3>{t("importSettings")}</h3><label>{t("simultaneousDownloads")}<input type="number" min="1" max="8" bind:value={preferences.concurrentDownloads} /></label><label>{t("youtubeAutoSelectBestMatch")}<input type="checkbox" bind:checked={preferences.youtubeAutoSelectBestMatch} /></label><label>{t("conversionFormat")}<select bind:value={preferences.conversionFormat}><option value="keep">{t("keepSupported")}</option><option value="mp3">MP3</option><option value="wav">WAV</option><option value="flac">FLAC</option></select></label><label>{t("mp3Quality")}<select bind:value={preferences.mp3Quality}><option value="vbrHigh">{t("mp3VbrHigh")}</option><option value="kbps320">320 kb/s</option><option value="kbps256">256 kb/s</option><option value="kbps192">192 kb/s</option></select></label><label>{t("sampleRate")}<select bind:value={preferences.sampleRate}><option value="preserve">{t("preserve")}</option><option value="hz44100">44.1 kHz</option><option value="hz48000">48 kHz</option></select></label><label>{t("channels")}<select bind:value={preferences.channels}><option value="preserve">{t("preserve")}</option><option value="stereo">{t("stereo")}</option><option value="mono">{t("mono")}</option></select></label></section>
        <section><h3>{t("practiceDefaults")}</h3><label>{t("navigationDefault")}<select bind:value={preferences.navigationMode}><option value="time">{t("navigationTime")}</option><option value="beat">{t("navigationBeat")}</option><option value="chord">{t("navigationChord")}</option></select></label><label>{t("navigationTimeStep")}<span class="preference-number"><input type="number" min="1" max="60" bind:value={preferences.navigationTimeSeconds} /><small>{t("seconds")}</small></span></label><label>{t("loopLoadPosition")}<select bind:value={preferences.loopLoadPosition}><option value="beginning">{t("fromBeginning")}</option><option value="loopStart">{t("fromLoopStart")}</option></select></label><label>{t("loopSnap")}<input type="checkbox" bind:checked={preferences.loopSnapEnabled} /></label><label>{t("startSpeed")}<input type="number" min="50" max="199" value={preferences.defaultTrainerStartRate * 100} onchange={(event) => preferences.defaultTrainerStartRate = Number(event.currentTarget.value) / 100} /></label><label>{t("endSpeed")}<input type="number" min="51" max="200" value={preferences.defaultTrainerTargetRate * 100} onchange={(event) => preferences.defaultTrainerTargetRate = Number(event.currentTarget.value) / 100} /></label><label>{t("stepSize")}<input type="number" min="1" max="25" value={preferences.defaultTrainerIncrement * 100} onchange={(event) => preferences.defaultTrainerIncrement = Number(event.currentTarget.value) / 100} /></label><label>{t("loopsPerStep")}<input type="number" min="1" max="99" bind:value={preferences.defaultTrainerRepetitions} /></label></section>
        <section><h3>{t("audio")}</h3><label>{t("masterVolume")}<input class="master-volume-preference" type="range" min="0" max="2" step="0.01" bind:value={preferences.masterVolume} style={`--master-volume-color: ${masterVolumeColor(preferences.masterVolume)}`} ondblclick={() => resetPreferenceVolume("masterVolume")} /></label><label>{t("musicVolume")}<input type="range" min="0" max="1" step="0.01" bind:value={preferences.musicVolume} ondblclick={() => resetPreferenceVolume("musicVolume")} /></label><label>{t("loudnessNormalization")}<input type="checkbox" bind:checked={preferences.loudnessNormalization} /></label><label>{t("metronomeVolume")}<input type="range" min="0" max="1" step="0.01" bind:value={preferences.metronomeVolume} ondblclick={() => resetPreferenceVolume("metronomeVolume")} /></label><label>{t("metronomeSound")}<select bind:value={preferences.metronomeSound}><option value="electronic">{t("metronomeElectronic")}</option><option value="woodblock">{t("metronomeWoodblock")}</option><option value="metallic">{t("metronomeMetallic")}</option></select></label></section>
      </div>
      <div class="modal-actions"><button onclick={resetUserPreferences}>{t("resetPreferences")}</button></div>
    </Modal>
  {/if}

  {#if shortcutsVisible}
    <Modal title={t("shortcuts")} closeLabel={t("close")} wide close={() => shortcutsVisible = false}>
      <div class="shortcut-groups" class:macos={shortcutPlatform === "macos"}>
        <section><h3>{t("transport")}</h3><dl class="shortcut-list">
          <dt>{t("playPause")}</dt><dd><kbd>{shortcutKeys.space}</kbd></dd>
        </dl></section>
        <section><h3>{t("navigation")}</h3><dl class="shortcut-list">
          <dt>{t("previousNavigation")}</dt><dd><kbd>←</kbd></dd>
          <dt>{t("nextNavigation")}</dt><dd><kbd>→</kbd></dd>
          <dt>{t("changeNavigationMode")}</dt><dd><kbd>N</kbd></dd>
        </dl></section>
        <section><h3>{t("loop")}</h3><dl class="shortcut-list">
          <dt>{t("moveA")}</dt><dd><kbd>A</kbd></dd>
          <dt>{t("moveB")}</dt><dd><kbd>B</kbd></dd>
          <dt>{t("toggleLoop")}</dt><dd><kbd>L</kbd></dd>
          <dt>{t("clearLoop")}</dt><dd><kbd>Esc</kbd></dd>
        </dl></section>
        <section><h3>{t("tempo")}</h3><dl class="shortcut-list">
          <dt>{t("faster")}</dt><dd><kbd>T</kbd><span>+</span><kbd>↑</kbd><span>{t("or")}</span><kbd>→</kbd><span>{t("or")}</span><kbd>+</kbd></dd>
          <dt>{t("slower")}</dt><dd><kbd>T</kbd><span>+</span><kbd>↓</kbd><span>{t("or")}</span><kbd>←</kbd><span>{t("or")}</span><kbd>−</kbd></dd>
          <dt>{t("resetTempo")}</dt><dd><kbd>T</kbd><span>+</span><kbd>{shortcutKeys.backspace}</kbd><span>{t("or")}</span><kbd>{shortcutKeys.delete}</kbd></dd>
        </dl></section>
        <section><h3>{t("pitch")}</h3><dl class="shortcut-list">
          <dt>{t("pitchUp")}</dt><dd><kbd>P</kbd><span>+</span><kbd>↑</kbd><span>{t("or")}</span><kbd>→</kbd><span>{t("or")}</span><kbd>+</kbd></dd>
          <dt>{t("pitchDown")}</dt><dd><kbd>P</kbd><span>+</span><kbd>↓</kbd><span>{t("or")}</span><kbd>←</kbd><span>{t("or")}</span><kbd>−</kbd></dd>
          <dt>{t("resetPitch")}</dt><dd><kbd>P</kbd><span>+</span><kbd>{shortcutKeys.backspace}</kbd><span>{t("or")}</span><kbd>{shortcutKeys.delete}</kbd></dd>
        </dl></section>
        <section><h3>{t("zoom")}</h3><dl class="shortcut-list">
          <dt>{t("zoom")} +</dt><dd><kbd>Z</kbd><span>+</span><kbd>↑</kbd><span>{t("or")}</span><kbd>→</kbd><span>{t("or")}</span><kbd>+</kbd></dd>
          <dt>{t("zoom")} −</dt><dd><kbd>Z</kbd><span>+</span><kbd>↓</kbd><span>{t("or")}</span><kbd>←</kbd><span>{t("or")}</span><kbd>−</kbd></dd>
          <dt>{t("fitThirtySeconds")}</dt><dd><kbd>Z</kbd><span>+</span><kbd>{shortcutKeys.backspace}</kbd><span>{t("or")}</span><kbd>{shortcutKeys.delete}</kbd></dd>
        </dl></section>
        <section><h3>{t("metronome")}</h3><dl class="shortcut-list">
          <dt>{t("metronome")}</dt><dd><kbd>M</kbd></dd>
          <dt>{t("metronomeSound")}</dt><dd><kbd>M</kbd><span>+</span><kbd>↑</kbd><span>{t("or")}</span><kbd>↓</kbd></dd>
          <dt>{t("metronomeVolume")} +</dt><dd><kbd>M</kbd><span>+</span><kbd>→</kbd><span>{t("or")}</span><kbd>+</kbd></dd>
          <dt>{t("metronomeVolume")} −</dt><dd><kbd>M</kbd><span>+</span><kbd>←</kbd><span>{t("or")}</span><kbd>−</kbd></dd>
          <dt>{t("metronomeVolume")} · 55%</dt><dd><kbd>M</kbd><span>+</span><kbd>{shortcutKeys.backspace}</kbd><span>{t("or")}</span><kbd>{shortcutKeys.delete}</kbd></dd>
        </dl></section>
        <section><h3>{t("instrumentView")}</h3><dl class="shortcut-list">
          <dt>{t("changeInstrumentView")}</dt><dd><kbd>I</kbd></dd>
        </dl></section>
        <section><h3>{t("chords")}</h3><dl class="shortcut-list">
          <dt>{t("chordEditMode")}</dt><dd><kbd>E</kbd></dd>
        </dl></section>
        <section><h3>{t("shortcutInterface")}</h3><dl class="shortcut-list">
          <dt>{t("showConsole")}</dt><dd><kbd>C</kbd></dd>
          <dt>{t("showHelp")}</dt><dd><kbd>H</kbd></dd>
        </dl></section>
      </div>
      <div class="modal-actions"><button onclick={() => shortcutsVisible = false}>{t("close")}</button></div>
    </Modal>
  {/if}

  {#if importVisible}
    <Modal title={t("importCenter")} closeLabel={t("close")} wide close={() => importVisible = false} keydown={handleImportDialogKeydown}>
      <div class="import-center" role="region" aria-label={t("importCenter")}>
        <div class="import-toolbar">
          <div class="import-toolbar-actions">
            <button onclick={chooseImportFiles}>{t("addFiles")}</button>
          </div>
        </div>
        <textarea bind:this={importTextarea} use:disableTextareaAutocorrect class:drop-active={importDropActive} bind:value={importText} oninput={scheduleImportAnalysis} ondragover={(event) => { event.preventDefault(); importDropActive = true; }} ondragleave={() => importDropActive = false} ondrop={(event) => { event.preventDefault(); importDropActive = false; const text = event.dataTransfer?.getData("text/plain"); if (text) { importText = [importText, text].filter(Boolean).join("\n"); void analyzeImports(); } }} placeholder={t("importPlaceholder")}></textarea>
        <div class="import-analysis-state">
          {#if importAnalyzing && importSearchTotal > 0}
            <div class="import-search-progress" aria-live="polite">
              <span><i class="mini-spinner"></i>{t("searchProgress")} <b>{importSearchCompleted}/{importSearchTotal}</b></span>
              <progress value={importSearchCompleted} max={importSearchTotal} aria-label={`${t("searchProgress")} ${importSearchCompleted}/${importSearchTotal}`}></progress>
            </div>
          {:else if importAnalyzing}<span><i class="mini-spinner"></i>{t("analyzingSources")}</span>
          {:else if importAnalysisError}<span class="failed">{importAnalysisError}</span>
          {:else if importHasAnalyzed}<span>{importCandidates.length} {t("sourcesFound")}</span>
          {:else}<span>{t("dropToAnalyze")}</span>{/if}
        </div>
        {#if importCandidateGroups.length}
          <div class="candidate-groups">
            {#each importCandidateGroups as group}
              <section class="candidate-group" class:loading={importPendingGroupIds.has(group.id)}>
                  <header><span data-tooltip={group.query === null ? t("directSources") : `${t("searchResults")} ${group.searchIndex}`}><Icon name={group.query === null ? "file" : "magnifying-glass"} label={group.query === null ? t("directSources") : `${t("searchResults")} ${group.searchIndex}`} size="13px" /></span>{#if group.query}<strong>{group.query}</strong>{/if}{#if importActiveGroupIds.has(group.id)}<i class="mini-spinner"></i>{:else if importPendingGroupIds.has(group.id)}<small>{t("queued")}</small>{/if}</header>
                {#if group.candidates.length}
                  <div class="candidate-list">
                    {#each group.candidates as candidate}
                      <div class="candidate-row" class:selected={selectedImports.has(candidate.input)} class:has-thumbnail={candidate.thumbnailUrl !== undefined}>
                        <button class="candidate-select-target" aria-label={candidate.title} aria-pressed={selectedImports.has(candidate.input)} onclick={() => toggleImport(candidate.input)}></button>
                        <span class="candidate-check" aria-hidden="true"><i>{selectedImports.has(candidate.input) ? "✓" : ""}</i></span>
                        {#if candidate.thumbnailUrl}<span class="candidate-thumbnail"><Icon name="music" size="15px" /><img src={candidate.thumbnailUrl} alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" onerror={hideBrokenThumbnail} /></span>{/if}
                        <div class="candidate-copy">
                          <strong class="candidate-title">{candidate.title}</strong>
                          <div class="candidate-meta">
                            <span class="candidate-detail">{candidate.detail}</span>
                            {#if candidate.matchScore !== undefined}<span class="candidate-separator" aria-hidden="true">•</span><span class={`candidate-score relevance-${importRelevanceLevel(candidate.matchScore)}`}>{t("youtubeMatchScore")} {importRelevancePercent(candidate.matchScore)} %</span>{/if}
                            {#if candidate.videoId}<span class="candidate-separator" aria-hidden="true">•</span><button class="candidate-video-link" aria-label={`${candidate.title} · YouTube ${candidate.videoId}`} data-tooltip={candidate.input} onclick={() => openImportVideo(candidate.videoId!)}><span class="candidate-youtube-icon"><Icon name="youtube" size="13px" /></span>{candidate.videoId}</button>{/if}
                          </div>
                        </div>
                      </div>
                    {/each}
                  </div>
                {:else if importGroupErrors.has(group.id)}<p class="candidate-group-error">{importGroupErrors.get(group.id)}</p>
                {:else if importPendingGroupIds.has(group.id)}<div class="candidate-group-wait"><i></i><i></i><i></i></div>
                {:else}<div class="candidate-group-empty">{t("noSourcesFound")}</div>{/if}
              </section>
            {/each}
          </div>
        {:else if importHasAnalyzed && !importAnalysisError}
          <div class="import-empty">{t("noSourcesFound")}</div>
        {/if}
        <small class="authorized-note">{t("authorizedOnly")}</small>
        <div class="modal-actions"><button onclick={() => importVisible = false}>{t("close")}</button><button class="primary" disabled={selectedImports.size === 0 || importAnalyzing || busy} onclick={startImports}>{t("startImport")} ({selectedImports.size})</button></div>
      </div>
    </Modal>
  {/if}

  {#if tasksVisible}
    <Modal title={t("importQueue")} closeLabel={t("close")} wide close={() => tasksVisible = false}>{#if !importQueue.length}<p>{t("noTasks")}</p>{:else}<div class="job-list">{#each [...importQueue].reverse() as job}<article class:failed={job.state === "failed"}><div class="job-heading"><span><strong>{job.label}</strong><span>{t(job.state as MessageKey)} · {Math.round(job.progress * 100)}%</span></span><button class="job-remove" aria-label={t("cancelImport")} data-tooltip={t("cancelImport")} onclick={() => void cancelImportJob(job.id)}><Icon name="xmark" size="11px" /></button></div><i><b style={`width:${job.progress * 100}%`}></b></i>{#if job.error}<p>{job.error}</p>{/if}{#if job.suggestion}<small>{job.suggestion}</small>{/if}{#if job.diagnostic}<details><summary>{t("technicalDetails")}</summary><pre>{job.diagnostic}</pre></details>{/if}</article>{/each}</div>{/if}<button onclick={() => tasksVisible = false}>{t("close")}</button></Modal>
  {/if}

  {#if trackContextMenu}
    <div class="context-menu" role="menu" tabindex="-1" style={`left:${trackContextMenu.x}px;top:${trackContextMenu.y}px`} onpointerdown={(event) => event.stopPropagation()}>
      <button onclick={() => { const track = project?.tracks.find((item) => item.id === trackContextMenu?.trackId); if (track) void removeTrack(track); }}>{t("removeTrack")}</button>
    </div>
  {/if}
</main>
