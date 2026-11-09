<script setup lang="ts" generic="T extends string">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import UiIcon from './UiIcon.vue';
import type { IconName } from './icons';

export interface SegmentOption<V extends string> {
  value: V;
  /** Accessible name and tooltip. */
  label: string;
  /** Icon segment. Omit it for a text segment that shows `label`. */
  icon?: IconName;
}

const props = defineProps<{
  modelValue: T;
  options: readonly SegmentOption<T>[];
  label: string;
  /** Let text segments wrap onto more rows (many named choices). */
  wrap?: boolean;
  /**
   * A thumb under the chosen segment that slides to the next one (E46.2, the toolbar's view
   * switch): `transform` only, at `--fm-motion-fast`. Under reduced motion it jumps.
   */
  slide?: boolean;
}>();
const emit = defineEmits<{ 'update:modelValue': [value: T] }>();

const root = ref<HTMLElement | null>(null);
/** Where the chosen segment is; null before a layout (the segment then fills itself). */
const place = ref<{ x: number; y: number; w: number; h: number } | null>(null);

function measure(): void {
  const pressed = root.value?.querySelector<HTMLElement>('.seg[aria-pressed="true"]');
  if (!props.slide || !pressed || !pressed.offsetWidth) {
    place.value = null;
    return;
  }
  place.value = {
    x: pressed.offsetLeft,
    y: pressed.offsetTop,
    w: pressed.offsetWidth,
    h: pressed.offsetHeight,
  };
}

const thumbStyle = computed(() => {
  const p = place.value;
  if (!p) return undefined;
  return { width: `${p.w}px`, height: `${p.h}px`, transform: `translate(${p.x}px, ${p.y}px)` };
});

let observer: ResizeObserver | undefined;
onMounted(() => {
  measure();
  if (props.slide && typeof ResizeObserver !== 'undefined' && root.value) {
    observer = new ResizeObserver(measure);
    observer.observe(root.value);
  }
});
onBeforeUnmount(() => observer?.disconnect());
watch(
  () => [props.modelValue, props.options, props.slide],
  () => nextTick(measure),
  { flush: 'post' },
);
</script>

<template>
  <div
    ref="root"
    class="segmented"
    :class="{ wrap, sliding: !!place }"
    role="group"
    :aria-label="label"
  >
    <span v-if="place" class="thumb" aria-hidden="true" :style="thumbStyle" />
    <button
      v-for="o in options"
      :key="o.value"
      type="button"
      class="seg"
      :class="{ text: !o.icon }"
      :data-value="o.value"
      :aria-label="o.label"
      :title="o.label"
      :aria-pressed="modelValue === o.value"
      @click="emit('update:modelValue', o.value)"
    >
      <UiIcon v-if="o.icon" :name="o.icon" />
      <template v-else>{{ o.label }}</template>
    </button>
  </div>
</template>

<style scoped>
.segmented {
  position: relative;
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
}
.seg {
  position: relative;
  display: grid;
  place-items: center;
  width: 28px;
  height: 24px;
  padding: 0;
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-muted);
  transition:
    background-color var(--fm-motion-fast) var(--fm-ease),
    color var(--fm-motion-fast) var(--fm-ease);
}
/* Still inline: the track hugs its segments and only wraps when the row runs out. */
.wrap {
  flex-wrap: wrap;
  max-width: 100%;
}
.seg.text {
  width: auto;
  min-width: 28px;
  padding: 0 var(--fm-space-2);
  font-size: var(--fm-text-sm);
  white-space: nowrap;
}
.seg:hover {
  color: var(--fm-fg);
}
.seg[aria-pressed='true'] {
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  box-shadow: 0 0 0 1px var(--fm-accent);
}
/* The sliding thumb (`slide`): it paints the chosen segment's fill and moves under the
   segments; the segment itself then stays clear. Its size changes at once, its place glides. */
.thumb {
  position: absolute;
  top: 0;
  left: 0;
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-accent);
  box-shadow: 0 0 0 1px var(--fm-accent);
  pointer-events: none;
  transition: transform var(--fm-motion-fast) var(--fm-ease);
}
.sliding .seg[aria-pressed='true'] {
  background-color: transparent;
  box-shadow: none;
}
</style>
