<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { FullSettings } from '../api/client';
import type { Language, SettingsUpdate } from '../api/types';
import AdobeFontsNote from '../components/AdobeFontsNote.vue';
import CopyValue from '../components/CopyValue.vue';
import LocalApiSettings from '../components/LocalApiSettings.vue';
import McpSettings from '../components/McpSettings.vue';
import {
  CATALOGS,
  LOCALES,
  errorMessage,
  resolveLocale,
  t,
  type Catalog,
  type MessageKey,
} from '../i18n';
import { useAppStore } from '../stores/app';
import { useCatalogsStore } from '../stores/catalogs';
import { useCommandsStore } from '../stores/commands';
import { useSettingsStore } from '../stores/settings';
import { useSourcesStore } from '../stores/sources';
import { useToastStore } from '../stores/toast';
import { useUpdatesStore } from '../stores/updates';

const store = useSettingsStore();
const catalogs = useCatalogsStore();
const sources = useSourcesStore();
const commands = useCommandsStore();
const app = useAppStore();
const toast = useToastStore();
const updates = useUpdatesStore();
const route = useRoute();
const router = useRouter();

const sections = [
  { id: 'sources' },
  { id: 'activation' },
  { id: 'appearance' },
  { id: 'catalogs' },
  { id: 'advanced' },
  { id: 'localApi' },
  { id: 'mcp' },
  { id: 'about' },
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
  await Promise.all([
    store.settings ? undefined : store.load(),
    store.loadWindow(),
    app.info ? undefined : app.load(),
    updates.support ? undefined : updates.loadSupport(),
    sources.load(),
    catalogs.load(),
  ]);
});
onBeforeUnmount(() => sources.stop());

const s = computed(() => store.settings);
/** Online catalogs: the Adobe Fonts entry shows its note when opened. */
const adobeFontsOpen = ref(false);
/** The last failure of any store this window reads. */
const error = computed(() => store.error ?? sources.error ?? app.error);

/** Radio choices; each change is saved at once. */
function choose(patch: SettingsUpdate): void {
  void store.update(patch);
}

/**
 * "Start at login": the OS may refuse the login entry; then a toast says so and
 * the switch shows the state fm-api reports again.
 */
async function saveStartAtLogin(e: Event): Promise<void> {
  const input = e.target as HTMLInputElement;
  if (await store.update({ startAtLogin: input.checked })) return;
  toast.show(store.error ? errorMessage(store.error) : t('settings.activation.startAtLoginFailed'));
  input.checked = store.settings?.startAtLogin ?? false;
}

type NumberKey = keyof Pick<FullSettings, 'cacheLimitMb' | 'rescanIntervalMinutes'>;

/** A number field: whole numbers are sent; anything else shows the saved value again. */
async function saveNumber(key: NumberKey, e: Event): Promise<void> {
  const input = e.target as HTMLInputElement;
  const text = input.value.trim();
  const ok = /^\d{1,10}$/.test(text) && (await store.update({ [key]: Number(text) }));
  if (!ok && store.settings) input.value = String(store.settings[key]);
}

// Identifiers the template uses, kept here: the catalog check (src/i18n/templates.test.ts)
// reads a camelCase literal in a template as text.
const saveCacheLimit = (e: Event) => saveNumber('cacheLimitMb', e);
const saveRescanInterval = (e: Event) => saveNumber('rescanIntervalMinutes', e);
const KEEP_EXISTING = 'keepExisting' satisfies FullSettings['onConflict'];
const upToDate = computed(() => updates.status === 'upToDate');

/** A section's title, which is also its heading. */
const sectionTitle = (id: SectionId) => t(`settings.sections.${id}` as MessageKey);

/** A locale's own name for itself (`Deutsch`), from its catalog. */
function localeName(tag: string): string {
  const name = (CATALOGS[tag]?.meta as Catalog | undefined)?.name;
  return typeof name === 'string' ? name : tag;
}

/** The locale "System" resolves to on this computer, by its own name. */
const systemLocaleName = computed(() => {
  const preferred = navigator.languages?.length ? navigator.languages : [navigator.language];
  return localeName(resolveLocale('system', preferred));
});

