<script setup lang="ts">
import { computed } from 'vue';
import { ICONS, type IconName } from './icons';

// `size` 20 is the row actions' glyph (E54.9): the same 16 px drawing, scaled.
const props = withDefaults(defineProps<{ name: IconName; size?: 16 | 20 }>(), { size: 16 });
const markup = computed(() => ICONS[props.name]);
</script>

<template>
  <!-- Static, trusted markup from icons.ts, never user input. -->
  <!-- eslint-disable vue/no-v-html -->
  <svg
    class="ui-icon"
    :width="size"
    :height="size"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    v-html="markup"
  />
  <!-- eslint-enable vue/no-v-html -->
</template>

<style scoped>
.ui-icon {
  display: block;
  flex: none;
}
</style>
