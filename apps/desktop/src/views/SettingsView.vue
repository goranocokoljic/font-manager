<script setup lang="ts">
// Settings (boards 11 and 15, E57.3): the shell alone. A 232 px navigation on the surface
// colour with the title and the nine sections as a vertical tab list, and beside it the
// panel: the open section (a component of its own under components/settings/), at most
// 760 px wide (Sources keeps its 920). No top bar: the x icon button in the panel's top
// corner and Esc close the view. The open section is in the URL
// (`#/settings?section=advanced`), so a reload keeps it.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import AboutSettings from '../components/settings/AboutSettings.vue';
import ActivationSettings from '../components/settings/ActivationSettings.vue';
import AdvancedSettings from '../components/settings/AdvancedSettings.vue';
import AppearanceSettings from '../components/settings/AppearanceSettings.vue';
import CatalogsSettings from '../components/settings/CatalogsSettings.vue';
import LanguagesSettings from '../components/settings/LanguagesSettings.vue';
import LocalApiSettings from '../components/settings/LocalApiSettings.vue';
import McpSettings from '../components/settings/McpSettings.vue';
import SourcesSettings from '../components/settings/SourcesSettings.vue';
import UiIcon from '../components/ui/UiIcon.vue';
import UiIconButton from '../components/ui/UiIconButton.vue';
import { errorMessage, t, type MessageKey } from '../i18n';
import { useAppStore } from '../stores/app';
import { useCatalogsStore } from '../stores/catalogs';
import { useCommandsStore } from '../stores/commands';
import { useSettingsStore } from '../stores/settings';
import { useSourcesStore } from '../stores/sources';
import { useThemesStore } from '../stores/themes';
import { useUpdatesStore } from '../stores/updates';

const store = useSettingsStore();
const catalogs = useCatalogsStore();
const sources = useSourcesStore();
const commands = useCommandsStore();
const app = useAppStore();
const updates = useUpdatesStore();
const themes = useThemesStore();
const route = useRoute();
const router = useRouter();

const sections = [
  { id: 'sources', icon: 'folder', component: SourcesSettings },
  { id: 'activation', icon: 'power', component: ActivationSettings },
  { id: 'appearance', icon: 'sun', component: AppearanceSettings },
  { id: 'languages', icon: 'languages', component: LanguagesSettings },
  { id: 'catalogs', icon: 'globe', component: CatalogsSettings },
  { id: 'advanced', icon: 'sliders', component: AdvancedSettings },
  { id: 'localApi', icon: 'code', component: LocalApiSettings },
  { id: 'mcp', icon: 'sparkles', component: McpSettings },
  { id: 'about', icon: 'info', component: AboutSettings },
] as const;
type SectionId = (typeof sections)[number]['id'];

const current = computed<SectionId>(
  () => sections.find((s) => s.id === route.query.section)?.id ?? 'sources',
);
const section = computed(() => sections.find((s) => s.id === current.value)!);

const focusTab = (id: SectionId) => document.getElementById(`tab-${id}`)?.focus();

/**
 * Where focus goes once the section has changed: the tab when an arrow key moved it (the tab
 * list keeps working), the section's heading otherwise (a click, Enter or Space on a tab).
 */
let focusAfter: 'tab' | 'heading' | null = null;

async function select(id: SectionId, focus: 'tab' | 'heading'): Promise<void> {
  if (id === current.value) {
    if (focus === 'heading') focusHeading();
    return;
  }
  focusAfter = focus;
  await router.replace({ query: { ...route.query, section: id } });
}

function focusHeading(): void {
  document.querySelector<HTMLElement>('.panel [data-section-heading]')?.focus();
}

watch(current, async (id) => {
  const how = focusAfter;
  focusAfter = null;
  await nextTick();
  if (how === 'tab') focusTab(id);
  else if (how === 'heading') focusHeading();
});

// A vertical tab list: arrows move between sections, Home and End jump to the ends.
function onTabKey(e: KeyboardEvent): void {
  const i = sections.findIndex((s) => s.id === current.value);
  const last = sections.length - 1;
  const to =
    e.key === 'ArrowDown'
      ? (i + 1) % sections.length
      : e.key === 'ArrowUp'
        ? (i + last) % sections.length
        : e.key === 'Home'
          ? 0
          : e.key === 'End'
            ? last
            : -1;
  if (to < 0) return;
  e.preventDefault();
  void select(sections[to].id, 'tab');
}

function close(): void {
  void router.push({ name: 'home' });
}

const root = ref<HTMLElement | null>(null);

