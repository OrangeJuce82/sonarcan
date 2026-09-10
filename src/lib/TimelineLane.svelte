<script lang="ts">
  import Icon from "./Icon.svelte";
  import { centeredChordOptionScrollTop, chordEditPointerAction } from "./chordEditing";
  import { nearestTimelineSnapPosition, snappedTimelineRange, type TimelineLaneItem } from "./timelineLane";

  type LaneIcon = "flag" | "music" | "microphone";
  type DragState = {
    id: string;
    edge: "start" | "end";
    pointerId: number;
    startSeconds: number;
    endSeconds: number;
  };

  export let icon: LaneIcon;
  export let label: string;
  export let addLabel: string;
  export let editLabel: string;
  export let deleteLabel: string;
  export let startLabel: string;
  export let endLabel: string;
  export let items: TimelineLaneItem[];
  export let durationSeconds: number;
  export let viewportStart: number;
  export let viewportZoom: number;
  export let selectedId: string | null;
  export let onSelect: (id: string | null) => void;
  export let onSeek: (seconds: number) => void;
  export let onAdd: () => void;
  export let onRename: (id: string, label: string, replaceAll: boolean) => boolean;
  export let suggestions: ((value: string) => string[]) | null = null;
  export let options: string[] | null = null;
  export let snapPoints: number[] = [];
  export let formatOption: (value: string) => string = (value) => value;
  export let onResize: (id: string, startSeconds: number, endSeconds: number) => void;
  export let onDelete: (id: string) => void;
  export let onEditStart: (id: string) => void = () => {};
  export let onWheel: (event: WheelEvent) => void;
  export let onPointerEnter: (event: PointerEvent) => void = () => {};
  export let onPointerLeave: (event: PointerEvent) => void = () => {};
  export let onFocusIn: () => void = () => {};
  export let onFocusOut: (event: FocusEvent) => void = () => {};

  let lane: HTMLElement;
  let editingId: string | null = null;
  let editValue = "";
  let drag: DragState | null = null;
  let preview: { id: string; startSeconds: number; endSeconds: number } | null = null;
  let editSuggestions: string[] = [];
  let editInvalid = false;
  let editSuggestionIndex = -1;
  let editSuggestionSelect: HTMLSelectElement | undefined;
  let editOptionsOverlay: HTMLElement | undefined;
  let editWheelAccumulator = 0;
  let contextMenu: { id: string; x: number; y: number } | null = null;
  let contextDeleteButton: HTMLButtonElement | undefined;

  $: viewportEnd = viewportStart + 1 / Math.max(1, viewportZoom);
  $: visibleItems = items.flatMap((item) => {
    if (!(durationSeconds > 0) || item.endSeconds <= item.startSeconds) return [];
    const startRatio = item.startSeconds / durationSeconds;
    const endRatio = item.endSeconds / durationSeconds;
    if (endRatio <= viewportStart || startRatio >= viewportEnd) return [];
    const shown = preview?.id === item.id ? { ...item, ...preview } : item;
    const shownStart = shown.startSeconds / durationSeconds;
    const shownEnd = shown.endSeconds / durationSeconds;
    const clippedStart = Math.max(viewportStart, shownStart);
    const clippedEnd = Math.min(viewportEnd, shownEnd);
    return [{
      ...shown,
      leftPercent: (clippedStart - viewportStart) * viewportZoom * 100,
      widthPercent: Math.max(0, (clippedEnd - clippedStart) * viewportZoom * 100),
      startVisible: shownStart >= viewportStart,
      endVisible: shownEnd <= viewportEnd,
    }];
  });

  function focusInput(node: HTMLInputElement): void {
    queueMicrotask(() => { node.focus(); node.select(); });
  }

  function select(item: TimelineLaneItem): void {
    onSelect(item.id);
    onSeek(item.startSeconds);
  }

  function beginEdit(item: TimelineLaneItem): void {
    onSelect(item.id);
    onEditStart(item.id);
    editingId = item.id;
    editValue = item.label;
    editInvalid = false;
    const availableOptions = options ?? [];
    editSuggestions = availableOptions.includes(item.label)
      ? availableOptions
      : [item.label, ...availableOptions];
    editSuggestionIndex = editSuggestions.findIndex((suggestion) => suggestion === item.label);
    editWheelAccumulator = 0;
  }

  function commitEdit(replaceAll = false): void {
    if (!editingId) return;
    if (!onRename(editingId, editValue.trim(), replaceAll)) {
      editInvalid = true;
      return;
    }
    editingId = null;
    editInvalid = false;
    editSuggestions = [];
  }

  function handleEditBlur(event: FocusEvent): void {
    const block = (event.currentTarget as HTMLElement).closest(".timeline-block");
    const blurredEditingId = editingId;
    requestAnimationFrame(() => {
      if (editingId !== blurredEditingId) return;
      if (!block?.contains(document.activeElement) && !editOptionsOverlay?.contains(document.activeElement)) commitEdit();
    });
  }

  function cancelEdit(): void {
    editingId = null;
    editInvalid = false;
    editSuggestions = [];
  }

  function refreshEditSuggestions(): void {
    editSuggestions = suggestions?.(editValue) ?? [];
    editSuggestionIndex = -1;
    editInvalid = false;
  }

  function stepEditSuggestion(direction: -1 | 1): void {
    if (!editSuggestions.length) refreshEditSuggestions();
    if (!editSuggestions.length) return;
    const currentIndex = editSuggestions.findIndex((suggestion) => suggestion === editValue);
    const anchor = currentIndex >= 0 ? currentIndex : editSuggestionIndex;
    editSuggestionIndex = Math.max(0, Math.min(
      editSuggestions.length - 1,
      anchor < 0 ? (direction > 0 ? 0 : editSuggestions.length - 1) : anchor + direction,
    ));
    editValue = editSuggestions[editSuggestionIndex] ?? editValue;
    editInvalid = false;
    queueMicrotask(() => editSuggestionSelect?.querySelector("option:checked")?.scrollIntoView({ block: "nearest" }));
  }

  function handleEditKeydown(event: KeyboardEvent): void {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      stepEditSuggestion(event.key === "ArrowDown" ? 1 : -1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      commitEdit(event.shiftKey);
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      cancelEdit();
    }
  }

  function chooseEditSuggestion(event: Event): void {
    const select = event.currentTarget as HTMLSelectElement;
    editValue = select.value;
    editSuggestionIndex = select.selectedIndex;
    editInvalid = false;
  }

  function validateEditSuggestion(event: MouseEvent): void {
    const action = chordEditPointerAction("option", event.button, event.shiftKey);
    if (action !== "commit" && action !== "commitAll") return;
    event.stopPropagation();
    chooseEditSuggestion(event);
    commitEdit(action === "commitAll");
  }

  function handleEditWheel(event: WheelEvent): void {
    event.preventDefault();
    event.stopPropagation();
    if (event.shiftKey) {
      commitEdit(true);
      return;
    }
    const delta = event.deltaY !== 0 ? event.deltaY : event.deltaX;
    editWheelAccumulator += delta;
    if (Math.abs(editWheelAccumulator) < 16) return;
    stepEditSuggestion(editWheelAccumulator > 0 ? 1 : -1);
    editWheelAccumulator = 0;
  }

  function editOptionsPortal(node: HTMLSelectElement): { destroy: () => void } {
    const block = node.closest<HTMLElement>(".timeline-block");
    const overlay = document.createElement("div");
    let positionFrame: number | undefined;
    overlay.className = "timeline-edit-options-overlay";
    overlay.appendChild(node);
    document.body.appendChild(overlay);
    editOptionsOverlay = overlay;
    const position = (): void => {
      if (positionFrame !== undefined) return;
      positionFrame = requestAnimationFrame(() => {
        positionFrame = undefined;
        if (!block) return;
        const bounds = block.getBoundingClientRect();
        const width = Math.min(Math.max(180, bounds.width), window.innerWidth - 16);
        const left = Math.max(8, Math.min(bounds.left, window.innerWidth - width - 8));
        const height = node.offsetHeight;
        const below = bounds.bottom + 6;
        const top = below + height <= window.innerHeight - 8
          ? below
          : Math.max(8, bounds.top - height - 6);
        overlay.style.left = `${left}px`;
        overlay.style.top = `${top}px`;
        overlay.style.width = `${width}px`;
        overlay.style.height = `${height}px`;
        node.style.width = `${width}px`;
      });
    };
    const outside = (event: PointerEvent): void => {
      const target = event.target;
      if (target instanceof Node && (block?.contains(target) || overlay.contains(target))) return;
      if (chordEditPointerAction("outside", event.button, event.shiftKey) === "cancel") cancelEdit();
    };
    const initialFrame = requestAnimationFrame(() => {
      position();
      if (editSuggestionIndex < 0) return;
      node.selectedIndex = editSuggestionIndex;
      node.scrollTop = centeredChordOptionScrollTop(editSuggestionIndex, node.options.length, node.clientHeight, node.scrollHeight);
    });
    node.addEventListener("wheel", handleEditWheel, { passive: false });
    window.addEventListener("resize", position);
    window.addEventListener("scroll", position, true);
    document.addEventListener("pointerdown", outside, true);
    return { destroy: () => {
      cancelAnimationFrame(initialFrame);
      if (positionFrame !== undefined) cancelAnimationFrame(positionFrame);
      node.removeEventListener("wheel", handleEditWheel);
      window.removeEventListener("resize", position);
      window.removeEventListener("scroll", position, true);
      document.removeEventListener("pointerdown", outside, true);
      if (editOptionsOverlay === overlay) editOptionsOverlay = undefined;
      overlay.remove();
    } };
  }

  function handleWindowKey(event: KeyboardEvent): void {
    if (event.key === "Escape" && contextMenu) {
      event.preventDefault();
      contextMenu = null;
      return;
    }
    if (!selectedId || editingId || event.key !== "Delete" && event.key !== "Backspace") return;
    const target = event.target;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement || (target instanceof HTMLElement && target.isContentEditable)) return;
    if (!items.some((item) => item.id === selectedId)) return;
    event.preventDefault();
    onDelete(selectedId);
    onSelect(null);
  }

  function startResize(event: PointerEvent, item: TimelineLaneItem, edge: "start" | "end"): void {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    onSelect(item.id);
    drag = { id: item.id, edge, pointerId: event.pointerId, startSeconds: item.startSeconds, endSeconds: item.endSeconds };
    preview = { id: item.id, startSeconds: item.startSeconds, endSeconds: item.endSeconds };
    event.currentTarget instanceof HTMLElement && event.currentTarget.setPointerCapture(event.pointerId);
  }

  function moveResize(event: PointerEvent): void {
    if (!drag || drag.pointerId !== event.pointerId || !lane || !(durationSeconds > 0)) return;
    const bounds = lane.getBoundingClientRect();
    const ratio = viewportStart + Math.max(0, Math.min(1, (event.clientX - bounds.left) / Math.max(1, bounds.width))) / viewportZoom;
    const rawSeconds = Math.max(0, Math.min(durationSeconds, ratio * durationSeconds));
    const snapToleranceSeconds = durationSeconds / Math.max(1, viewportZoom) * 10 / Math.max(1, bounds.width);
    const seconds = event.shiftKey
      ? nearestTimelineSnapPosition(rawSeconds, snapPoints, snapToleranceSeconds)
      : rawSeconds;
    const minimum = Math.min(0.05, durationSeconds);
    preview = drag.edge === "start"
      ? { id: drag.id, startSeconds: Math.min(seconds, drag.endSeconds - minimum), endSeconds: drag.endSeconds }
      : { id: drag.id, startSeconds: drag.startSeconds, endSeconds: Math.max(seconds, drag.startSeconds + minimum) };
  }

  function finishResize(event: PointerEvent): void {
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (preview) onResize(preview.id, preview.startSeconds, preview.endSeconds);
    drag = null;
    preview = null;
  }

  function snapEdge(event: MouseEvent, item: TimelineLaneItem, edge: "start" | "end"): void {
    event.preventDefault();
    event.stopPropagation();
    const minimum = Math.min(0.05, durationSeconds);
    const snapped = snappedTimelineRange(items, item.id, edge, durationSeconds, minimum);
    if (snapped) onResize(item.id, snapped.startSeconds, snapped.endSeconds);
  }

  function openContextMenu(event: MouseEvent, item: TimelineLaneItem): void {
    event.preventDefault();
    event.stopPropagation();
    onSelect(item.id);
    contextMenu = {
      id: item.id,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - 196)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - 48)),
    };
    queueMicrotask(() => contextDeleteButton?.focus());
  }

  function deleteFromContextMenu(): void {
    if (!contextMenu) return;
    onDelete(contextMenu.id);
    onSelect(null);
    contextMenu = null;
  }
