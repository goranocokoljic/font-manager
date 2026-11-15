<script setup lang="ts">
// The one icon button of the app (E50.6): a 28 px target around a 16 px glyph. The label is
// the accessible name and the tooltip. `size="sm"` (24 px, same glyph) is only for a button
// inside a row that is 24 px tall: a sidebar row in the compact density (styles/sidebar.css)
// and the remove button of a tag chip. Everything else keeps the default. `aria-pressed`
// (a toggle) keeps a tint; `aria-expanded` turns it accent, like UiButton. The hover/press
// tint and the disabled fade come from the global `button` baseline in theme.css (do not add
// a second tint here). The default slot adds a small trailing count. No component draws an
// icon-only button of its own (iconButtons.test.ts).
import UiIcon from './UiIcon.vue';
import type { IconName } from './icons';

withDefaults(defineProps<{ icon: IconName; label: string; title?: string; size?: 'md' | 'sm' }>(), {
  title: undefined,
  size: 'md',
});
</script>

<template>
  <button
    type="button"
    class="ui-icon-button"
    :class="size"
    :data-size="size"
    :aria-label="label"
    :title="title ?? label"
  >
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
  box-sizing: border-box;
  min-width: 28px;
  height: 28px;
  padding: 0 var(--fm-space-1);
  gap: var(--fm-space-1);
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-muted);
}
.ui-icon-button.sm {
  min-width: 24px;
  height: 24px;
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
