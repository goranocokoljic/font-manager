<script setup lang="ts">
import { onMounted, watch, watchEffect } from 'vue';
import { RouterView, useRouter } from 'vue-router';
import { useClient } from './api/client';
import CommandPalette from './components/CommandPalette.vue';
import ToastMessage from './components/ToastMessage.vue';
import UpdateDialog from './components/UpdateDialog.vue';
import { HELP_PAGES } from './help';
import { direction, locale, resolveLocale, setLocale, t, type MessageKey } from './i18n';
import { ACTIONS } from './keyboard/keymap';
import { useActions, useShortcuts } from './stores/commands';
import { useSettingsStore } from './stores/settings';
import { useUpdatesStore } from './stores/updates';

const settings = useSettingsStore();
const updates = useUpdatesStore();
const router = useRouter();

useShortcuts();
// Getters: the palette shows the titles in the language chosen now.
useActions([
  {
    id: ACTIONS.settings,
    get title() {
      return t('app.openSettings');
    },
    get group() {
      return t('app.groups.app');
    },
    run: () => router.push({ name: 'settings' }),
  },
  {
    id: 'app.conflicts',
    get title() {
      return t('app.showConflicts');
    },
    get group() {
      return t('app.groups.app');
    },
    run: () => router.push({ name: 'conflicts' }),
  },
  // The same pages as the native Help menu.
  ...HELP_PAGES.map((page) => ({
    id: `help.${page.slug}`,
    get title() {
      return t('app.helpAction', { page: t(`shell.helpPages.${page.slug}` as MessageKey) });
    },
    get group() {
      return t('app.groups.help');
    },
    run: () => useClient().openHelp(page.slug),
  })),
]);

// The one check without a click: only when the user turned "Check automatically" on.
onMounted(async () => {
  await settings.load();
  await updates.checkAtStart(settings.settings);
});

// Theme and density apply the moment a setting changes: theme.css reads them
// from the root element.
watchEffect(() => {
  const s = settings.settings;
  if (!s) return;
  document.documentElement.dataset.theme = s.theme;
  document.documentElement.dataset.density = s.density;
});

// The Language setting relabels the UI at once: "System" (and the time before the
// settings are read) follows the OS's preferred languages. A right-to-left
// language mirrors the chrome: theme.css and the components use logical
// properties, so `dir` on the root is all it takes.
watchEffect(() => {
  const preferred = navigator.languages?.length ? navigator.languages : [navigator.language];
  setLocale(resolveLocale(settings.settings?.language, preferred));
  document.documentElement.lang = locale();
  document.documentElement.dir = direction();
});

// The native Help menu and the tray follow the same language.
watch(
  locale,
  (tag) => {
    useClient()
      .setShellLocale(tag)
      .catch((e: unknown) => console.warn('the shell language was not changed', e));
  },
  { immediate: true },
);
</script>

<template>
  <RouterView />
  <CommandPalette />
  <ToastMessage />
  <UpdateDialog />
</template>
