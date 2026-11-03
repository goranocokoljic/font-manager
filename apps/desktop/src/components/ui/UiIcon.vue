<script setup lang="ts">
import { computed } from 'vue';
import { ICONS, type IconName } from './icons';

const props = defineProps<{ name: IconName }>();
const markup = computed(() => ICONS[props.name]);
</script>

<template>
  <!-- Static, trusted markup from icons.ts, never user input. -->
  <!-- eslint-disable vue/no-v-html -->
  <svg
    class="ui-icon"
    width="16"
    height="16"
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
