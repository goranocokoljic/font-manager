<script setup lang="ts">
// One text block of a body-text layout, set as live text in the webview: the
// block's size (pt), line height, tracking (thousandths of an em), paragraph
// spacing, first-line indent, columns and alignment map onto CSS. Columns are
// balanced by the browser (`column-fill: balance`). A wrap block breaks its
// paragraphs into lines; a clip block keeps one line per paragraph and hides
// what does not fit. The values arrive clamped by the core.
//
// Direct manipulation (E12.2), with `bounds`: a horizontal drag changes the size, a vertical
// drag the line height, Ctrl+wheel (a trackpad pinch in WebView2) and WebKit gesture events
// (a pinch in WKWebView) scale the size, and the arrow keys do both from the keyboard. While
// a gesture runs, only this block's style changes, once per animation frame; the
// font-family stays the same, so the face is not loaded again. The result is emitted once,
// as `change`, when the gesture ends; Escape during a drag, a cancelled pointer, a lost
// pointer capture or a button released without a pointerup puts the starting values back.
//
// An editable block opens its text in a box set in the same face on a double click or
// Enter; leaving the box (or Ctrl/Cmd+Enter) hands the text to the parent, Escape drops it.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { LayoutBounds, TextBlock } from '../api/types';
import { formatNumber, t } from '../i18n';
import type { BlockProps } from '../stores/textLayouts';
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
  /** The block's text can be edited; the parent gets it through `edit`. */
  editable?: boolean;
  /** The accessible name of the block and its text box ("Block 1"). */
  label?: string;
  /** The core's ranges and steps; without them the block cannot be resized. */
  bounds?: LayoutBounds | null;
  /** Id of the element that explains the gestures and keys. */
  describedby?: string;
}>();

const emit = defineEmits<{ edit: [text: string]; change: [change: BlockProps] }>();

/** How long a pinch rests before its size is committed. */
const PINCH_COMMIT_MS = 200;

const root = ref<HTMLElement | null>(null);
const field = ref<HTMLTextAreaElement | null>(null);

/** The values while a gesture runs; null shows the block's own. */
const live = ref<BlockSize | null>(null);
/** The values of the last committed gesture until the parent's block shows them. */
const held = ref<BlockSize | null>(null);
const own = computed<BlockSize>(() => ({
  // The core sends clamped numbers; NaN (null) cannot reach a block.
  sizePt: props.block.sizePt ?? Number.NaN,
  lineHeight: props.block.lineHeight ?? Number.NaN,
}));
const shown = computed<BlockSize>(() => live.value ?? held.value ?? own.value);

// The parent's answer (the core's clamped block) replaces the held values; another block
// (a layout switched or reverted) drops them.
watch(own, (b) => {
  const h = held.value;
  if (h && h.sizePt === b.sizePt && h.lineHeight === b.lineHeight) held.value = null;
});
watch(
  () => props.block,
  () => (held.value = null),
);

const resizable = computed(() => Boolean(props.bounds));
const focusable = computed(() => resizable.value || Boolean(props.editable));

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
const ariaLabel = computed(() => {
  if (!resizable.value) return props.editable ? props.label : undefined;
  const { sizePt, lineHeight } = shown.value;
  return t('blocks.ariaResizable', {
    label: props.label ?? t('blocks.block'),
    size: sizePt,
    lineHeight: formatNumber(lineHeight, { minimumFractionDigits: 2, maximumFractionDigits: 2 }),
  });
});

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
  const from = held.value ?? own.value;
  live.value = null;
  if (!values) return;
  const change: BlockProps = {};
  if (values.sizePt !== from.sizePt) change.sizePt = values.sizePt;
  if (values.lineHeight !== from.lineHeight) change.lineHeight = values.lineHeight;
  if (!Object.keys(change).length) return;
  held.value = values;
  emit('change', change);
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
  // The button came up where no pointerup reached the block: the drag ends as a
  // cancelled one does, with the starting values back.
  if ((e.buttons & 1) === 0) {
    endDrag(false);
    return;
  }
  const dx = e.clientX - drag.x;
  const dy = e.clientY - drag.y;
  if (!drag.moved && Math.abs(dx) + Math.abs(dy) < DRAG_THRESHOLD_PX) return;
  drag.moved = true;
  drag.last = dragSize(drag.start, dx, dy, props.bounds);
  schedule(drag.last);
}