function chooseLanguage(e: Event): void {
  choose({ language: (e.target as HTMLSelectElement).value as Language });
}
</script>

<template>
  <div class="settings" @keydown="onKey">
    <header class="bar">
      <h1 id="settings-title" class="title">{{ t('settings.title') }}</h1>
      <button type="button" class="button" aria-keyshortcuts="Escape" @click="close">
        {{ t('common.close') }}
      </button>
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
          {{ sectionTitle(section.id) }}
        </button>
      </div>

      <div
        :id="`panel-${current}`"
        role="tabpanel"
        :aria-labelledby="`tab-${current}`"
        tabindex="0"
        class="panel"
      >
        <p v-if="error" role="alert" class="error">{{ errorMessage(error) }}</p>

        <template v-if="current === 'sources'">
          <h2 class="heading">{{ sectionTitle('sources') }}</h2>
          <p class="note">{{ t('settings.sources.note') }}</p>
          <ul class="sources" :aria-label="sectionTitle('sources')">
            <li v-for="source in sources.sources" :key="source.id" class="source">
              <div class="source-text">
                <span class="path">{{ source.path }}</span>
                <span class="meta">
                  {{ t(`settings.sources.kinds.${source.kind}`) }} ·
                  {{ t('settings.sources.fonts', { count: source.faceCount }) }}
                </span>
              </div>
              <label class="check">
                <input
                  type="checkbox"
                  :checked="source.paused"
                  :aria-label="t('settings.sources.pause', { path: source.path })"
                  @change="
                    sources.setPaused(source.id, ($event.target as HTMLInputElement).checked)
                  "
                />
                {{ t('settings.sources.paused') }}
              </label>
              <button
                type="button"
                class="button"
                :aria-label="t('settings.sources.remove', { path: source.path })"
                :disabled="source.kind === 'system'"
                :title="
                  source.kind === 'system' ? t('settings.sources.systemCannotRemove') : undefined
                "
                @click="sources.remove(source.id)"
              >
                {{ t('common.remove') }}
              </button>
            </li>
          </ul>
          <div class="add">
            <button type="button" class="button" @click="sources.add">
              {{ t('settings.sources.addFolder') }}
            </button>
          </div>
        </template>

        <template v-else-if="current === 'activation' && s">
          <h2 class="heading">{{ sectionTitle('activation') }}</h2>
          <fieldset class="group">
            <legend>{{ t('settings.activation.modeLegend') }}</legend>
            <label class="choice">
              <input
                type="radio"
                name="activation-mode"
                :checked="s.activationMode === 'session'"
                @change="choose({ activationMode: 'session' })"
              />
              {{ t('settings.activation.modeSession') }}
            </label>
            <label class="choice">
              <input
                type="radio"
                name="activation-mode"
                :checked="s.activationMode === 'permanent'"
                @change="choose({ activationMode: 'permanent' })"
              />
              {{ t('settings.activation.modePermanent') }}
            </label>
          </fieldset>
          <fieldset class="group">
            <legend>{{ t('settings.activation.conflictLegend') }}</legend>
            <label class="choice">
              <input
                type="radio"
                name="on-conflict"
                :checked="s.onConflict === KEEP_EXISTING"
                @change="choose({ onConflict: KEEP_EXISTING })"
              />
              {{ t('settings.activation.conflictAsk') }}
            </label>
            <label class="choice">
              <input
                type="radio"
                name="on-conflict"
                :checked="s.onConflict === 'replace'"
                @change="choose({ onConflict: 'replace' })"
              />
              {{ t('settings.activation.conflictReplace') }}
            </label>
          </fieldset>
          <label class="choice">
            <input
              type="checkbox"
              name="reactivate-on-launch"
              :checked="s.reactivateOnLaunch"
              @change="choose({ reactivateOnLaunch: ($event.target as HTMLInputElement).checked })"
            />
            {{ t('settings.activation.reactivateOnLaunch') }}
          </label>
          <label class="choice">
            <input
              type="checkbox"
              name="keep-in-tray"
              aria-describedby="keep-in-tray-hint"
              :checked="s.keepInTray"
              @change="choose({ keepInTray: ($event.target as HTMLInputElement).checked })"
            />
            {{ t('settings.activation.keepInTray') }}
          </label>
          <p id="keep-in-tray-hint" class="note hint">
            {{ t('settings.activation.keepInTrayHint') }}
          </p>
          <!-- fm-api reports null where the system does not offer it (ADR 0011). -->
          <template v-if="s.startAtLogin != null">
            <label class="choice start-at-login">
              <input
                type="checkbox"
                name="start-at-login"
                aria-describedby="start-at-login-hint"
                :checked="s.startAtLogin"
                @change="saveStartAtLogin"
              />
              {{ t('settings.activation.startAtLogin') }}
            </label>
            <p id="start-at-login-hint" class="note hint">
              {{ t('settings.activation.startAtLoginHint') }}
            </p>
          </template>
        </template>

        <template v-else-if="current === 'appearance' && s">
          <h2 class="heading">{{ sectionTitle('appearance') }}</h2>
          <fieldset class="group">
            <legend>{{ t('settings.appearance.theme') }}</legend>
            <label
              v-for="value in ['system', 'light', 'dark'] as const"
              :key="value"
              class="choice"
            >
              <input
                type="radio"
                name="theme"
                :checked="s.theme === value"
                @change="choose({ theme: value })"
              />
              {{ t(`settings.appearance.themes.${value}`) }}
            </label>
          </fieldset>
          <fieldset class="group">
            <legend>{{ t('settings.appearance.density') }}</legend>
            <label v-for="value in ['comfortable', 'compact'] as const" :key="value" class="choice">
              <input
                type="radio"
                name="density"
                :checked="s.density === value"
                @change="choose({ density: value })"
              />
              {{ t(`settings.appearance.densities.${value}`) }}
            </label>
          </fieldset>
          <label class="field">
            <span>{{ t('language.label') }}</span>
            <select
              name="language"
              class="input select"
              aria-describedby="language-hint"
              :value="s.language ?? 'system'"
              @change="chooseLanguage"
            >
              <option value="system">
                {{ t('language.system', { name: systemLocaleName }) }}
              </option>
              <option v-for="tag in LOCALES" :key="tag" :value="tag">{{ localeName(tag) }}</option>
            </select>
          </label>
          <p id="language-hint" class="note">{{ t('language.hint') }}</p>
        </template>

        <template v-else-if="current === 'catalogs'">
          <h2 class="heading">{{ sectionTitle('catalogs') }}</h2>
          <p class="note">{{ t('settings.catalogs.note') }}</p>
          <template v-for="c in catalogs.catalogs" :key="c.id">
            <label class="choice">
              <input
                type="checkbox"
                :name="`catalog-${c.id}`"
                :aria-describedby="`catalog-hint-${c.id}`"
                :checked="c.enabled"
                :disabled="catalogs.busy === c.id"
                @change="catalogs.setEnabled(c.id, ($event.target as HTMLInputElement).checked)"
              />
              <bdi>{{ c.name }}</bdi>
            </label>
            <p :id="`catalog-hint-${c.id}`" class="note hint">
              {{ t('settings.catalogs.connectsHint', { name: c.name }) }}
            </p>
          </template>
          <p v-if="catalogs.error" role="alert" class="note">
            {{ errorMessage(catalogs.error) }}
          </p>
          <button
            type="button"
            class="button"
            aria-controls="settings-adobe-fonts"
            :aria-expanded="adobeFontsOpen"
            @click="adobeFontsOpen = !adobeFontsOpen"
          >
            {{ t('catalogs.adobeFonts.name') }}
          </button>
          <AdobeFontsNote v-if="adobeFontsOpen" id="settings-adobe-fonts" :level="3" />
        </template>

        <template v-else-if="current === 'advanced' && s">
          <h2 class="heading">{{ sectionTitle('advanced') }}</h2>
          <label class="field">
            <span>{{ t('settings.advanced.cacheSize') }}</span>
            <input
              type="number"
              class="input number"
              inputmode="numeric"
              :value="s.cacheLimitMb"
              @change="saveCacheLimit"
            />
          </label>
          <label class="field">
            <span>{{ t('settings.advanced.rescanInterval') }}</span>
            <input
              type="number"
              class="input number"
              inputmode="numeric"
              aria-describedby="rescan-hint"
              :value="s.rescanIntervalMinutes"
              @change="saveRescanInterval"
            />
          </label>
          <p id="rescan-hint" class="note">{{ t('settings.advanced.rescanHint') }}</p>
          <div v-if="store.folders" class="field">
            <span>{{ t('settings.advanced.logsFolder') }}</span>
            <span>
              <CopyValue
                :value="store.folders.logs"
                :label="t('settings.advanced.logsFolderLabel')"
              />
            </span>
          </div>
        </template>

        <LocalApiSettings v-else-if="current === 'localApi'" />

        <McpSettings v-else-if="current === 'mcp'" />

        <template v-else-if="current === 'about' && app.info">
          <h2 class="heading">{{ sectionTitle('about') }}</h2>
          <p class="note">{{ app.info.name }} {{ app.info.version }}</p>
          <!-- Hidden where Flatpak or a package manager installs the updates; the
               platform notes below say so (ADR 0012). -->
          <section v-if="updates.support?.offered" class="updates" aria-labelledby="updates-title">
            <h3 id="updates-title" class="subheading">{{ t('updates.title') }}</h3>
            <div class="add">
              <button
                type="button"
                class="button"
                :disabled="updates.status === 'checking' || updates.status === 'installing'"
                @click="updates.check()"
              >
                {{ t('updates.check') }}
              </button>
              <button
                v-if="updates.status === 'available' && updates.update && !updates.open"
                type="button"
                class="button"
                @click="updates.open = true"
              >
                {{ t('updates.showVersion', { version: updates.update.version }) }}
              </button>
            </div>
            <p role="status" class="note status">
              <template v-if="updates.status === 'checking'">{{ t('updates.checking') }}</template>
              <template v-else-if="upToDate">{{ t('updates.upToDate') }}</template>
              <template v-else-if="updates.status === 'available' && updates.update">{{
                t('updates.available', { version: updates.update.version })
              }}</template>
            </p>
            <p v-if="updates.error && !updates.open" role="alert" class="error">
              {{ errorMessage(updates.error) }}
            </p>
            <label v-if="s" class="choice">
              <input
                type="checkbox"
                name="check-updates-automatically"
                aria-describedby="check-updates-hint"
                :checked="s.checkUpdatesAutomatically"
                @change="
                  choose({ checkUpdatesAutomatically: ($event.target as HTMLInputElement).checked })
                "
              />
              {{ t('updates.checkAutomatically') }}
            </label>
            <p id="check-updates-hint" class="note hint">
              {{ t('updates.checkAutomaticallyHint') }}
            </p>
          </section>
          <h3 id="platform-notes-title" class="subheading">
            {{ t('settings.about.platformNotes') }}
          </h3>
          <ul
            v-if="app.info.platformNotes?.length"
            class="notes"
            aria-labelledby="platform-notes-title"
          >
            <li v-for="note in app.info.platformNotes" :key="note">{{ note }}</li>
          </ul>
          <p v-else class="note">{{ t('settings.about.noPlatformNotes') }}</p>
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
  border-inline-end: 1px solid var(--fm-border);
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
.hint {
  margin-block: 4px 0;
  margin-inline: 22px 0;
}
.start-at-login {
  margin-top: 12px;
}
.updates {
  margin-bottom: 8px;
}
.status {
  margin-top: 8px;
}
.status:empty {
  display: none;
}
.subheading {
  margin: 16px 0 8px;
  font-size: 13px;
}
.notes {
  margin: 0;
  padding-inline-start: 20px;
}
.notes li + li {
  margin-top: 6px;
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
  gap: 8px;
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
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: inherit;
  font: inherit;
}
.number {
  width: 120px;
}
.select {
  align-self: flex-start;
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
