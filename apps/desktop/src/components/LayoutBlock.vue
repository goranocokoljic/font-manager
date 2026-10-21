<script setup lang="ts">
// One text block of a body-text layout, set as live text in the webview: the
// block's size (pt), line height, tracking (thousandths of an em), paragraph
// spacing, first-line indent, columns and alignment map onto CSS. Columns are
// balanced by the browser (`column-fill: balance`). A wrap block breaks its
// paragraphs into lines; a clip block keeps one line per paragraph and hides
// what does not fit. The values arrive clamped by the core.
//
// Direct manipulation (E12.2): a horizontal drag changes the size, a vertical
// drag the line height, a trackpad pinch (Ctrl+wheel in WebView2, gesture
// events in WebKit) scales the size, and the arrow keys do both from the
// keyboard. While a gesture runs, only this block's style changes, once per
// animation frame; the font-family stays the same, so the face is not loaded
// again. The result is emitted once, when the gesture ends; Escape during a
// drag puts the starting values back. Double-click (or Enter) edits the text
// in place: Escape cancels, Ctrl/Cmd+Enter or a click outside keeps it.
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import type { LayoutBounds, TextBlock } from '../api/types';
import type { BlockEdit } from '../stores/textLayouts';
import {
  DRAG_THRESHOLD_PX,
  dragSize,
  keySize,
  pinchSize,
  sizeLabel,
  wheelScale,
  type BlockSize,
} from '../textLayout/gesture';

const props = defineProps<{
  block: TextBlock;
  /** The CSS `font-family` of the face the block is set in. */
  fontFamily: string;
  /** The core's ranges and steps; null while they load (the block is then read-only). */
  bounds?: LayoutBounds | null;
  /** The block's position in the layout, for its accessible name. */
  index?: number;
  /** Id of the element that explains the gestures and keys. */
  describedby?: string;
}>();

const emit = defineEmits<{ change: [edit: BlockEdit] }>();

/** How long a pinch rests before its size is committed. */
const PINCH_COMMIT_MS = 200;

const root = ref<HTMLElement | null>(null);
const field = ref<HTMLTextAreaElement | null>(null);

/** The values while a gesture runs; null shows the block's own. */
const live = ref<BlockSize | null>(null);
const shown = computed<BlockSize>(
  () =>
    live.value ?? {
      // The core sends clamped numbers; NaN (null) cannot reach a block.
      sizePt: props.block.sizePt ?? Number.NaN,
      lineHeight: props.block.lineHeight ?? Number.NaN,
    },
);

const clip = computed(() => props.block.overflow === 'clip');

const blockStyle = computed(() => ({
  fontFamily: props.fontFamily,
  fontSize: `${shown.value.sizePt}pt`,
  lineHeight: String(shown.value.lineHeight),
  letterSpacing: `${(props.block.tracking ?? 0) / 1000}em`,
  textAlign: props.block.align,
  columnCount: props.block.columns > 1 ? props.block.columns : undefined,
}));

const paragraphStyle = computed(() => ({
  marginBottom: `${props.block.paragraphSpacingPt}pt`,
  textIndent: props.block.firstLineIndentEm ? `${props.block.firstLineIndentEm}em` : undefined,
}));

const paragraphs = computed(() => props.block.text.split('\n'));
const badge = computed(() => sizeLabel(shown.value));
const label = computed(
  () =>
    `Text block ${(props.index ?? 0) + 1}, ${shown.value.sizePt} points, line height ${shown.value.lineHeight.toFixed(2)}`,
);

// ---- frame-paced updates -------------------------------------------------

let frame = 0;
let next: BlockSize | null = null;

/** Shows `values` on the next animation frame (the latest one wins). */
function schedule(values: BlockSize): void {
  next = values;
  frame ||= requestAnimationFrame(() => {
    frame = 0;
    if (next) live.value = next;
  });
}

function stopFrame(): void {
  if (frame) cancelAnimationFrame(frame);
  frame = 0;
}

