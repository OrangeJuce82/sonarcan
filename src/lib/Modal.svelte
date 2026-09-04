<script lang="ts">
  import Icon from "./Icon.svelte";
  import { isPointOutsideModal, shouldDismissModalFromBackdrop } from "./modalInteraction";
  import { onMount } from "svelte";
  let { title, closeLabel, close, children, titleContent, headerActions, wide = false, keydown }: { title: string; closeLabel: string; close: () => void; children: import("svelte").Snippet; titleContent?: import("svelte").Snippet; headerActions?: import("svelte").Snippet; wide?: boolean; keydown?: (event: KeyboardEvent) => void } = $props();
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
    <header><h2>{#if titleContent}{@render titleContent()}{:else}{title}{/if}</h2><div class="modal-header-actions">{#if headerActions}{@render headerActions()}{/if}<button class="modal-close" aria-label={closeLabel} onclick={close}><Icon name="xmark" size="12px" /></button></div></header>
  <div class="modal-content">{@render children()}</div>
</dialog>
