<script lang="ts">
  import Icon from "./Icon.svelte";
  import { isPointOutsideModal, shouldDismissModalFromBackdrop } from "./modalInteraction";
  import { onMount } from "svelte";
  let { title, closeLabel, close, children, wide = false, keydown }: { title: string; closeLabel: string; close: () => void; children: import("svelte").Snippet; wide?: boolean; keydown?: (event: KeyboardEvent) => void } = $props();
  let dialog: HTMLDialogElement;
  let pointerStartedOnBackdrop = false;
  onMount(() => { if (!dialog.open) dialog.showModal(); return () => { if (dialog.open) dialog.close(); }; });
  function cancel(event: Event): void { event.preventDefault(); close(); }
  function rememberPointerStart(event: PointerEvent): void {
    pointerStartedOnBackdrop = isPointOutsideModal(event, dialog);
  }
  function backdrop(event: MouseEvent): void {
    const shouldClose = pointerStartedOnBackdrop && shouldDismissModalFromBackdrop(event, dialog);
    pointerStartedOnBackdrop = false;
    if (shouldClose) close();
  }
  function cancelPointer(): void { pointerStartedOnBackdrop = false; }
</script>

<svelte:window onpointerdown={rememberPointerStart} onpointercancel={cancelPointer} />
<dialog bind:this={dialog} class:wide class="app-modal" aria-label={title} oncancel={cancel} onclick={backdrop} onkeydown={keydown}>
    <header><h2>{title}</h2><button class="modal-close" aria-label={closeLabel} onclick={close}><Icon name="xmark" size="12px" /></button></header>
  <div class="modal-content">{@render children()}</div>
</dialog>
