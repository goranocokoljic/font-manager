<script setup lang="ts">
// Setup step one, "Where are your fonts?" (board 13, E55.4). "Add folder…" opens the native
// folder picker, again and again; it takes no dropped folder (a drop gives the webview no path
// while in-window drags are on, ADR 0017, as in Settings › Sources). Each folder is a Sources
// card: name, full path, font count, the scan from the scan events, and a remove button. A
// folder the API refuses keeps a row with the API's message. Every fact comes from fm-api
// through the setup and sources stores; this file only lays it out.
import { nextTick, onMounted } from 'vue';
import type { Source } from '../api/types';
import { formatRelativeTime, t } from '../i18n';
import { useSetupStore } from '../stores/setup';
import { useSourcesStore } from '../stores/sources';
import UiBadge from './ui/UiBadge.vue';
import UiIcon from './ui/UiIcon.vue';
import UiIconButton from './ui/UiIconButton.vue';

const setup = useSetupStore();
const sources = useSourcesStore();

onMounted(async () => {
  await nextTick();
  document.getElementById('setup-step-heading')?.focus();
});

/** The folder's own name; the full path is on the line below. */
function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

/** A last read this recent reads "Read just now". */
const JUST_NOW_MS = 60_000;

function countLine(source: Source): string {
  const p = sources.progressOf(source.id);
  return p
    ? t('setup.folders.scanningCount', { done: p.done, total: p.total })
    : t('setup.folders.fonts', { count: source.faceCount });
}

function whenLine(source: Source): string {
  if (sources.progressOf(source.id)) return t('setup.folders.soFar');
  if (!source.lastScanned) return t('setup.folders.notRead');
  const when = new Date(source.lastScanned);
  if (Math.abs(Date.now() - when.getTime()) < JUST_NOW_MS) return t('setup.folders.readJustNow');
  return t('setup.folders.read', { when: formatRelativeTime(when) });
}

/** After a remove the focus stays in the step: on the next row's remove button or Add. */
async function remove(source: Source): Promise<void> {
  const ids = setup.folders.map((s) => s.id);
  const after = ids[ids.indexOf(source.id) + 1] ?? ids[ids.indexOf(source.id) - 1];
  await setup.removeFolder(source.id);
  await nextTick();
  (
    document.getElementById(`setup-remove-${source.id}`) ??
    (after != null ? document.getElementById(`setup-remove-${after}`) : null) ??
    document.getElementById('setup-add')
  )?.focus();
}

async function dismiss(key: number): Promise<void> {
  setup.dismiss(key);
  await nextTick();
  document.getElementById('setup-add')?.focus();
}
</script>

<template>
  <div class="folders-step">
    <h1 id="setup-step-heading" class="heading" tabindex="-1">
      {{ t('setup.folders.heading') }}
    </h1>
    <p class="lead">
      {{ t('setup.folders.lead') }}
      <span data-testid="setup-system-line">{{ t('setup.folders.system') }}</span>
    </p>

    <ul
      v-if="setup.folders.length || setup.refused.length"
      class="cards"
      :aria-label="t('setup.folders.listLabel')"
      data-testid="setup-folders"
    >
      <li
        v-for="source in setup.folders"
        :key="`s${source.id}`"
        class="card"
        :aria-label="folderName(source.path)"
        :data-source="source.id"
      >
        <span class="tile" aria-hidden="true">
          <UiIcon :name="source.cloud ? 'cloud' : 'folder'" />
        </span>
        <div class="text">
          <span class="name-line">
            <bdi class="name">{{ folderName(source.path) }}</bdi>
            <UiBadge v-if="sources.progressOf(source.id)" tone="accent">{{
              t('setup.folders.scanningBadge')
            }}</UiBadge>
          </span>
          <bdi class="path" :title="source.path">{{ source.path }}</bdi>
          <progress
            v-if="sources.progressOf(source.id)"
            class="bar"
            :value="sources.progressOf(source.id)!.done"
            :max="sources.progressOf(source.id)!.total || 1"
            :aria-label="t('setup.folders.scanningLabel', { name: folderName(source.path) })"
          />
          <span v-if="setup.removeErrors[source.id]" class="refusal" role="alert">{{
            setup.removeErrors[source.id]
          }}</span>
        </div>
        <div class="stats" data-testid="setup-folder-count">
          <span>{{ countLine(source) }}</span>
          <span class="when">{{ whenLine(source) }}</span>
        </div>
        <UiIconButton
          :id="`setup-remove-${source.id}`"
          icon="close"
          :label="t('setup.folders.removeLabel', { name: folderName(source.path) })"
          @click="remove(source)"
        />
      </li>
      <li
        v-for="row in setup.refused"
        :key="`r${row.key}`"
        class="card refused"
        :aria-label="folderName(row.path)"
        data-testid="setup-refused"
      >
        <span class="tile" aria-hidden="true"><UiIcon name="folder" /></span>
        <div class="text">
          <span class="name-line">
            <bdi class="name">{{ folderName(row.path) }}</bdi>
            <UiBadge tone="warning">{{ t('setup.folders.refusedBadge') }}</UiBadge>
          </span>
          <bdi class="path" :title="row.path">{{ row.path }}</bdi>
          <span class="refusal" role="alert">{{ row.message }}</span>
        </div>
        <span />
        <UiIconButton
          icon="close"
          :label="t('setup.folders.dismissLabel', { name: folderName(row.path) })"
          @click="dismiss(row.key)"
        />
      </li>
    </ul>

    <!-- A click opens the folder picker; the zone takes no drop (ADR 0017). -->
    <div v-if="!setup.folders.length" class="zone empty">
      <span class="tile" aria-hidden="true"><UiIcon name="folder" /></span>
      <p class="empty-title">{{ t('setup.folders.emptyTitle') }}</p>
      <p class="empty-note">{{ t('setup.folders.emptyNote') }}</p>
      <button
        id="setup-add"
        type="button"
        class="primary"
        data-testid="setup-add"
        @click="setup.addFolder()"
      >
        <UiIcon name="plus" />
        {{ t('setup.folders.add') }}
      </button>
    </div>
    <button
      v-else
      id="setup-add"
      type="button"
      class="zone another"
      data-testid="setup-add"
      @click="setup.addFolder()"
    >
      <UiIcon name="plus" />
      {{ t('setup.folders.addAnother') }}
    </button>
  </div>
