<script setup lang="ts">
// A floating layer under its anchor (over it when there is no room below), or at a point
// (`at`: a context menu opens at the pointer, E54.2). It lives in
// <body>, so no scrolling pane clips it; a pointer press outside it or a scroll elsewhere
// asks the owner to close it; a resize moves it with its anchor. The owner keeps the focus and handles the keys —
// the layer never takes focus, so the dialogs' focus guards never see it leave.
// It comes up through `<FmSlide>` (E46.3): a 120 ms fade and a 4 px rise, a fade only under
// reduced motion. Every `UiSelect` list is one of these.
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { FmSlide } from '../../motion';

const props = withDefaults(
  defineProps<{
    open: boolean;
    anchor: HTMLElement | null;
    /** The layer is at least as wide as its anchor. */
    matchWidth?: boolean;
    /**
     * A point in the window (client pixels) to open at instead of under the anchor: the
     * layer's corner is at the point, below it or above it as there is room. The anchor
     * still gives the reading direction, and a press on it still does not close the layer.
     */
    at?: { x: number; y: number } | null;
    /**
     * A layer over the others (the text fields' menu, E54.2): a press inside it does not close
     * the other open layers, so a text field inside a popover keeps its popover while its menu
     * runs.
     */
    over?: boolean;
  }>(),
  { matchWidth: false, at: null, over: false },
);
// `placed`: the layer is visible, so an owner that moves the focus into it can do so now.
const emit = defineEmits<{ close: []; placed: [] }>();

const layer = ref<HTMLElement | null>(null);
const style = ref<Record<string, string>>({});

/** Space between the anchor and the layer, and between the layer and the window edge. */
const GAP = 4;
const MARGIN = 8;
const MIN_HEIGHT = 96;
const MAX_HEIGHT = 320;

function place(): void {
  const anchor = props.anchor;
  const el = layer.value;
  const r = props.at ? pointRect(props.at) : anchor?.getBoundingClientRect();
  if (!el || !r) return;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const below = vh - r.bottom - GAP - MARGIN;
  const above = r.top - GAP - MARGIN;
  const wanted = el.scrollHeight;
  // Open upwards only when the layer does not fit below and there is more room above.
  const flip = wanted > below && above > below;
  const maxHeight = Math.max(MIN_HEIGHT, Math.min(wanted, MAX_HEIGHT, flip ? above : below));
  const width = Math.max(el.offsetWidth, props.matchWidth ? r.width : 0);
  const rtl = getComputedStyle(anchor ?? document.documentElement).direction === 'rtl';
  const wantedLeft = rtl ? r.right - width : r.left;
  const left = Math.min(Math.max(MARGIN, wantedLeft), Math.max(MARGIN, vw - width - MARGIN));
  const s: Record<string, string> = { left: `${left}px`, maxHeight: `${maxHeight}px` };
  if (props.matchWidth) s.minWidth = `${r.width}px`;
  if (flip) s.bottom = `${vh - r.top + GAP}px`;
  else s.top = `${r.bottom + GAP}px`;
  style.value = s;
}

/** A point as an empty rectangle, so the layer opens at it as it opens at an anchor. */
function pointRect(p: { x: number; y: number }) {
  return { left: p.x, right: p.x, top: p.y, bottom: p.y, width: 0 };
}

function onPointerDown(e: PointerEvent): void {
  const target = e.target;
  if (!(target instanceof Node)) return;
  // The anchor's own label is part of it: a click there toggles, it does not close first.
  const own = props.anchor?.closest('label') ?? props.anchor;
  if (layer.value?.contains(target) || own?.contains(target)) return;
  const el = target instanceof Element ? target : target.parentElement;
  if (el?.closest('[data-ui-popover-over]')) return;
  emit('close');
}
function onScroll(e: Event): void {
  if (e.target instanceof Node && layer.value?.contains(e.target)) return;
  emit('close');
}
function onResize(): void {
  place();
}

function listen(on: boolean): void {
  if (on) {
    document.addEventListener('pointerdown', onPointerDown, true);
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onResize);
  } else {
    document.removeEventListener('pointerdown', onPointerDown, true);
    window.removeEventListener('scroll', onScroll, true);
    window.removeEventListener('resize', onResize);
  }
}

watch(
  () => props.open,
  async (open) => {
    listen(open);
    if (!open) return;
    // Measured at the origin first, so a layer near the edge keeps its natural width.
    style.value = { left: '0px', top: '0px', visibility: 'hidden' };
    await nextTick();
    place();
    await nextTick();
    if (props.open) emit('placed');
  },
  { immediate: true },
);
// A new point while open (a second right-click): the layer moves there.
watch(
  () => props.at,
  () => {
    if (props.open) place();
  },
);
onBeforeUnmount(() => listen(false));
</script>

<template>
  <Teleport to="body">
    <FmSlide from="bottom" distance="4px" speed="fast" reduced="fade">
      <div
        v-if="open"
        ref="layer"
        class="ui-popover"
        data-ui-popover
        :data-ui-popover-over="over ? '' : undefined"
        :style="style"
      >
        <slot />
      </div>
    </FmSlide>
  </Teleport>
</template>

<style scoped>
.ui-popover {
  position: fixed;
  z-index: var(--fm-z-popover);
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  min-width: 120px;
  max-width: min(360px, calc(100vw - 16px));
  padding: var(--fm-space-1);
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface-2);
  color: var(--fm-fg);
  box-shadow: var(--fm-shadow-2);
  font-family: var(--fm-font-ui);
  font-size: var(--fm-text-md);
  line-height: var(--fm-leading);
}
/* A layer over the others (the text fields' menu) is drawn above them: the popover its field
   is in was opened earlier and sits at the same level. */
.ui-popover[data-ui-popover-over] {
  z-index: calc(var(--fm-z-popover) + 1);
}
</style>
