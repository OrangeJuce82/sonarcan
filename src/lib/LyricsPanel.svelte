<script lang="ts">
  import { onDestroy, tick } from "svelte";
  import type { Language } from "./i18n";
  import type { LyricsDocument, LyricsSearchResult } from "./types";
  import { activeLyricsLineIndex, activeLyricsWordIndex, estimatedLyricsLineIndex, formatLrcTime, lyricsEditorContent, LyricsParseError, parseLyrics } from "./lyrics";
  import { lyricsDurationRelevanceLevel } from "./lyricsMatching";
  import { lyricsTranslate } from "./lyricsI18n";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  export let document: LyricsDocument | null;
  export let language: Language;
  export let currentMs: number;
  export let durationMs: number;
  export let loading = false;
  export let loadError = "";
  export let onSeek: (milliseconds: number) => void;
  export let onLoop: (startMs: number, endMs: number) => void;
  export let onSave: (document: LyricsDocument) => Promise<void>;
  export let onDelete: () => Promise<void>;
  export let initialSearchQuery: string;
  export let searchResults: LyricsSearchResult[] = [];
  export let searching = false;
  export let searchError = "";
  export let onSearch: (query: string) => Promise<void>;
  export let onChooseSearchResult: (result: LyricsSearchResult) => Promise<void>;
  export let onOpenProvider: (query: string) => void;

  let autoFollow = true;
  let editorVisible = false;
  let deleteVisible = false;
  let editorText = "";
  let editorError = "";
  let searchVisible = false;
  let searchQuery = "";
  let searchSubmitted = false;
  let searchDebouncing = false;
  let searchDebounceTimer: number | undefined;
  let selectedStart = -1;
  let selectedEnd = -1;
  let linesElement: HTMLElement | undefined;
  let editorElement: HTMLTextAreaElement | undefined;
  let lastFollowedIndex = -1;
  let activeIndex = -1;
  let tr: (key: Parameters<typeof lyricsTranslate>[1]) => string;
  $: tr = (key) => lyricsTranslate(language, key);
  $: activeIndex = document?.syncLevel === "none"
    ? estimatedLyricsLineIndex(document, currentMs, durationMs)
    : activeLyricsLineIndex(document, currentMs);
  $: if (!document) resetSelection();
  $: if (!editorVisible
    && document?.syncLevel !== "none"
    && activeIndex >= 0
    && selectedStart >= 0
    && selectedStart === selectedEnd
    && selectedStart !== activeIndex) {
    selectedStart = activeIndex;
    selectedEnd = activeIndex;
  }
  $: if (!editorVisible && autoFollow && activeIndex >= 0 && activeIndex !== lastFollowedIndex) followLine(activeIndex);
  onDestroy(() => clearSearchDebounce());

  function resetSelection(): void {
    selectedStart = -1;
    selectedEnd = -1;
    lastFollowedIndex = -1;
  }

  function openSearch(): void {
    searchVisible = true;
    scheduleSearch(initialSearchQuery);
  }

  function closeSearch(): void {
    clearSearchDebounce();
    searchVisible = false;
  }

  function clearSearchDebounce(): void {
    if (searchDebounceTimer !== undefined) window.clearTimeout(searchDebounceTimer);
    searchDebounceTimer = undefined;
    searchDebouncing = false;
  }

  function scheduleSearch(value: string): void {
    searchQuery = value;
    searchSubmitted = false;
    clearSearchDebounce();
    const query = value.trim();
    if (!query) return;
    searchDebouncing = true;
    searchDebounceTimer = window.setTimeout(() => {
      searchDebounceTimer = undefined;
      searchDebouncing = false;
      void submitSearch(query);
    }, 350);
  }

  async function submitSearch(query: string): Promise<void> {
    searchSubmitted = true;
    await onSearch(query);
  }

  async function chooseResult(result: LyricsSearchResult): Promise<void> {
    try {
      await onChooseSearchResult(result);
      closeSearch();
    } catch {
      // The search dialog retains the provider error and remains retryable.
    }
  }

  async function followLine(index: number): Promise<void> {
    lastFollowedIndex = index;
    await tick();
    const target = linesElement?.querySelector<HTMLElement>(`[data-lyrics-index="${index}"]`);
    if (!target || !linesElement) return;
    const top = linesElement.scrollTop + target.getBoundingClientRect().top
      - linesElement.getBoundingClientRect().top - (linesElement.clientHeight - target.clientHeight) / 2;
    linesElement.scrollTo({ top: Math.max(0, top), behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
  }

  function selectLine(event: MouseEvent, index: number): void {
    if (!document) return;
    if (event.shiftKey && selectedStart >= 0) selectedEnd = index;
    else selectedStart = selectedEnd = index;
    const start = document.lines[index]?.startMs;
    if (start !== null && start !== undefined) onSeek(Math.max(0, start + document.offsetMs));
  }

  function isSelected(index: number): boolean {
    return selectedStart >= 0 && index >= Math.min(selectedStart, selectedEnd) && index <= Math.max(selectedStart, selectedEnd);
  }

  function loopSelection(): void {
    if (!document || selectedStart < 0) return;
    const startIndex = Math.min(selectedStart, selectedEnd);
    const endIndex = Math.max(selectedStart, selectedEnd);
    const start = document.lines[startIndex]?.startMs;
    const endLine = document.lines[endIndex];
    const end = endLine?.endMs ?? document.lines[endIndex + 1]?.startMs ?? durationMs;
    if (start !== null && start !== undefined && end !== null && end !== undefined && end > start) {
      onLoop(Math.max(0, start + document.offsetMs), Math.min(durationMs, end + document.offsetMs));
    }
  }

  async function changeOffset(delta: number): Promise<void> {
    if (!document) return;
    try {
      await onSave({ ...document, offsetMs: Math.max(-30_000, Math.min(30_000, document.offsetMs + delta)) });
    } catch {
      // The application shell reports persistence failures through its toast stack.
    }
  }

  async function openEditor(): Promise<void> {
    const editor = document ? lyricsEditorContent(document, activeIndex) : { text: editorText, selectionStart: 0, selectionEnd: editorText.length };
    if (document) editorText = editor.text;
    editorError = "";
    autoFollow = false;
    editorVisible = true;
    await tick();
    editorElement?.focus();
    editorElement?.setSelectionRange(editor.selectionStart, editor.selectionEnd);
  }

  async function saveEditor(): Promise<void> {
    try {
      const parsed = parseLyrics(editorText, document?.language ?? language, durationMs);
      parsed.offsetMs = document?.offsetMs ?? 0;
      await onSave(parsed);
      editorVisible = false;
    } catch (error) {
      editorError = error instanceof LyricsParseError
        ? tr(error.code)
        : error instanceof Error ? error.message : tr("importError");
    }
  }

  async function confirmDelete(): Promise<void> {
    try {
      await onDelete();
      editorText = "";
      editorError = "";
      editorVisible = true;
      deleteVisible = false;
    } catch {
      // Keep the confirmation open so the user can retry.
    }
  }

</script>

<section class="lyrics-panel" aria-label={tr("lyrics")}>
  <div class="lyrics-toolbar">
    <div class="lyrics-source">
      <b class="lyrics-sync-badge" class:synchronized={Boolean(document && document.syncLevel !== "none")}>{document && document.syncLevel !== "none" ? "Sync" : "Unsync"}</b>
      <b>{(document?.language ?? language).toUpperCase()}</b>
    </div>
    <div class="lyrics-actions">
      <button class="lyrics-header-button" class:active={editorVisible || !document} disabled={editorVisible} aria-label={tr("edit")} data-tooltip={tr("edit")} onclick={openEditor}><Icon name="pen" size="12px" /></button>
      <button class="lyrics-header-button danger-icon" disabled={!document} aria-label={tr("delete")} data-tooltip={tr("delete")} onclick={() => deleteVisible = true}><Icon name="trash" size="11px" /></button>
      <i class="lyrics-action-separator" aria-hidden="true"></i>
      <button class="lyrics-header-button" aria-label={tr("searchOnline")} data-tooltip={tr("searchOnline")} onclick={openSearch}><Icon name="magnifying-glass" size="12px" /></button>
      <i class="lyrics-action-separator" aria-hidden="true"></i>
      <button class="lyrics-header-button" class:active={autoFollow && !editorVisible && Boolean(document)} disabled={editorVisible || !document} aria-pressed={autoFollow && !editorVisible && Boolean(document)} aria-label={tr("follow")} data-tooltip={tr("follow")} onclick={() => autoFollow = !autoFollow}><Icon name="arrow-down" size="13px" /></button>
    </div>
  </div>
  {#if loading}
    <p class="lyrics-state"><i class="mini-spinner"></i>{tr("loading")}</p>
  {:else if loadError}
    <p class="lyrics-state failed">{tr("error")}<small>{loadError}</small></p>
  {:else}
    {#if editorVisible || !document}
      <div class="lyrics-inline-editor">
        <textarea bind:this={editorElement} bind:value={editorText} maxlength={2 * 1024 * 1024} spellcheck="false" aria-label={tr("editorTitle")} placeholder={tr("editorHelp")}></textarea>
        {#if editorError}<p class="lyrics-editor-error">{editorError}</p>{/if}
        <div class="lyrics-validation-actions">{#if document}<button onclick={() => editorVisible = false}>{tr("cancel")}</button>{/if}<button class="primary" disabled={!editorText.trim()} onclick={saveEditor}>{tr("save")}</button></div>
      </div>
    {:else if document}
      <div class="lyrics-lines" bind:this={linesElement} aria-live="off">
        {#each document.lines as line, index}
          {@const wordIndex = activeLyricsWordIndex(line, currentMs, document.offsetMs)}
          <button
            type="button"
            data-lyrics-index={index}
            class:active={index === activeIndex}
            class:selected={isSelected(index)}
            class:untimed={line.startMs === null}
            aria-current={index === activeIndex ? "true" : undefined}
            aria-pressed={isSelected(index)}
            onclick={(event) => selectLine(event, index)}
          >
            {#if line.words.length}
              {#each line.words as word, wordPosition}<span class:active={wordPosition <= wordIndex}>{word.text}</span>{/each}
            {:else}{line.text}{/if}
            {#if line.startMs !== null}<small>{formatLrcTime(line.startMs)}</small>{/if}
          </button>
        {/each}
      </div>
    {/if}
    {#if document}<div class="lyrics-footer">
        <span>{tr("selectionHelp")}</span>
        <div><button disabled={selectedStart < 0 || document.syncLevel === "none"} onclick={loopSelection}>{tr("loopSelection")}</button><i></i><b>{tr("offset")}</b><button onclick={() => changeOffset(-100)}>−100 ms</button><output>{document.offsetMs > 0 ? "+" : ""}{document.offsetMs} ms</output><button onclick={() => changeOffset(100)}>+100 ms</button><button disabled={document.offsetMs === 0} onclick={() => changeOffset(-document.offsetMs)}>{tr("reset")}</button></div>
      </div>{/if}
  {/if}
</section>

{#if searchVisible}
  <Modal title={tr("lyrics")} closeLabel={tr("close")} close={closeSearch} wide>
    {#snippet titleContent()}<span class="lyrics-search-title"><Icon name="magnifying-glass" size="13px" />{tr("lyrics")}</span>{/snippet}
    {#snippet headerActions()}
      <div class="lyrics-search-input">
        <input value={searchQuery} oninput={(event) => scheduleSearch(event.currentTarget.value)} maxlength="200" autocomplete="off" aria-label={tr("searchPlaceholder")} placeholder={tr("searchPlaceholder")} />
        {#if searchDebouncing || searching}<i class="mini-spinner" aria-label={tr("loading")}></i>{/if}
      </div>
      <button class="lyrics-provider-badge" aria-label={`${tr("searchOnline")}: LRCLIB`} data-tooltip={`${tr("searchOnline")}: LRCLIB`} onclick={() => onOpenProvider(searchQuery || initialSearchQuery)}>LRCLIB</button>
    {/snippet}
    <div class="lyrics-search-dialog">
      <div class="lyrics-results">
        {#if searchError}<p class="lyrics-editor-error">{searchError}</p>
        {:else if searchSubmitted && !searchDebouncing && !searching && searchResults.length === 0}<p class="lyrics-search-empty">{tr("noResults")}</p>{/if}
        {#each searchResults as result}
          <button type="button" class="lyrics-result-row" onclick={() => chooseResult(result)}>
            <div><strong>{result.trackName}</strong><span>{result.artistName}{result.albumName ? ` · ${result.albumName}` : ""}</span></div>
            <div class="lyrics-result-badges">
              <small class="lyrics-result-duration" style={`--duration-relevance:var(--relevance-${lyricsDurationRelevanceLevel(result.durationSeconds, durationMs / 1_000)})`}>{Math.floor(result.durationSeconds / 60)}:{String(Math.round(result.durationSeconds % 60)).padStart(2, "0")}</small>
              <b class:sync={result.hasSyncedLyrics} class:unsync={!result.hasSyncedLyrics}>{result.hasSyncedLyrics ? "Sync" : "Unsync"}</b>
            </div>
          </button>
        {/each}
      </div>
    </div>
  </Modal>
{/if}

<style>
  .lyrics-panel { display: grid; grid-template-rows: auto minmax(0, 1fr) auto; min-width: 0; min-height: 0; height: 100%; overflow: hidden; }
  .lyrics-toolbar { display: flex; align-items: center; justify-content: space-between; min-width: 0; margin-bottom: 8px; gap: 10px; }
  .lyrics-source, .lyrics-actions, .lyrics-footer > div { display: flex; align-items: center; min-width: 0; gap: 7px; }
  .lyrics-provider-badge { display: inline-flex; align-items: center; min-height: 0; padding: 2px 6px; gap: 4px; border: 1px solid var(--accent-border); border-radius: 999px; color: var(--accent-strong); background: var(--accent-soft); font-size: .55rem; font-weight: 800; line-height: 1.2; letter-spacing: .08em; }
  .lyrics-provider-badge:hover:not(:disabled), .lyrics-provider-badge:focus-visible { color: var(--text-strong); background: var(--accent-bg); }
  .lyrics-source b { padding: 2px 5px; border: 1px solid var(--border); border-radius: 999px; color: var(--accent-strong); background: var(--accent-soft); font-size: .5rem; letter-spacing: .08em; }
  .lyrics-source .lyrics-sync-badge { color: var(--muted); background: var(--surface-deep); }
  .lyrics-source .lyrics-sync-badge.synchronized { border-color: var(--accent-border); color: var(--accent-strong); background: var(--accent-soft); }
  .lyrics-actions { flex: 0 0 auto; }
  .lyrics-actions > button, .lyrics-footer button { min-height: 25px; padding: 3px 7px; font-size: .56rem; }
  .lyrics-actions > button.active { border-color: var(--accent-border); color: var(--accent-strong); background: var(--accent-soft); }
  .lyrics-header-button { display: grid; width: 27px; min-width: 27px; height: 25px; padding: 0; place-items: center; border-radius: 5px; color: var(--muted); }
  .lyrics-header-button:hover:not(:disabled), .lyrics-header-button:focus-visible { color: var(--text-strong); }
  .lyrics-header-button.danger-icon:not(:disabled) { color: var(--danger); }
  .lyrics-action-separator { width: 1px; height: 18px; margin-inline: 1px; background: var(--border); }
  .lyrics-lines { display: grid; align-content: start; min-height: 176px; max-height: 290px; overflow-y: auto; padding: 0 8px; gap: 4px; overscroll-behavior: contain; scrollbar-gutter: stable; }
  .lyrics-lines button { position: relative; display: block; width: 100%; min-height: 34px; padding: 7px 58px 7px 10px; border: 1px solid transparent; color: var(--muted); background: transparent; font-size: .84rem; font-weight: 650; line-height: 1.35; text-align: left; transition: color 140ms ease, background-color 140ms ease, transform 140ms ease; }
  :global([dir="rtl"]) .lyrics-lines button { padding: 7px 10px 7px 58px; text-align: right; }
  .lyrics-lines button:hover, .lyrics-lines button:focus-visible { color: var(--text-strong); background: var(--surface-hover); }
  .lyrics-lines button.selected { border-color: var(--purple); background: var(--purple-bg); }
  .lyrics-lines button.active { color: var(--text-strong); background: var(--accent-soft); transform: scale(1.015); }
  .lyrics-lines button span { color: var(--muted); transition: color 100ms linear; }
  .lyrics-lines button span.active { color: var(--accent-strong); }
  .lyrics-lines button small { position: absolute; top: 10px; right: 8px; color: var(--muted-soft); font: .52rem/1 ui-monospace, SFMono-Regular, monospace; }
  :global([dir="rtl"]) .lyrics-lines button small { right: auto; left: 8px; }
  .lyrics-lines button.untimed { cursor: default; }
  .lyrics-footer { display: grid; margin-top: 8px; gap: 7px; }
  .lyrics-footer > span { overflow: hidden; color: var(--muted); font-size: .55rem; text-overflow: ellipsis; white-space: nowrap; }
  .lyrics-footer > div { justify-content: flex-end; overflow-x: auto; }
  .lyrics-footer i { width: 1px; height: 18px; background: var(--border); }
  .lyrics-footer b { color: var(--muted); font-size: .55rem; text-transform: uppercase; }
  .lyrics-footer output { min-width: 54px; color: var(--text-strong); font: .56rem/1 ui-monospace, SFMono-Regular, monospace; text-align: center; }
  .lyrics-state { display: grid; align-content: center; justify-items: center; min-height: 205px; margin: 0; gap: 10px; color: var(--muted); text-align: center; }
  .lyrics-state { grid-template-columns: auto auto; }
  .lyrics-state.failed { display: grid; color: var(--danger); }
  .lyrics-state small { color: var(--muted); }
  .lyrics-inline-editor { display: grid; min-height: 176px; max-height: 290px; gap: 7px; }
  .lyrics-inline-editor textarea { box-sizing: border-box; width: 100%; min-height: 0; height: 100%; resize: none; padding: 12px; border: 1px solid var(--accent-border); border-radius: 7px; color: var(--text); background: var(--surface-deep); font: .75rem/1.55 ui-monospace, SFMono-Regular, monospace; }
  .lyrics-validation-actions { display: flex; justify-content: flex-end; gap: 6px; }
  .lyrics-validation-actions button { min-height: 25px; padding: 4px 8px; border-radius: 5px; font-size: .58rem; line-height: 1; }
  .lyrics-editor-error { color: var(--danger); font-size: .68rem; }
  .lyrics-search-title { display: inline-flex; align-items: center; gap: 8px; }
  .lyrics-search-dialog { height: 420px; }
  .lyrics-search-input { position: relative; display: flex; align-items: center; width: 100%; height: 34px; }
  .lyrics-search-input input { width: 100%; height: 34px; padding: 6px 36px 6px 11px; border: 1px solid var(--border); border-radius: 7px; color: var(--text); background: var(--surface-deep); font: inherit; font-size: .7rem; }
  :global([dir="rtl"]) .lyrics-search-input input { padding: 6px 11px 6px 36px; }
  .lyrics-search-input .mini-spinner { position: absolute; right: 11px; }
  :global([dir="rtl"]) .lyrics-search-input .mini-spinner { right: auto; left: 11px; }
  .lyrics-results { display: grid; height: 100%; min-height: 0; overflow-y: auto; align-content: start; gap: 6px; }
  .lyrics-result-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; width: 100%; padding: 9px; gap: 10px; border: 1px solid var(--border); border-radius: 7px; color: var(--text); background: var(--surface-deep); text-align: left; }
  .lyrics-result-row:hover, .lyrics-result-row:focus-visible { border-color: var(--accent-border); color: var(--text); background: var(--surface-hover); }
  .lyrics-result-row > div:first-child { display: grid; min-width: 0; gap: 3px; }
  .lyrics-results strong, .lyrics-results span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .lyrics-results strong { color: var(--text-strong); font-size: .7rem; }
  .lyrics-results span, .lyrics-results small, .lyrics-search-empty { color: var(--muted); font-size: .58rem; }
  .lyrics-result-badges { display: flex; align-items: center; justify-content: flex-end; gap: 6px; }
  .lyrics-result-duration { color: var(--duration-relevance) !important; font-weight: 800; font-variant-numeric: tabular-nums; }
  .lyrics-result-badges b { padding: 2px 6px; border: 1px solid; border-radius: 999px; font-size: .5rem; line-height: 1.2; letter-spacing: .06em; }
  .lyrics-result-badges b.sync { border-color: color-mix(in srgb, var(--relevance-4) 65%, var(--border)); color: var(--relevance-4); background: color-mix(in srgb, var(--relevance-4) 12%, transparent); }
  .lyrics-result-badges b.unsync { border-color: color-mix(in srgb, var(--gold) 55%, var(--border)); color: var(--gold-text); background: var(--gold-bg); }
  .lyrics-search-empty { padding: 24px; text-align: center; }
  :global(button.danger) { border-color: var(--danger-border); color: var(--danger); background: var(--danger-soft); }
  @media (prefers-reduced-motion: reduce) { .lyrics-lines button { transition: none; } }
</style>

{#if deleteVisible}
  <Modal title={tr("deleteTitle")} closeLabel={tr("close")} close={() => deleteVisible = false}>
    <p>{tr("deleteHelp")}</p>
    <div class="modal-actions"><button onclick={() => deleteVisible = false}>{tr("cancel")}</button><button class="danger" onclick={confirmDelete}>{tr("delete")}</button></div>
  </Modal>
{/if}