</template>

<style scoped>
.heading {
  margin: 0;
  font-size: var(--fm-text-xl);
  font-weight: 600;
  letter-spacing: -0.01em;
}
/* The heading takes the focus by script (tabindex -1, never a Tab stop) so a screen reader
   starts there; it is not a control, so it draws no ring. */
.heading:focus {
  outline: none;
}
.lead {
  margin: var(--fm-space-3) 0 var(--fm-space-6);
  color: var(--fm-muted);
  line-height: 1.55;
}
.cards {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
.card {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto auto;
  align-items: center;
  gap: var(--fm-space-3);
  padding: var(--fm-space-3) var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface-2);
}
.tile {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-hover);
  color: var(--fm-fg);
}
.refused .tile {
  background-color: color-mix(in srgb, var(--fm-warning) 14%, transparent);
  color: var(--fm-warning);
}
.text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.name-line {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  min-width: 0;
}
.name {
  overflow: hidden;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.path {
  overflow: hidden;
  color: var(--fm-muted);
  font-family: var(--fm-font-mono);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.refusal {
  color: var(--fm-danger);
  font-size: var(--fm-text-sm);
}
.bar {
  width: 100%;
  height: 3px;
  margin-top: var(--fm-space-1);
  border: 0;
  border-radius: 2px;
  background-color: var(--fm-border);
  appearance: none;
}
.bar::-webkit-progress-bar {
  border-radius: 2px;
  background-color: var(--fm-border);
}
.bar::-webkit-progress-value {
  border-radius: 2px;
  background-color: var(--fm-accent);
}
.bar::-moz-progress-bar {
  border-radius: 2px;
  background-color: var(--fm-accent);
}
.stats {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  font-size: var(--fm-text-sm);
  text-align: end;
  white-space: nowrap;
}
.when {
  color: var(--fm-muted);
}
.zone {
  box-sizing: border-box;
  width: 100%;
  margin-top: var(--fm-space-2);
  border: 1.5px dashed var(--fm-border-strong);
  border-radius: var(--fm-radius-lg);
  background: none;
  font: inherit;
}
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-top: 0;
  padding: var(--fm-space-8) var(--fm-space-4);
  text-align: center;
}
.empty-title {
  margin: var(--fm-space-3) 0 0;
  font-weight: 500;
}
.empty-note {
  margin: var(--fm-space-1) 0 var(--fm-space-3);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.another {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--fm-space-2);
  min-height: 48px;
  color: var(--fm-muted);
  cursor: pointer;
}
.another:hover {
  border-color: var(--fm-fg);
  color: var(--fm-fg);
}
.primary {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-1);
  height: 32px;
  padding: 0 var(--fm-space-3);
  border: 1px solid var(--fm-accent);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font: inherit;
  font-weight: 600;
}
</style>
