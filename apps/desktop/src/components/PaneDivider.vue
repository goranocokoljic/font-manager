<script setup lang="ts">
// A 1 px vertical divider that resizes the column next to it by pointer drag or
// arrow keys. `direction` is +1 when the column is on the left of the divider
// (moving right widens it) and -1 when it is on the right.
const props = defineProps<{
  label: string;
  controls: string;
  value: number;
  min: number;
  max: number;
  direction: 1 | -1;
}>();
const emit = defineEmits<{ resize: [px: number] }>();

/** Keyboard step: one unit of the 8 px grid. */
const STEP = 8;

let start: { x: number; value: number } | null = null;

function onPointerDown(e: PointerEvent): void {
  if (e.button !== 0) return;
  start = { x: e.clientX, value: props.value };
  (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
  e.preventDefault();
}

function onPointerMove(e: PointerEvent): void {
  if (!start) return;
  emit('resize', start.value + props.direction * (e.clientX - start.x));
}

function onPointerUp(e: PointerEvent): void {
  start = null;
  (e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId);
}

function onKeyDown(e: KeyboardEvent): void {
  const moves: Record<string, number> = {
    ArrowLeft: props.value - props.direction * STEP,
    ArrowRight: props.value + props.direction * STEP,
    Home: props.min,
    End: props.max,
  };
  if (!(e.key in moves)) return;
  e.preventDefault();
  emit('resize', moves[e.key]);
}
</script>

<template>
  <div
    class="divider"
    role="separator"
    tabindex="0"
    aria-orientation="vertical"
    :aria-label="label"
    :aria-controls="controls"
    :aria-valuenow="value"
    :aria-valuemin="min"
    :aria-valuemax="max"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerUp"
    @keydown="onKeyDown"
  />
</template>

<style scoped>
.divider {
  position: relative;
  flex: none;
  width: 1px;
  background: var(--fm-border);
  z-index: 1;
  cursor: col-resize;
  touch-action: none;
}
/* A wider invisible hit area so the 1 px hairline is easy to grab. */
.divider::before {
  content: '';
  position: absolute;
  inset: 0 -4px;
}
.divider:hover,
.divider:focus-visible {
  background: var(--fm-accent);
  outline: none;
}
</style>
