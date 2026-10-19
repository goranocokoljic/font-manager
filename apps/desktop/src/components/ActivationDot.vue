<script setup lang="ts">
// The activation dot of a row or card: hollow for inactive, half filled for a
// partly active family, filled for active. Decorative: the row's accessible
// name already says the state (`rowLabel`).
import type { RowActivation } from '../list/activation';

defineProps<{ state: RowActivation }>();
</script>

<template>
  <span class="dot" :class="state" :data-activation="state" aria-hidden="true" />
</template>

<style scoped>
.dot {
  display: inline-block;
  flex: none;
  width: 8px;
  height: 8px;
  box-sizing: border-box;
  border: 1.5px solid var(--fm-muted);
  border-radius: 50%;
}
.dot.active {
  border-color: var(--fm-accent);
  background: var(--fm-accent);
}
.dot.partial {
  border-color: var(--fm-accent);
  background: linear-gradient(90deg, var(--fm-accent) 50%, transparent 50%);
}
</style>
