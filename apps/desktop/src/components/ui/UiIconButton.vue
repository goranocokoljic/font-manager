<script setup lang="ts">
// A quiet 24 px icon button. The label is the accessible name and the tooltip.
// `aria-pressed` (a toggle) tints it; the default slot adds a small trailing count.
import UiIcon from './UiIcon.vue';
import type { IconName } from './icons';

defineProps<{ icon: IconName; label: string; title?: string }>();
</script>

<template>
  <button type="button" class="ui-icon-button" :aria-label="label" :title="title ?? label">
    <UiIcon :name="icon" />
    <slot />
  </button>
</template>

<style scoped>
.ui-icon-button {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  min-width: 24px;
  height: 24px;
  padding: 0 4px;
  gap: 4px;
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-muted);
  transition: background-color var(--fm-motion-fast) var(--fm-ease);
}
.ui-icon-button[aria-disabled='true'],
.ui-icon-button:disabled {
  opacity: 0.55;
}
.ui-icon-button:hover {
  background-color: var(--fm-hover);
  color: var(--fm-fg);
}
.ui-icon-button[aria-pressed='true'] {
  background-color: var(--fm-hover);
  color: var(--fm-fg);
}
</style>
