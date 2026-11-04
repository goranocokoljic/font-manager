<script setup lang="ts">
/**
 * Children that change place glide to the new place; children that come or go do not
 * animate. Keyed children, as for any `<TransitionGroup>`; other props (`tag`) pass through.
 * Vue measures the move and animates `transform` only, so nothing lays out again per frame.
 */
</script>

<template>
  <TransitionGroup name="fm-flip">
    <slot />
  </TransitionGroup>
</template>

<!-- Not scoped: the class lands on the slotted children, which belong to the parent. -->
<style>
.fm-flip-move {
  transition: transform var(--fm-motion-move) var(--fm-ease);
}
</style>
