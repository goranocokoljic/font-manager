<script setup lang="ts">
// The activation dot of a row or card: hollow for inactive, half filled for a
// partly active family, filled for active. Decorative: the row's accessible
// name already says the state (`rowLabel`). Activation fills it: the fill scales 0 → 1 over
// `--fm-motion-fast` (E46.4, motion table "Activate"); under reduced motion it changes at once.
// `still` (the Table view) changes at once always: nothing moves on the Table.
import type { RowActivation } from '../list/activation';

defineProps<{ state: RowActivation; still?: boolean }>();
</script>

<template>
  <span class="dot" :class="[state, { still }]" :data-activation="state" aria-hidden="true" />
</template>

<style scoped>
.dot {
  position: relative;
  display: inline-block;
  flex: none;
  width: 8px;
  height: 8px;
  box-sizing: border-box;
  border: 1.5px solid var(--fm-muted);
  border-radius: 50%;
  transition: border-color var(--fm-motion-fast) var(--fm-ease);
}
/* The fill, over the ring: hidden at scale 0, grown to 1 when the face is active. */
.dot::before {
  position: absolute;
  inset: -1.5px;
  border-radius: 50%;
  background: var(--fm-accent);
  transform: scale(0);
  transition: transform var(--fm-motion-fast) var(--fm-ease);
  content: '';
}
.dot.active,
.dot.partial {
  border-color: var(--fm-accent);
}
.dot.active::before {
  transform: scale(1);
}
.dot.partial::before {
  background: linear-gradient(90deg, var(--fm-accent) 50%, transparent 50%);
  transform: scale(1);
}
.dot.still,
.dot.still::before {
  transition: none;
}
</style>
