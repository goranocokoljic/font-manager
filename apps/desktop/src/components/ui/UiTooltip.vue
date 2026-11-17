<script setup lang="ts">
// The app's one tooltip (E54.9): the name of an icon button and its shortcut, in a small
// layer over the page. UiIconButton owns it and decides when it is up (tooltip.ts); this
// draws it. It lives in <body>, so no scrolling pane or row clips it, takes no pointer, and is
// placed above its anchor — below when there is no room above — and never over it, so the
// button stays visible and clickable. It is always in the page, shown only while `open`, so
// the button's `aria-describedby` points at it: assistive technology reads its text (the name
// and the shortcut) as the button's description whether it is up or not.
// It comes up through `<FmFade speed="fast">` (MOTION.md: hover, 120 ms), the reduced fade
// under reduced motion; its colors are the popover's surface tokens, so it follows the theme.
import { nextTick, ref, watch } from 'vue';
import { FmFade } from '../../motion';

const props = defineProps<{
  id: string;
  open: boolean;
  anchor: HTMLElement | null;
  text: string;
  shortcut?: string;
}>();

const layer = ref<HTMLElement | null>(null);
const style = ref<Record<string, string>>({});

/** Space between the button and the tooltip, and between the tooltip and the window edge. */
const GAP = 6;
const MARGIN = 4;

// Centred over the anchor, below it when there is no room above, kept inside the window
// sideways. The gap is outside the anchor, so the tooltip never covers it.
function place(): void {
  const el = layer.value;
  const anchor = props.anchor;
  if (!el || !anchor) return;
  const r = anchor.getBoundingClientRect();
  const w = el.offsetWidth;
  const h = el.offsetHeight;
  const vw = window.innerWidth;
  const above = r.top - GAP - h;
  const top = above >= MARGIN ? above : r.bottom + GAP;
  const centred = r.left + r.width / 2 - w / 2;
  const left = Math.min(Math.max(MARGIN, centred), Math.max(MARGIN, vw - w - MARGIN));
  style.value = { left: `${left}px`, top: `${top}px` };
}

watch(
  () => [props.open, props.text, props.shortcut] as const,
  async ([open]) => {
    if (!open) return;
    // Measured hidden first, so it never shows at the wrong place.
    style.value = { left: '0px', top: '0px', visibility: 'hidden' };
    await nextTick();
    place();
  },
  { immediate: true },
);
</script>

<template>
  <Teleport to="body">
    <FmFade speed="fast" reduced="fade">
      <div
        v-show="open"
        :id="id"
        ref="layer"
        class="ui-tooltip"
        role="tooltip"
        data-ui-tooltip
        :style="style"
      >
        <span class="text">{{ text }}</span
        ><template v-if="shortcut"
          ><span class="sr-only">, </span><kbd class="shortcut">{{ shortcut }}</kbd></template
        >
      </div>
    </FmFade>
  </Teleport>
</template>

<style scoped>
.ui-tooltip {
  position: fixed;
  z-index: var(--fm-z-tooltip);
  box-sizing: border-box;
  display: flex;
  align-items: flex-start;
  gap: var(--fm-space-2);
  max-width: min(320px, calc(100vw - 8px));
  padding: 3px var(--fm-space-2);
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-surface-2);
  color: var(--fm-fg);
  box-shadow: var(--fm-shadow-2);
  font-family: var(--fm-font-ui);
  font-size: var(--fm-text-sm);
  line-height: var(--fm-leading);
  pointer-events: none;
}
/* The whole text, never cut (review E54.9): a long name or a reason wraps onto more lines
   inside the max width, as a native tooltip did. The key stays on the first line's end. */
.text {
  min-width: 0;
  overflow-wrap: anywhere;
}
.shortcut {
  flex: none;
  align-self: flex-start;
  white-space: nowrap;
  color: var(--fm-muted);
  font-family: inherit;
  font-variant-numeric: tabular-nums;
}
</style>
