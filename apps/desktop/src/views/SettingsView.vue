<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { FullSettings } from '../api/client';
import type { SettingsUpdate } from '../api/types';
import CopyValue from '../components/CopyValue.vue';
import { useCommandsStore } from '../stores/commands';
import { useSettingsStore } from '../stores/settings';

const store = useSettingsStore();
const commands = useCommandsStore();
const route = useRoute();
const router = useRouter();

const sections = [
  { id: 'sources', title: 'Sources' },
  { id: 'activation', title: 'Activation' },
  { id: 'appearance', title: 'Appearance' },
  { id: 'advanced', title: 'Advanced' },
] as const;
type SectionId = (typeof sections)[number]['id'];

// The open section is in the URL (`#/settings?section=advanced`), so a reload keeps it.
const current = computed<SectionId>(
  () => sections.find((s) => s.id === route.query.section)?.id ?? 'sources',
);

const focusTab = (id: SectionId) => document.getElementById(`tab-${id}`)?.focus();

async function select(id: SectionId, focus = false): Promise<void> {
  await router.replace({ query: { ...route.query, section: id } });
  if (focus) {
    await nextTick();
    focusTab(id);
  }
}

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
  void select(sections[to].id, true);
}

function close(): void {
  void router.push({ name: 'home' });
}

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' && !commands.open && !e.defaultPrevented) {
    e.preventDefault();
    close();
  }
}

onMounted(async () => {
  focusTab(current.value);
  await Promise.all([store.settings ? undefined : store.load(), store.loadWindow()]);
});

const s = computed(() => store.settings);

/** Radio choices; each change is saved at once. */
function choose(patch: SettingsUpdate): void {
  void store.update(patch);
}

type NumberKey = keyof Pick<FullSettings, 'cacheLimitMb' | 'rescanIntervalMinutes'>;

/** A number field: whole numbers are sent; anything else shows the saved value again. */
async function saveNumber(key: NumberKey, e: Event): Promise<void> {
  const input = e.target as HTMLInputElement;
  const text = input.value.trim();
  const ok = /^\d{1,10}$/.test(text) && (await store.update({ [key]: Number(text) }));
  if (!ok && store.settings) input.value = String(store.settings[key]);
}

const newFolder = ref('');
async function addFolder(): Promise<void> {
  if (await store.addSource(newFolder.value)) newFolder.value = '';
}

const kindLabel = { watched: 'Folder', system: 'System fonts', libraryFolder: 'Library folder' };
</script>

