<script setup lang="ts">
// A quiet text button: no border at rest, an accent tint while it is pressed or expanded.
// The hover/press tint and the disabled fade come from the global `button` baseline in
// theme.css (do not add a second tint here). Attributes (aria-*, @click, title) fall
// through to the <button>.
</script>

<template>
  <button type="button" class="ui-button"><slot /></button>
</template>

<style scoped>
.ui-button {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  gap: var(--fm-space-1);
  height: 24px;
  padding: 0 var(--fm-space-2);
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-muted);
  font: inherit;
  font-size: var(--fm-text-sm);
  white-space: nowrap;
}
.ui-button[aria-disabled='true'] {
  opacity: 0.55;
}
.ui-button:not(:disabled):hover {
  color: var(--fm-fg);
}
.ui-button[aria-pressed='true'],
.ui-button[aria-expanded='true'] {
  background-color: color-mix(in srgb, var(--fm-accent) 16%, transparent);
  color: var(--fm-fg);
}
</style>