/** Ends a gesture: emits the final values when they differ, then shows the block's own. */
function commit(values: BlockSize | null): void {
  stopFrame();
  next = null;
  live.value = null;
  if (!values) return;
  const edit: BlockEdit = {};
  if (values.sizePt !== props.block.sizePt) edit.sizePt = values.sizePt;
  if (values.lineHeight !== props.block.lineHeight) edit.lineHeight = values.lineHeight;
  if (Object.keys(edit).length) emit('change', edit);
}

// ---- drag ------------------------------------------------------------------

interface Drag {
  x: number;
  y: number;
  pointerId: number;
  start: BlockSize;
  moved: boolean;
  last: BlockSize;
}
let drag: Drag | null = null;

function onPointerDown(e: PointerEvent): void {
  if (e.button !== 0 || editing.value || !props.bounds || drag) return;
  const start = { ...shown.value };
  drag = { x: e.clientX, y: e.clientY, pointerId: e.pointerId, start, moved: false, last: start };
  root.value?.setPointerCapture?.(e.pointerId);
  window.addEventListener('keydown', onDragKey, true);
}

function onPointerMove(e: PointerEvent): void {
  if (!drag || e.pointerId !== drag.pointerId || !props.bounds) return;
  const dx = e.clientX - drag.x;
  const dy = e.clientY - drag.y;
  if (!drag.moved && Math.abs(dx) + Math.abs(dy) < DRAG_THRESHOLD_PX) return;
  drag.moved = true;
  drag.last = dragSize(drag.start, dx, dy, props.bounds);
  schedule(drag.last);
}

function endDrag(keep: boolean): void {
  if (!drag) return;
  const d = drag;
  drag = null;
  window.removeEventListener('keydown', onDragKey, true);
  if (root.value?.hasPointerCapture?.(d.pointerId)) root.value.releasePointerCapture(d.pointerId);
  commit(keep && d.moved ? d.last : null);
}

function onPointerUp(e: PointerEvent): void {
  if (drag && e.pointerId === drag.pointerId) endDrag(true);
}

/** Escape during a drag puts the starting values back. */
function onDragKey(e: KeyboardEvent): void {
  if (e.key !== 'Escape' || !drag) return;
  e.preventDefault();
  e.stopPropagation();
  endDrag(false);
}

// ---- pinch -----------------------------------------------------------------

let pinchTimer: ReturnType<typeof setTimeout> | undefined;
let pinchStart: BlockSize | null = null;
let pinchLast: BlockSize | null = null;

function pinchTo(values: BlockSize): void {
  pinchLast = values;
  schedule(values);
}

function endPinch(): void {
  clearTimeout(pinchTimer);
  pinchTimer = undefined;
  const last = pinchLast;
  pinchStart = pinchLast = null;
  commit(last);
}

/** A trackpad pinch in Chromium (WebView2) arrives as a wheel event with Ctrl held. */
function onWheel(e: WheelEvent): void {
  if (!e.ctrlKey || !props.bounds || editing.value || drag) return;
  e.preventDefault();
  pinchTo(pinchSize(pinchLast ?? shown.value, wheelScale(e.deltaY), props.bounds));
  clearTimeout(pinchTimer);
  pinchTimer = setTimeout(endPinch, PINCH_COMMIT_MS);
}

/** WebKit (WKWebView) reports a pinch as gesture events with a running `scale`. */
interface GestureLike extends Event {
  scale: number;
}

function onGestureStart(e: Event): void {
  if (!props.bounds || editing.value || drag) return;
  e.preventDefault();
  pinchStart = { ...shown.value };
}

function onGestureChange(e: Event): void {
  if (!pinchStart || !props.bounds) return;
  e.preventDefault();
  pinchTo(pinchSize(pinchStart, (e as GestureLike).scale, props.bounds));
}

function onGestureEnd(e: Event): void {
  if (!pinchStart) return;
  e.preventDefault();
  endPinch();
}

const GESTURES: [string, (e: Event) => void][] = [
  ['gesturestart', onGestureStart],
  ['gesturechange', onGestureChange],
  ['gestureend', onGestureEnd],
];

