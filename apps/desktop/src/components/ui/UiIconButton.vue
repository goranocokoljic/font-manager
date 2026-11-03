<script setup lang="ts">
// A quiet 24 px icon button. The label is the accessible name and the tooltip.
import UiIcon from './UiIcon.vue';
import type { IconName } from './icons';

defineProps<{ icon: IconName; label: string; title?: string }>();
</script>

<template>
  <button type="button" class="ui-icon-button" :aria-label="label" :title="title ?? label">
    <UiIcon :name="icon" />
  </button>
</template>

<style scoped>
.ui-icon-button {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-muted);
  transition: background-color var(--fm-motion-fast) var(--fm-ease);
}
.ui-icon-button:hover {
  background-color: var(--fm-hover);
  color: var(--fm-fg);
}
</style>