<template>
  <div class="settings" @keydown="onKey">
    <header class="bar">
      <h1 id="settings-title" class="title">Settings</h1>
      <button type="button" class="button" aria-keyshortcuts="Escape" @click="close">Close</button>
    </header>

    <div class="body">
      <div
        role="tablist"
        aria-orientation="vertical"
        aria-labelledby="settings-title"
        class="tabs"
        @keydown="onTabKey"
      >
        <button
          v-for="section in sections"
          :id="`tab-${section.id}`"
          :key="section.id"
          type="button"
          role="tab"
          class="tab"
          :aria-selected="current === section.id"
          :aria-controls="`panel-${section.id}`"
          :tabindex="current === section.id ? 0 : -1"
          @click="select(section.id)"
        >
          {{ section.title }}
        </button>
      </div>

      <div
        :id="`panel-${current}`"
        role="tabpanel"
        :aria-labelledby="`tab-${current}`"
        tabindex="0"
        class="panel"
      >
        <p v-if="store.error" role="alert" class="error">{{ store.error.message }}</p>

        <template v-if="current === 'sources'">
          <h2 class="heading">Sources</h2>
          <p class="note">
            Folders the library indexes in place. Removing a source forgets its fonts; the files
            stay where they are.
          </p>
          <ul class="sources" aria-label="Sources">
            <li v-for="source in store.sources" :key="source.id" class="source">
              <div class="source-text">
                <span class="path">{{ source.path }}</span>
                <span class="meta">
                  {{ kindLabel[source.kind] }} · {{ source.faceCount }}
                  {{ source.faceCount === 1 ? 'font' : 'fonts' }}
                </span>
              </div>
              <label class="check">
                <input
                  type="checkbox"
                  :checked="source.paused"
                  :aria-label="`Pause ${source.path}`"
                  @change="
                    store.setSourcePaused(source.id, ($event.target as HTMLInputElement).checked)
                  "
                />
                Paused
              </label>
              <button
                type="button"
                class="button"
                :aria-label="`Remove ${source.path}`"
                :disabled="source.kind === 'system'"
                :title="source.kind === 'system' ? 'System fonts cannot be removed.' : undefined"
                @click="store.removeSource(source.id)"
              >
                Remove
              </button>
            </li>
          </ul>
          <form class="add" @submit.prevent="addFolder">
            <label class="field">
              <span>Folder to add</span>
              <input v-model="newFolder" type="text" class="input" spellcheck="false" />
            </label>
            <button type="submit" class="button" :disabled="!newFolder.trim()">Add folder</button>
          </form>
        </template>

        <template v-else-if="current === 'activation' && s">
          <h2 class="heading">Activation</h2>
          <fieldset class="group">
            <legend>The Activate button activates fonts</legend>
            <label class="choice">
              <input
                type="radio"
                name="activation-mode"
                :checked="s.activationMode === 'session'"
                @change="choose({ activationMode: 'session' })"
              />
              For this session (until the app quits)
            </label>
            <label class="choice">
              <input
                type="radio"
                name="activation-mode"
                :checked="s.activationMode === 'permanent'"
                @change="choose({ activationMode: 'permanent' })"
              />
              Permanently (also after a restart)
            </label>
          </fieldset>
          <fieldset class="group">
            <legend>When a font has the same name as a system font or an active font</legend>
            <label class="choice">
              <input
                type="radio"
                name="on-conflict"
                :checked="s.onConflict === 'keepExisting'"
                @change="choose({ onConflict: 'keepExisting' })"
              />
              Keep the other font and tell me
            </label>
            <label class="choice">
              <input
                type="radio"
                name="on-conflict"
                :checked="s.onConflict === 'replace'"
                @change="choose({ onConflict: 'replace' })"
              />
              Activate this font anyway
            </label>
          </fieldset>
        </template>

        <template v-else-if="current === 'appearance' && s">
          <h2 class="heading">Appearance</h2>
          <fieldset class="group">
            <legend>Theme</legend>
            <label
              v-for="[value, text] in [
                ['system', 'Same as the system'],
                ['light', 'Light'],
                ['dark', 'Dark'],
              ] as const"
              :key="value"
              class="choice"
            >
              <input
                type="radio"
                name="theme"
                :checked="s.theme === value"
                @change="choose({ theme: value })"
              />
              {{ text }}
            </label>
          </fieldset>
          <fieldset class="group">
            <legend>Density</legend>
            <label
              v-for="[value, text] in [
                ['comfortable', 'Comfortable'],
                ['compact', 'Compact'],
              ] as const"
              :key="value"
              class="choice"
            >
              <input
                type="radio"
                name="density"
                :checked="s.density === value"
                @change="choose({ density: value })"
              />
              {{ text }}
            </label>
          </fieldset>
        </template>

        <template v-else-if="current === 'advanced' && s">
          <h2 class="heading">Advanced</h2>
          <label class="field">
            <span>Preview cache size (MB)</span>
            <input
              type="number"
              class="input number"
              inputmode="numeric"
              :value="s.cacheLimitMb"
              @change="saveNumber('cacheLimitMb', $event)"
            />
          </label>
          <label class="field">
            <span>Rescan sources every (minutes)</span>
            <input
              type="number"
              class="input number"
              inputmode="numeric"
              aria-describedby="rescan-hint"
              :value="s.rescanIntervalMinutes"
              @change="saveNumber('rescanIntervalMinutes', $event)"
            />
          </label>
          <p id="rescan-hint" class="note">0 turns automatic rescans off.</p>
          <div v-if="store.folders" class="field">
            <span>Logs folder</span>
            <span><CopyValue :value="store.folders.logs" label="logs folder" /></span>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex-direction: column;
  height: 100vh;
}
.bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 48px;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.body {
  display: flex;
  flex: 1;
  min-height: 0;
}
.tabs {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 180px;
  padding: 8px;
  border-right: 1px solid var(--fm-border);
  background: var(--fm-surface);
}
.tab {
  padding: var(--fm-gap) 8px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: start;
}
.tab[aria-selected='true'] {
  background: var(--fm-bg);
  color: var(--fm-accent);
  font-weight: 600;
}
.panel {
  flex: 1;
  max-width: 720px;
  padding: 16px 24px;
  overflow-y: auto;
}
.heading {
  margin: 0 0 12px;
  font-size: 15px;
}
.note {
  margin: 0 0 12px;
  color: var(--fm-muted);
}
.error {
  margin: 0 0 12px;
  color: var(--fm-danger);
}
.sources {
  margin: 0 0 16px;
  padding: 0;
  list-style: none;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
}
.source {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: var(--fm-row-height);
  padding: var(--fm-gap) 12px;
}
.source + .source {
  border-top: 1px solid var(--fm-border);
}
.source-text {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}
.path {
  overflow-wrap: anywhere;
}
.meta {
  color: var(--fm-muted);
  font-size: 11.5px;
}
.check,
.choice {
  display: flex;
  align-items: center;
  gap: 6px;
}
.add {
  display: flex;
  align-items: flex-end;
  gap: 8px;
}
.add .field {
  flex: 1;
  margin: 0;
}
.group {
  display: flex;
  flex-direction: column;
  gap: var(--fm-gap);
  margin: 0 0 16px;
  padding: 0;
  border: 0;
}
.group legend {
  margin-bottom: 6px;
  padding: 0;
  font-weight: 600;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0 0 12px;
}
.field > span:first-child {
  font-weight: 600;
}
.input {
  padding: 4px 8px;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: inherit;
  font: inherit;
}
.number {
  width: 120px;
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.button:disabled {
  opacity: 0.5;
}
</style>
