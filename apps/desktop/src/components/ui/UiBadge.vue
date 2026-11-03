<script setup lang="ts">
// A small monochrome label (licence, variable, fallback). `accent` is for the one
// state that asks for attention (an update); nothing else is coloured.
withDefaults(defineProps<{ tone?: 'quiet' | 'accent' }>(), { tone: 'quiet' });
</script>

<template>
  <span class="ui-badge" :class="`tone-${tone}`"><slot /></span>
</template>

<style scoped>
.ui-badge {
  display: inline-flex;
  align-items: center;
  flex: none;
  padding: 0 5px;
  border-radius: var(--fm-radius-sm);
  border: 1px solid var(--fm-border);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 500;
  line-height: 14px;
  white-space: nowrap;
}
.tone-accent {
  border-color: color-mix(in srgb, var(--fm-accent) 50%, transparent);
  color: var(--fm-accent);
}
</style>
