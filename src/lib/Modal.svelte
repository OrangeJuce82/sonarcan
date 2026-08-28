<script lang="ts">
  import Icon from "./Icon.svelte";
  import { onMount } from "svelte";
  let { title, close, children, wide = false }: { title: string; close: () => void; children: import("svelte").Snippet; wide?: boolean } = $props();
  let dialog: HTMLDialogElement;
  onMount(() => { if (!dialog.open) dialog.showModal(); return () => { if (dialog.open) dialog.close(); }; });
  function cancel(event: Event): void { event.preventDefault(); close(); }
  function backdrop(event: MouseEvent): void { if (event.target === dialog) close(); }
</script>

<dialog bind:this={dialog} class:wide class="app-modal" aria-label={title} oncancel={cancel} onclick={backdrop}>
    <header><h2>{title}</h2><button class="modal-close" aria-label="Close" onclick={close}><Icon name="xmark" size="12px" /></button></header>
  <div class="modal-content">{@render children()}</div>
</dialog>
