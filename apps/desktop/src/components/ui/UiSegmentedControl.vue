<script setup lang="ts" generic="T extends string">
import UiIcon from './UiIcon.vue';
import type { IconName } from './icons';

export interface SegmentOption<V extends string> {
  value: V;
  /** Accessible name and tooltip. */
  label: string;
  icon: IconName;
}

defineProps<{ modelValue: T; options: readonly SegmentOption<T>[]; label: string }>();
const emit = defineEmits<{ 'update:modelValue': [value: T] }>();
</script>

<template>
  <div class="segmented" role="group" :aria-label="label">
    <button
      v-for="o in options"
      :key="o.value"
      type="button"
      class="seg"
      :aria-label="o.label"
      :title="o.label"
      :aria-pressed="modelValue === o.value"
      @click="emit('update:modelValue', o.value)"
    >
      <UiIcon :name="o.icon" />
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
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
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
