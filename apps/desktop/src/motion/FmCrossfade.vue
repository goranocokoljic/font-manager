<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { crossfade } from './crossfade';
import type { MotionSpeed } from './speed';

/**
 * One element whose content crossfades when `value` changes (E46.4): the old content fades
 * out over the new one, both at once, at the `speed` token (`crossfade()`). A count that ticks
 * up after a drop is the case. The element is `tag` (a `span` by default); attributes fall
 * through to it. Its parent must be its containing block (`position: relative`). Under reduced
 * motion the change is a cut.
 */
const props = withDefaults(
  defineProps<{ value: string | number; tag?: string; speed?: MotionSpeed }>(),
  { tag: 'span', speed: 'move' },
);

const el = ref<HTMLElement | null>(null);

watch(
  () => props.value,
  () => {
    if (!el.value) return;
    const run = crossfade(el.value, props.speed);
    void nextTick(run);
  },
  { flush: 'pre' },
);
</script>

<template>
  <component :is="tag" ref="el"><slot /></component>
</template>
