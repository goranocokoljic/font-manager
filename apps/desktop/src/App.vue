<script setup lang="ts">
import { RouterView } from 'vue-router';
import CommandPalette from './components/CommandPalette.vue';
import { useShortcuts } from './stores/commands';

useShortcuts();
</script>

<template>
  <RouterView />
  <CommandPalette />
</template>
