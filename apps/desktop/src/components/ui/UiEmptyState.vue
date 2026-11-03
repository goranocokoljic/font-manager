<script setup lang="ts">
// A typographic empty state: a headline, one quiet line, and exactly one action that fills
// the state. No illustration. The action goes in the default slot.
defineProps<{ title: string; body?: string }>();
</script>

<template>
  <div class="ui-empty">
    <h2 class="title">{{ title }}</h2>
    <p v-if="body" class="body">{{ body }}</p>
    <div v-if="$slots.default" class="action"><slot /></div>
  </div>
</template>

<style scoped>
.ui-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  max-inline-size: 360px;
  gap: var(--fm-space-2);
  text-align: center;
}
.title {
  margin: 0;
  color: var(--fm-fg);
  font-size: var(--fm-text-xl);
  font-weight: 500;
  letter-spacing: -0.01em;
  line-height: 1.25;
}
.body {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-md);
  line-height: 1.5;
}
.action {
  margin-block-start: var(--fm-space-3);
}
</style>
