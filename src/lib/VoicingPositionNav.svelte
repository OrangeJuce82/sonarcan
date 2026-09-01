<script lang="ts">
  export let label: string;
  export let current: number;
  export let total: number;
  export let onPrevious: () => void;
  export let onNext: () => void;
</script>

<div class="voicing-position-nav" aria-label={`${label} ${current}/${total}`}>
  <button
    type="button"
    disabled={total < 2}
    aria-label={`‹ ${label}`}
    onclick={onPrevious}
  >‹</button>
  <strong aria-hidden="true">{current}/{total}</strong>
  <button
    type="button"
    disabled={total < 2}
    aria-label={`${label} ›`}
    onclick={onNext}
  >›</button>
</div>

<style>
  .voicing-position-nav { display: flex; align-items: center; justify-content: center; gap: 6px; min-height: 26px; }
  strong { box-sizing: border-box; display: grid; place-items: center; min-width: 46px; height: 24px; padding-inline: 8px; border: 1px solid var(--border); border-radius: 4px; color: var(--text); background: var(--surface-raised); font-size: .65rem; line-height: 1; text-align: center; }
  button { display: grid; place-items: center; width: 27px; min-width: 27px; height: 24px; padding: 0; border-color: var(--border); border-radius: 4px; color: var(--text); background: var(--surface-raised); font: 800 .9rem/1 sans-serif; }
  button:disabled { opacity: .35; }
</style>
