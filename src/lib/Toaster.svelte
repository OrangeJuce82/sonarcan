<script lang="ts">
  import ToastItem from "./ToastItem.svelte";
  import type { ToastMessage } from "./toasts";

  let { toasts, durationMs, closeLabel, notificationsLabel, dismiss }: {
    toasts: ToastMessage[];
    durationMs: number;
    closeLabel: string;
    notificationsLabel: string;
    dismiss: (id: number) => void;
  } = $props();
  let stack: HTMLElement;

  $effect(() => {
    const shouldOpen = toasts.length > 0;
    queueMicrotask(() => {
      if (!stack || typeof stack.showPopover !== "function") return;
      const isOpen = stack.matches(":popover-open");
      if (!shouldOpen && isOpen) stack.hidePopover();
      else if (shouldOpen) {
        if (isOpen) stack.hidePopover();
        stack.showPopover();
      }
    });
  });
</script>

<section bind:this={stack} class="toast-stack" aria-label={notificationsLabel} popover="manual">
  {#each toasts as toast (toast.id)}
    <ToastItem {toast} {durationMs} {closeLabel} {dismiss} />
  {/each}
</section>
