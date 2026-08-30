<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import type { ToastMessage } from "./toasts";

  let { toast, durationMs, closeLabel, dismiss }: {
    toast: ToastMessage;
    durationMs: number;
    closeLabel: string;
    dismiss: (id: number) => void;
  } = $props();

  let timer: number | undefined;
  let remainingMs = 0;
  let startedAt = 0;
  let hovered = false;
  let focused = false;
  let visible = $state(false);
  let exiting = $state(false);
  let entranceFrame: number | undefined;
  let removalTimer: number | undefined;

  function requestDismiss(): void {
    if (exiting) return;
    exiting = true;
    pauseTimer();
    removalTimer = window.setTimeout(() => dismiss(toast.id), 180);
  }

  function startTimer(): void {
    window.clearTimeout(timer);
    startedAt = performance.now();
    timer = window.setTimeout(requestDismiss, remainingMs);
  }

  function pauseTimer(): void {
    if (timer === undefined) return;
    window.clearTimeout(timer);
    timer = undefined;
    remainingMs = Math.max(0, remainingMs - (performance.now() - startedAt));
  }

  function resumeTimer(): void {
    if (exiting || timer !== undefined || hovered || focused) return;
    if (remainingMs <= 0) requestDismiss();
    else startTimer();
  }

  function pointerEntered(): void {
    hovered = true;
    pauseTimer();
  }

  function pointerLeft(): void {
    hovered = false;
    resumeTimer();
  }

  function focusEntered(): void {
    focused = true;
    pauseTimer();
  }

  function focusLeft(): void {
    focused = false;
    resumeTimer();
  }

  onMount(() => {
    remainingMs = durationMs;
    entranceFrame = window.requestAnimationFrame(() => visible = true);
    startTimer();
    return () => {
      if (entranceFrame !== undefined) window.cancelAnimationFrame(entranceFrame);
      window.clearTimeout(timer);
      window.clearTimeout(removalTimer);
    };
  });
</script>

<article
  class={`toast ${toast.level}`}
  class:visible
  class:exiting
  role={toast.level === "warn" || toast.level === "error" ? "alert" : "status"}
  onpointerenter={pointerEntered}
  onpointerleave={pointerLeft}
  onfocusin={focusEntered}
  onfocusout={focusLeft}
>
  <span class="toast-icon" aria-hidden="true">
    {#if toast.level === "success"}<Icon name="check" size="10px" />
    {:else if toast.level === "error"}<Icon name="xmark" size="9px" />
    {:else}<b>{toast.level === "info" ? "i" : "!"}</b>{/if}
  </span>
  <div class="toast-content">
    <strong>{toast.title}</strong>
    {#if toast.detail}<p>{toast.detail}</p>{/if}
  </div>
  <button aria-label={closeLabel} onclick={requestDismiss}><Icon name="xmark" size="11px" /></button>
</article>
