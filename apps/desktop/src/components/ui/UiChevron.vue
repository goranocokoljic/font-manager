<script setup lang="ts">
// The expand / fold chevron: points to the inline end when closed, down when open.
// The parent keeps `fm-directional` on its wrapper so the closed chevron flips in RTL.
import UiIcon from './UiIcon.vue';

defineProps<{ open: boolean }>();
</script>

<template>
  <UiIcon class="ui-chevron" :class="{ open }" name="chevron" />
</template>

<style scoped>
.ui-chevron {
  transition: transform var(--fm-motion-fast) var(--fm-ease);
}
.ui-chevron.open {
  transform: rotate(90deg);
}
</style>