</script>

<svelte:window onkeydown={handleWindowKey} onpointerdown={() => contextMenu = null} onblur={() => contextMenu = null} />

<div class="timeline-lane-row timeline-axis-row" class:dragging={Boolean(drag)}>
  <span class="timeline-lane-icon" role="img" aria-label={label} data-tooltip={label}><Icon name={icon} size="13px" /></span>
  <div
    class="timeline-lane"
    bind:this={lane}
    role="group"
    aria-label={label}
    onwheel={onWheel}
    onpointerenter={onPointerEnter}
    onpointerleave={onPointerLeave}
    onfocusin={onFocusIn}
    onfocusout={onFocusOut}
  >
    {#each visibleItems as item (item.id)}
      <div
        class="timeline-block"
        class:selected={item.id === selectedId}
        class:active={item.active}
        class:muted={item.muted}
        role="group"
        aria-label={item.label}
        style={`left:${item.leftPercent}%;width:${item.widthPercent}%;--block-color:${item.color ?? "var(--accent)"}`}
        title={`${item.label} · ${item.startSeconds.toFixed(2)}–${item.endSeconds.toFixed(2)} s · ${deleteLabel}`}
        oncontextmenu={(event) => openContextMenu(event, item)}
      >
        {#if item.startVisible}<button class="resize-handle start" type="button" tabindex="-1" aria-label={`${label}, ${item.label}, ${startLabel}`} onpointerdown={(event) => startResize(event, item, "start")} onpointermove={moveResize} onpointerup={finishResize} onpointercancel={finishResize} ondblclick={(event) => snapEdge(event, item, "start")}></button>{/if}
        {#if editingId === item.id}
          <input
            use:focusInput
            bind:value={editValue}
            maxlength="120"
            spellcheck={options ? "false" : undefined}
            aria-label={`${editLabel}: ${item.label}`}
            aria-invalid={editInvalid}
            onclick={(event) => event.stopPropagation()}
            ondblclick={(event) => event.stopPropagation()}
            oncontextmenu={(event) => event.stopPropagation()}
            onblur={handleEditBlur}
            oninput={refreshEditSuggestions}
            onkeydown={handleEditKeydown}
          />
          {#if editSuggestions.length}
            <select
              class="edit-suggestions"
              size={Math.min(7, editSuggestions.length)}
              bind:this={editSuggestionSelect}
              use:editOptionsPortal
              aria-label={editLabel}
              value={editValue}
              oncontextmenu={(event) => event.stopPropagation()}
              onchange={chooseEditSuggestion}
              onclick={validateEditSuggestion}
              onblur={handleEditBlur}
              onkeydown={handleEditKeydown}
            >
              {#each editSuggestions as suggestion}<option value={suggestion}>{formatOption(suggestion)}</option>{/each}
            </select>
          {/if}
        {:else}
          <button
            class="block-content"
            type="button"
            aria-current={item.active ? "true" : undefined}
            aria-label={`${item.label}. ${editLabel}. ${deleteLabel}`}
            onclick={() => select(item)}
            ondblclick={(event) => { event.stopPropagation(); beginEdit(item); }}
            onkeydown={(event) => {
              if (event.key === "Enter") { event.preventDefault(); beginEdit(item); }
              else if (event.key === "Delete" || event.key === "Backspace") {
                event.preventDefault();
                onDelete(item.id);
                onSelect(null);
              }
            }}
          ><span>{item.label}</span></button>
        {/if}
        {#if item.endVisible}<button class="resize-handle end" type="button" tabindex="-1" aria-label={`${label}, ${item.label}, ${endLabel}`} onpointerdown={(event) => startResize(event, item, "end")} onpointermove={moveResize} onpointerup={finishResize} onpointercancel={finishResize} ondblclick={(event) => snapEdge(event, item, "end")}></button>{/if}
      </div>
    {/each}
  </div>
  <button class="timeline-lane-add" type="button" aria-label={addLabel} data-tooltip={addLabel} onclick={onAdd} disabled={!(durationSeconds > 0)}><Icon name="plus" size="11px" /></button>
</div>

{#if contextMenu}
  <div class="timeline-context-menu" role="menu" tabindex="-1" style={`left:${contextMenu.x}px;top:${contextMenu.y}px`} onpointerdown={(event) => event.stopPropagation()}>
    <button bind:this={contextDeleteButton} type="button" role="menuitem" onclick={deleteFromContextMenu}>{deleteLabel}</button>
  </div>
{/if}

<style>
  .timeline-lane-row { align-items: center; min-width: 0; }
  .timeline-lane-icon { display: grid; width: 25px; height: 30px; place-items: center; color: var(--muted); }
  .timeline-lane-add { display: grid; width: 25px; min-width: 25px; height: 25px; padding: 0; place-items: center; border-radius: 5px; color: var(--muted); }
  .timeline-lane-add:hover:not(:disabled), .timeline-lane-add:focus-visible { color: var(--accent-strong); }
  .timeline-lane { position: relative; height: 32px; min-width: 0; overflow: hidden; border: 1px solid var(--border); border-radius: 6px; background: color-mix(in srgb, var(--surface-deep) 88%, transparent); user-select: none; }
  .timeline-block { position: absolute; z-index: 1; top: 3px; bottom: 3px; min-width: 3px; overflow: hidden; border: 1px solid color-mix(in srgb, var(--block-color) 62%, var(--border)); border-radius: 4px; color: var(--text-strong); background: color-mix(in srgb, var(--block-color) 18%, var(--surface-raised)); box-shadow: inset 0 -2px 0 color-mix(in srgb, var(--block-color) 38%, transparent); cursor: pointer; }
  .block-content { display: block; width: 100%; height: 100%; overflow: hidden; padding: 0 8px; border: 0; border-radius: 3px; color: inherit; background: transparent; font-size: .6rem; font-weight: 780; line-height: 22px; text-align: center; text-overflow: ellipsis; white-space: nowrap; }
  .block-content span { display: block; overflow: hidden; text-overflow: ellipsis; }
  .timeline-block:hover, .timeline-block:focus-within { z-index: 3; border-color: var(--block-color); }
  .block-content:focus-visible { outline: 1px solid var(--block-color); outline-offset: -2px; }
  .timeline-block.selected { z-index: 4; outline: 2px solid var(--accent); outline-offset: -2px; }
  .timeline-block.active { box-shadow: inset 0 -2px 0 var(--accent), 0 0 5px color-mix(in srgb, var(--accent) 28%, transparent); }
  .timeline-block.muted { color: var(--muted); background: transparent; box-shadow: none; }
  .timeline-block input { position: absolute; z-index: 5; inset: 1px 5px; width: calc(100% - 10px); min-width: 0; height: 20px; padding: 0 4px; border: 1px solid var(--accent); border-radius: 3px; background: var(--surface-raised); color: var(--text-strong); font-size: .62rem; text-align: center; }
  .timeline-block input[aria-invalid="true"] { border-color: var(--danger); box-shadow: 0 0 0 1px color-mix(in srgb, var(--danger) 45%, transparent); }
  :global(.timeline-edit-options-overlay) { position: fixed; z-index: 9000; overflow: hidden; pointer-events: auto; }
  :global(.timeline-edit-options-overlay .edit-suggestions) { position: absolute; inset: 0 auto auto 0; min-width: 0; max-height: 132px; padding: 2px; border: 1px solid var(--accent-border); border-radius: 4px; color: var(--text); background: var(--surface-deep); box-shadow: var(--shadow-menu); font-size: .7rem; font-weight: 750; pointer-events: auto; }
  :global(.timeline-edit-options-overlay .edit-suggestions option) { padding: 3px 5px; border-radius: 3px; }
  .timeline-context-menu { position: fixed; z-index: 9100; padding: 4px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface-raised); box-shadow: var(--shadow-menu); }
  .timeline-context-menu button { width: 180px; padding: 8px 10px; border: 0; color: var(--danger); background: transparent; text-align: left; }
  .timeline-context-menu button:hover, .timeline-context-menu button:focus-visible { color: var(--text); background: var(--danger-soft); }
  .resize-handle { position: absolute; z-index: 6; top: 0; bottom: 0; width: 7px; min-width: 7px; height: 100%; padding: 0; border: 0; border-radius: 0; opacity: 0; background: var(--accent); cursor: ew-resize; touch-action: none; }
  .resize-handle.start { left: 0; }
  .resize-handle.end { right: 0; }
  .timeline-block:hover .resize-handle, .timeline-block.selected .resize-handle, .resize-handle:active { opacity: .78; }
  .timeline-lane-row.dragging, .timeline-lane-row.dragging * { cursor: ew-resize !important; }
  @media (prefers-reduced-motion: reduce) { .resize-handle { transition: none; } }
</style>
