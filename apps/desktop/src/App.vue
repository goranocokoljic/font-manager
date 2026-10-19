<script setup lang="ts">
import { onMounted, watchEffect } from 'vue';
import { RouterView, useRouter } from 'vue-router';
import { useClient } from './api/client';
import CommandPalette from './components/CommandPalette.vue';
import ToastMessage from './components/ToastMessage.vue';
import { HELP_PAGES } from './help';
import { ACTIONS } from './keyboard/keymap';
import { useActions, useShortcuts } from './stores/commands';
import { useSettingsStore } from './stores/settings';

const settings = useSettingsStore();
const router = useRouter();

useShortcuts();
useActions([
  {
    id: ACTIONS.settings,
    title: 'Open settings',
    group: 'App',
    run: () => router.push({ name: 'settings' }),
  },
  {
    id: 'app.conflicts',
    title: 'Show font conflicts',
    group: 'App',
    run: () => router.push({ name: 'conflicts' }),
  },
  // The same pages as the native Help menu.
  ...HELP_PAGES.map((page) => ({
    id: `help.${page.slug}`,
    title: `Help: ${page.title}`,
    group: 'Help',
    run: () => useClient().openHelp(page.slug),
  })),
]);

onMounted(() => settings.load());

// Theme and density apply the moment a setting changes: theme.css reads them
// from the root element.
watchEffect(() => {
  const s = settings.settings;
  if (!s) return;
  document.documentElement.dataset.theme = s.theme;
  document.documentElement.dataset.density = s.density;
});
</script>

<template>
  <RouterView />
  <CommandPalette />
  <ToastMessage />
</template>