/** Ends the drag; `keep` commits its last values, otherwise the starting ones come back. */
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

/**
 * A cancelled pointer, or a capture lost without a pointerup (the window lost focus, a
 * system gesture took the pointer), puts the starting values back.
 */
function onPointerLost(e: PointerEvent): void {
  if (drag && e.pointerId === drag.pointerId) endDrag(false);
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

/** Ctrl+wheel; a trackpad pinch in Chromium (WebView2) arrives as one too. */
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
    if (!props.editable || e.shiftKey) return;
    e.preventDefault();
    void startEdit();
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
const text = ref('');

async function startEdit(): Promise<void> {
  if (!props.editable || editing.value) return;
  endDrag(false);
  text.value = props.block.text;
  editing.value = true;
  await nextTick();
  field.value?.focus();
  field.value?.select();
}

function finish(keep: boolean): void {
  if (!editing.value) return;
  editing.value = false;
  if (keep && text.value !== props.block.text) emit('edit', text.value);
  void nextTick(() => root.value?.focus({ preventScroll: true }));
}

function onFieldKey(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    finish(false);
  } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    e.stopPropagation();
    finish(true);
  }
}
</script>

<template>
  <div class="host">
    <div
      ref="root"
      class="block"
      :class="{ clip, editing, resizable, editable, live: live !== null }"
      data-testid="layout-block"
      :data-overflow="block.overflow"
      :data-columns="block.columns"
      :data-size="shown.sizePt"
      :data-line-height="shown.lineHeight"
      :style="blockStyle"
      :tabindex="focusable ? 0 : undefined"
      :role="focusable ? 'group' : undefined"
      :aria-roledescription="resizable ? t('blocks.roleDescription') : undefined"
      :aria-label="ariaLabel"
      :aria-describedby="describedby"
      :title="editable ? t('blocks.editTitle') : undefined"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerLost"
      @lostpointercapture="onPointerLost"
      @wheel="onWheel"
      @dblclick="startEdit"
      @keydown="onKeyDown"
    >
      <textarea
        v-if="editing"
        ref="field"
        v-model="text"
        class="edit"
        dir="auto"
        data-testid="layout-block-editor"
        :aria-label="t('blocks.textLabel', { label: label ?? t('blocks.block') })"
        :rows="Math.max(2, text.split('\n').length)"
        :style="{ fontFamily, fontSize: `${shown.sizePt}pt`, lineHeight: String(shown.lineHeight) }"
        @keydown="onFieldKey"
        @blur="finish(true)"
      />
      <template v-else>
        <p
          v-for="(line, i) in paragraphs"
          :key="i"
          class="paragraph"
          dir="auto"
          :style="paragraphStyle"
        >
          {{ line }}
        </p>
      </template>
    </div>
    <span v-if="resizable" class="badge" data-testid="size-badge" aria-hidden="true">
      {{ badge }}
    </span>
  </div>
</template>

<style scoped>
.host {
  position: relative;
}
.block {
  column-gap: 2em;
  column-fill: balance;
  overflow-wrap: break-word;
  border-radius: 2px;
}
.block.resizable {
  cursor: move;
  touch-action: none;
  user-select: none;
}
.block.editing {
  cursor: auto;
}
.block:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
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
  inset-inline-end: 0;
  z-index: 1;
  padding: 1px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  background: var(--fm-surface);
  color: var(--fm-fg);
  font:
    11px/1.4 system-ui,
    sans-serif;
  white-space: nowrap;
  pointer-events: none;
  opacity: 0;
}
.host:hover .badge,
.block:focus-visible + .badge,
.block.live + .badge {
  opacity: 1;
}
.edit {
  display: block;
  column-span: all;
  box-sizing: border-box;
  width: 100%;
  margin: 0 0 8px;
  padding: 4px;
  border: 1px solid var(--fm-control-border);
  background: var(--fm-bg);
  color: var(--fm-fg);
  letter-spacing: inherit;
  text-align: inherit;
  resize: vertical;
  user-select: text;
}
</style>
