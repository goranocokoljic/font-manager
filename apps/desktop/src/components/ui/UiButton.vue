<script setup lang="ts">
// A quiet text button: no border at rest, a hover tint, accent text while it is
// pressed or expanded. Attributes (aria-*, @click, title) fall through to the <button>.
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
.ui-button:not(:disabled):hover {
  background-color: var(--fm-hover);
  color: var(--fm-fg);
}
.ui-button[aria-pressed='true'],
.ui-button[aria-expanded='true'] {
  color: var(--fm-accent);
}
</style>
