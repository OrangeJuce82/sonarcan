<script lang="ts">
  export let label: string;
  export let value: number;
  export let defaultValue: number;
  export let minimum: number;
  export let maximum: number;
  export let step: number;
  export let buttonStep: number = step;
  export let shiftButtonStep: number = step;
  export let display: (value: number) => string = (current) => String(current);
  export let onChange: (value: number) => void;
  export let onTap: (() => void) | undefined = undefined;
  export let tooltip = "";
  export let showStepButtons = true;

  let drag: { pointerId: number; y: number; value: number; moved: boolean } | null = null;
  let lastTapAt = 0;

  function normalized(next: number): number {
    const stepped = Math.round(next / step) * step;
    const precision = Math.max(0, (String(step).split(".")[1] ?? "").length);
    return Number(Math.max(minimum, Math.min(maximum, stepped)).toFixed(precision));
  }

  function increment(direction: number, incrementStep = step): void {
    const precision = Math.max(0, (String(incrementStep).split(".")[1] ?? "").length);
    const next = Math.max(minimum, Math.min(maximum, value + direction * incrementStep));
    onChange(Number(next.toFixed(precision)));
  }

  function buttonIncrement(event: MouseEvent, direction: number): void {
    increment(direction, event.shiftKey ? shiftButtonStep : buttonStep);
  }

  function reset(): void {
    onChange(normalized(defaultValue));
  }

  function startDrag(event: PointerEvent): void {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    drag = { pointerId: event.pointerId, y: event.clientY, value, moved: false };
  }

  function moveDrag(event: PointerEvent): void {
    if (!drag || drag.pointerId !== event.pointerId) return;
    const delta = drag.y - event.clientY;
    if (Math.abs(delta) >= 3) drag.moved = true;
    if (drag.moved) onChange(normalized(drag.value + delta / 6 * step));
  }

  function finishDrag(event: PointerEvent): void {
    if (!drag || drag.pointerId !== event.pointerId) return;
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    const wasTap = !drag.moved;
    drag = null;
    if (!wasTap || !onTap) return;
    const now = performance.now();
    if (now - lastTapAt < 180) reset();
    else onTap();
    lastTapAt = now;
  }

  function adjustWithWheel(event: WheelEvent): void {
    if (document.activeElement !== event.currentTarget) return;
    event.preventDefault();
    increment(event.deltaY < 0 ? 1 : -1);
  }
</script>

<div class="numeric-control" class:dragging={drag?.moved} data-tooltip={tooltip}>
  <span class="numeric-label" aria-hidden="true">{label}</span>
  <div class="numeric-buttons">
    {#if showStepButtons}<button type="button" class="step" aria-label={`${label} −`} onclick={(event) => buttonIncrement(event, -1)}><span aria-hidden="true">−</span></button>{/if}
    <button
      type="button"
      class="value"
      aria-label={label}
      onpointerdown={startDrag}
      onpointermove={moveDrag}
      onpointerup={finishDrag}
      onpointercancel={() => drag = null}
      onwheel={adjustWithWheel}
      ondblclick={onTap ? undefined : reset}
    ><strong>{display(value)}</strong></button>
    {#if showStepButtons}<button type="button" class="step" aria-label={`${label} +`} onclick={(event) => buttonIncrement(event, 1)}><span aria-hidden="true">+</span></button>{/if}
  </div>
</div>
