<script setup lang="ts">
// A quiet 24 px icon button. The label is the accessible name and the tooltip.
// `aria-pressed` (a toggle) keeps a tint; `aria-expanded` turns it accent, like UiButton.
// The hover/press tint and the disabled fade come from the global `button` baseline in
// theme.css (do not add a second tint here). The default slot adds a small trailing count.
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
  padding: 0 var(--fm-space-1);
  gap: var(--fm-space-1);
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-muted);
}
.ui-icon-button[aria-disabled='true'] {
  opacity: 0.55;
}
.ui-icon-button:not(:disabled):hover,
.ui-icon-button[aria-pressed='true'] {
  color: var(--fm-fg);
}
.ui-icon-button[aria-pressed='true'] {
  background-color: var(--fm-hover);
}
.ui-icon-button[aria-expanded='true'] {
  background-color: color-mix(in srgb, var(--fm-accent) 16%, transparent);
  color: var(--fm-fg);
}
</style>
