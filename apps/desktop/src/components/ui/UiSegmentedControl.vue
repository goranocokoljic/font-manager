<script setup lang="ts" generic="T extends string">
import UiIcon from './UiIcon.vue';
import type { IconName } from './icons';

export interface SegmentOption<V extends string> {
  value: V;
  /** Accessible name and tooltip. */
  label: string;
  /** Icon segment. Omit it for a text segment that shows `label`. */
  icon?: IconName;
}

defineProps<{
  modelValue: T;
  options: readonly SegmentOption<T>[];
  label: string;
  /** Let text segments wrap onto more rows (many named choices). */
  wrap?: boolean;
}>();
const emit = defineEmits<{ 'update:modelValue': [value: T] }>();
</script>

<template>
  <div class="segmented" :class="{ wrap }" role="group" :aria-label="label">
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
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
}
.seg {
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
  background-color: var(--fm-bg);
  color: var(--fm-accent);
  box-shadow: 0 0 0 1px var(--fm-border);
}
</style>