/**
 * Esc closes the view wherever focus is in it, and also when a click on plain text has left
 * focus on the page itself (no part of the panel takes focus on a click). A key inside a
 * dialog or popover outside the view (the update dialog, setup) is theirs, not ours.
 */
function onKey(e: KeyboardEvent): void {
  if (e.key !== 'Escape' || commands.open || e.defaultPrevented) return;
  const target = e.target as Node | null;
  const ours = target === document.body || target === document.documentElement;
  if (!ours && !(target && root.value?.contains(target))) return;
  e.preventDefault();
  close();
}

onMounted(async () => {
  window.addEventListener('keydown', onKey);
  focusTab(current.value);
  await Promise.all([
    store.settings ? undefined : store.load(),
    store.loadWindow(),
    app.info ? undefined : app.load(),
    updates.support ? undefined : updates.loadSupport(),
    updates.loadOverview(),
    sources.load(),
    catalogs.load(),
    themes.themes.length ? undefined : themes.load(),
  ]);
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  sources.stop();
});

/** The last failure of any store this window reads. */
const error = computed(() => store.error ?? sources.error ?? app.error);

/** A section's title, which is also its heading. */
const sectionTitle = (id: SectionId) => t(`settings.sections.${id}` as MessageKey);
</script>

<template>
  <div ref="root" class="settings">
    <nav class="nav" aria-labelledby="settings-title">
      <h1 id="settings-title" class="title">{{ t('settings.title') }}</h1>
      <div
        role="tablist"
        aria-orientation="vertical"
        aria-labelledby="settings-title"
        class="tabs"
        @keydown="onTabKey"
      >
        <button
          v-for="s in sections"
          :id="`tab-${s.id}`"
          :key="s.id"
          type="button"
          role="tab"
          class="tab"
          :aria-selected="current === s.id"
          :aria-controls="`panel-${s.id}`"
          :tabindex="current === s.id ? 0 : -1"
          @click="select(s.id, 'heading')"
        >
          <UiIcon :name="s.icon" />
          {{ sectionTitle(s.id) }}
        </button>
      </div>
    </nav>

    <div class="main">
      <UiIconButton
        class="close"
        icon="close"
        :label="t('settings.close')"
        shortcut="Esc"
        aria-keyshortcuts="Escape"
        data-testid="settings-close"
        @click="close"
      />
      <div
        :id="`panel-${current}`"
        role="tabpanel"
        :aria-labelledby="`tab-${current}`"
        class="panel"
        :class="{ wide: current === 'sources' }"
      >
        <p v-if="error" role="alert" class="error">{{ errorMessage(error) }}</p>
        <component :is="section.component" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  height: 100vh;
  min-height: 0;
}
.nav {
  box-sizing: border-box;
  display: flex;
  flex: none;
  flex-direction: column;
  width: 232px;
  padding: var(--fm-space-4) var(--fm-space-3);
  border-inline-end: 1px solid var(--fm-border);
  background-color: var(--fm-surface);
}
.title {
  margin: var(--fm-space-2) var(--fm-space-2) var(--fm-space-4);
  font-size: var(--fm-text-lg);
  font-weight: 600;
}
.tabs {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.tab {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  height: 32px;
  padding: 0 var(--fm-space-2);
  border: 0;
  border-radius: var(--fm-radius-md);
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  text-align: start;
}
.tab:hover {
  background-color: var(--fm-hover);
}
.tab:focus-visible {
  outline-offset: -2px;
}
.tab[aria-selected='true'] {
  background-color: var(--fm-side-active, var(--fm-hover));
  font-weight: 600;
}
.tab :deep(.ui-icon) {
  color: var(--fm-muted);
}
.tab[aria-selected='true'] :deep(.ui-icon) {
  color: var(--fm-fg);
}
.main {
  position: relative;
  flex: 1;
  min-width: 0;
  overflow-y: auto;
}
/* In the panel's top corner, at the end of the line. */
.close {
  position: absolute;
  inset-block-start: var(--fm-space-4);
  inset-inline-end: var(--fm-space-4);
}
.panel {
  box-sizing: border-box;
  max-width: calc(760px + 2 * var(--fm-space-8));
  padding: var(--fm-space-8) var(--fm-space-8) var(--fm-space-6);
}
.panel.wide {
  max-width: calc(920px + 2 * var(--fm-space-8));
}
/* Every section's heading, Sources' too, is drawn by the shell (boards 11 and 15). It takes
   focus when the section changes; a ring only for the keyboard. */
.panel :deep([data-section-heading]) {
  font-size: var(--fm-text-xl);
}
.panel :deep([data-section-heading]:focus:not(:focus-visible)) {
  outline: none;
}
.error {
  margin: 0 0 12px;
  color: var(--fm-danger);
}
</style>
