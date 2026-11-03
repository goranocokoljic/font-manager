<script setup lang="ts">
// A section heading that folds its group: chevron + label, one button. The parent owns
// the body (`v-show` with `controls` as its id) and the folded state (`useFolded`).
import UiChevron from './UiChevron.vue';
import UiIcon from './UiIcon.vue';
import type { IconName } from './icons';

defineProps<{ folded: boolean; controls: string; icon?: IconName }>();
defineEmits<{ toggle: [] }>();
</script>

<template>
  <button
    type="button"
    class="ui-fold"
    :aria-expanded="!folded"
    :aria-controls="controls"
    @click="$emit('toggle')"
  >
    <span class="chevron fm-directional" aria-hidden="true"><UiChevron :open="!folded" /></span>
    <UiIcon v-if="icon" class="section-icon" :name="icon" />
    <slot />
  </button>
</template>

<style scoped>
.ui-fold {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0;
  border: 0;
  background-color: transparent;
  color: inherit;
  font: inherit;
  cursor: pointer;
}
.chevron {
  display: inline-flex;
}
.section-icon {
  margin-inline-start: 2px;
  opacity: 0.7;
}
.chevron :deep(svg) {
  width: 12px;
  height: 12px;
}
</style>