onMounted(() => GESTURES.forEach(([n, f]) => root.value?.addEventListener(n, f)));
onBeforeUnmount(() => {
  GESTURES.forEach(([n, f]) => root.value?.removeEventListener(n, f));
  endDrag(false);
  clearTimeout(pinchTimer);
  stopFrame();
});

// ---- keyboard --------------------------------------------------------------

function onKeyDown(e: KeyboardEvent): void {
  if (editing.value || e.target !== root.value || e.altKey || e.ctrlKey || e.metaKey) return;
  if (e.key === 'Enter') {
    e.preventDefault();
    startEdit();
    return;
  }
  if (!props.bounds) return;
  const values = keySize(shown.value, e.key, e.shiftKey, props.bounds);
  if (!values) return;
  e.preventDefault();
  commit(values);
}

// ---- text editing ----------------------------------------------------------

const editing = ref(false);
const draft = ref('');

async function startEdit(): Promise<void> {
  if (editing.value) return;
  endDrag(false);
  draft.value = props.block.text;
  editing.value = true;
  await nextTick();
  field.value?.focus();
  field.value?.select();
}

function finishEdit(keep: boolean): void {
  if (!editing.value) return;
  editing.value = false;
  if (keep && draft.value !== props.block.text) emit('change', { text: draft.value });
  void nextTick(() => root.value?.focus({ preventScroll: true }));
}

function onFieldKey(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    finishEdit(false);
  } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    e.stopPropagation();
    finishEdit(true);
  }
}
</script>

<template>
  <div
    ref="root"
    class="block"
    :class="{ clip, editing, live: live !== null }"
    data-testid="layout-block"
    role="group"
    aria-roledescription="resizable text block"
    tabindex="0"
    :aria-label="label"
    :aria-describedby="describedby"
    :data-overflow="block.overflow"
    :data-columns="block.columns"
    :data-size="shown.sizePt"
    :data-line-height="shown.lineHeight"
    :style="blockStyle"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="endDrag(false)"
    @wheel="onWheel"
    @dblclick="startEdit"
    @keydown="onKeyDown"
  >
    <span class="badge" data-testid="size-badge" aria-hidden="true">{{ badge }}</span>
    <textarea
      v-if="editing"
      ref="field"
      v-model="draft"
      class="field"
      aria-label="Block text"
      :rows="Math.max(1, draft.split('\n').length)"
      @keydown="onFieldKey"
      @blur="finishEdit(true)"
    />
    <template v-else>
      <p v-for="(text, i) in paragraphs" :key="i" class="paragraph" :style="paragraphStyle">
        {{ text }}
      </p>
    </template>
  </div>
</template>

<style scoped>
.block {
  position: relative;
  column-gap: 2em;
  column-fill: balance;
  overflow-wrap: break-word;
  cursor: move;
  touch-action: none;
  user-select: none;
  border-radius: 2px;
}
.block:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.block.editing {
  cursor: auto;
}
.paragraph {
  margin-top: 0;
}
.block.clip {
  overflow: hidden;
}
.block.clip .paragraph {
  overflow: hidden;
  white-space: nowrap;
}
/* The size badge shows while the block is hovered, focused or being resized. */
.badge {
  position: absolute;
  top: 0;
  right: 0;
  z-index: 1;
  padding: 1px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  background: var(--fm-surface);
  color: var(--fm-fg);
  font:
    11px/1.4 system-ui,
    sans-serif;
  letter-spacing: normal;
  text-indent: 0;
  white-space: nowrap;
  pointer-events: none;
  opacity: 0;
}
.block:hover .badge,
.block:focus-visible .badge,
.block.live .badge {
  opacity: 1;
}
.field {
  display: block;
  column-span: all;
  box-sizing: border-box;
  width: 100%;
  margin: 0;
  padding: 0;
  border: 1px solid var(--fm-control-border);
  background: var(--fm-bg);
  color: inherit;
  font: inherit;
  letter-spacing: inherit;
  line-height: inherit;
  text-align: inherit;
  resize: none;
  user-select: text;
}
</style>
