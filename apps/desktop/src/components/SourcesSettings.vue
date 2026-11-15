<script setup lang="ts">
// Settings › Sources (E50.13, 11-sources.png): one card per folder with its icon (a cloud
// for a folder a sync client keeps), name, path, font count and last scan, a Watch switch
// and, on hover or focus, Rescan · Show in file manager · Remove. A running scan shows a
// badge and a 3 px bar fed by the scan events; a folder that is not there shows Missing.
// Under the cards: the dashed zone that opens the folder picker (also the empty state), the problems strip and the System
// fonts card. Removing asks once; it removes the index entries only (rule 2). Every fact
// shown (missing, cloud, last scan, counts) comes from fm-api; this file only lays it out.
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { RouterLink } from 'vue-router';
import type { Source, SourceId } from '../api/types';
import { formatRelativeTime, t, type MessageKey } from '../i18n';
import { PROBLEM_GROUPS, useProblemsStore } from '../stores/problems';
import { useSourcesStore } from '../stores/sources';
import UiBadge from './ui/UiBadge.vue';
import UiButton from './ui/UiButton.vue';
import UiIcon from './ui/UiIcon.vue';
import UiIconButton from './ui/UiIconButton.vue';
import UiSwitch from './ui/UiSwitch.vue';

const sources = useSourcesStore();
const problems = useProblemsStore();

onMounted(() => problems.load());
onBeforeUnmount(() => problems.stop());

/** The folders the user chose (and the library folder); the system fonts have their own card. */
const folders = computed(() => sources.sources.filter((s) => s.kind !== 'system'));
const system = computed(() => sources.sources.find((s) => s.kind === 'system'));

/** The folder's own name; the full path is on the line below. */
function folderName(source: Source): string {
  const parts = source.path.split(/[\\/]/).filter(Boolean);
  return parts.at(-1) ?? source.path;
}

/** The second line on the right: the last scan, the running one, or why there is none. */
function scanLine(source: Source): string {
  if (sources.progressOf(source.id)) return t('settings.sources.scanningNow');
  if (source.missing) return t('settings.sources.notFound');
  if (!source.lastScanned) return t('settings.sources.neverScanned');
  return t('settings.sources.scanned', { when: formatRelativeTime(new Date(source.lastScanned)) });
}

/** The first line on the right: the font count, or how far the running scan is. */
function countLine(source: Source): string {
  const p = sources.progressOf(source.id);
  return p
    ? t('settings.sources.scanningCount', { done: p.done, total: p.total })
    : t('settings.sources.fonts', { count: source.faceCount });
}

/** "3 duplicates, 2 broken files": the groups of the Problems view that have entries. */
const problemDetail = computed(() =>
  PROBLEM_GROUPS.filter((g) => problems.counts[g.id] > 0)
    .map((g) =>
      t(`settings.sources.problemCounts.${g.id}` as MessageKey, { count: problems.counts[g.id] }),
    )
    .join(', '),
);

/** The source whose removal waits for the user's answer. */
const confirming = ref<SourceId | null>(null);

async function askRemove(source: Source): Promise<void> {
  confirming.value = source.id;
  await nextTick();
  document.getElementById(`source-confirm-${source.id}`)?.focus();
}

/** Closes the question and gives the focus back to the card's Remove button. */
async function cancelRemove(id: SourceId): Promise<void> {
  confirming.value = null;
  await nextTick();
  document.getElementById(`source-remove-${id}`)?.focus();
}

async function confirmRemove(id: SourceId): Promise<void> {
  confirming.value = null;
  await sources.remove(id);
  // The card is gone (or stays with an error): the heading's button keeps the focus in the panel.
  await nextTick();
  (
    document.getElementById(`source-remove-${id}`) ?? document.getElementById('sources-add')
  )?.focus();
}

function onConfirmKey(e: KeyboardEvent, id: SourceId): void {
  if (e.key !== 'Escape') return;
  // Escape answers the question; it must not also close Settings.
  e.preventDefault();
  e.stopPropagation();
  void cancelRemove(id);
}
</script>

<template>
  <section class="sources-settings" aria-labelledby="sources-title">
    <div class="head">
      <div class="head-text">
        <h2 id="sources-title" class="heading">{{ t('settings.sections.sources') }}</h2>
        <p class="note">{{ t('settings.sources.note') }}</p>
      </div>
      <button id="sources-add" type="button" class="primary" @click="sources.add">
        <UiIcon name="plus" />
        {{ t('settings.sources.addFolder') }}
      </button>
    </div>

    <ul v-if="folders.length" class="cards" :aria-label="t('settings.sources.listLabel')">
      <li
        v-for="source in folders"
        :key="source.id"
        class="card"
        :class="{ missing: source.missing }"
        :aria-label="folderName(source)"
        :data-source="source.id"
      >
        <span class="tile" aria-hidden="true">
          <UiIcon :name="source.cloud ? 'cloud' : 'folder'" />
        </span>
        <div class="text">
          <span class="name-line">
            <bdi class="name">{{ folderName(source) }}</bdi>
            <UiBadge v-if="sources.progressOf(source.id)" tone="accent" class="badge">{{
              t('settings.sources.scanningBadge')
            }}</UiBadge>
            <UiBadge v-else-if="source.missing" tone="warning" class="badge">{{
              t('settings.sources.missingBadge')
            }}</UiBadge>
          </span>
          <bdi class="path" :title="source.path">{{ source.path }}</bdi>
          <progress
            v-if="sources.progressOf(source.id)"
            class="bar"
            :value="sources.progressOf(source.id)!.done"
            :max="sources.progressOf(source.id)!.total || 1"
            :aria-label="t('settings.sources.scanningLabel', { name: folderName(source) })"
          />
        </div>
        <div class="stats">
          <span class="count">{{ countLine(source) }}</span>
          <span class="when">{{ scanLine(source) }}</span>
        </div>
        <div class="actions">
          <UiIconButton
            icon="rescan"
            :label="t('settings.sources.rescanLabel', { name: folderName(source) })"
            :disabled="source.missing || !!sources.progressOf(source.id)"
            @click="sources.rescan(source.id)"
          />
          <UiIconButton
            icon="folderOpen"
            :label="t('settings.sources.showLabel', { name: folderName(source) })"
            :disabled="source.missing"
            @click="sources.showFolder(source.id)"
          />
          <UiIconButton
            :id="`source-remove-${source.id}`"
            icon="trash"
            :label="t('settings.sources.removeLabel', { name: folderName(source) })"
            @click="askRemove(source)"
          />
        </div>
        <div class="watch">
          <span aria-hidden="true">{{ t('settings.sources.watch') }}</span>
          <UiSwitch
            :checked="!source.paused"
            :aria-label="t('settings.sources.watchLabel', { name: folderName(source) })"
            @click="sources.setPaused(source.id, !source.paused)"
          />
        </div>
        <div
          v-if="confirming === source.id"
          role="alertdialog"
          class="confirm"
          :aria-labelledby="`source-confirm-text-${source.id}`"
          @keydown="onConfirmKey($event, source.id)"
        >
          <p :id="`source-confirm-text-${source.id}`" class="confirm-text">
            {{ t('settings.sources.confirmRemove', { name: folderName(source) }) }}
          </p>
          <UiButton @click="cancelRemove(source.id)">{{ t('common.cancel') }}</UiButton>
          <button
            :id="`source-confirm-${source.id}`"
            type="button"
            class="primary danger"
            @click="confirmRemove(source.id)"
          >
            {{ t('common.remove') }}
          </button>
        </div>
      </li>
    </ul>

    <!-- A click opens the folder picker. A dropped folder gives the webview no path while
         in-window drags are on (ADR 0017), so the zone does not take a drop. -->
    <button type="button" class="drop" data-testid="sources-drop" @click="sources.add">
      <UiIcon name="export" />
      <span>{{ folders.length ? t('settings.sources.drop') : t('settings.sources.empty') }}</span>
    </button>

    <div v-if="problems.total" class="problems" data-testid="sources-problems">
      <span class="dot" aria-hidden="true" />
      <span class="problems-text">
        <strong>{{ t('home.problems', { count: problems.total }) }}</strong>
        {{ t('settings.sources.problemsIn', { detail: problemDetail }) }}
      </span>
      <RouterLink :to="{ name: 'problems' }" class="link">{{
        t('settings.sources.openProblems')
      }}</RouterLink>
    </div>

    <template v-if="system">
      <h3 id="system-fonts-title" class="caption">{{ t('settings.sources.systemTitle') }}</h3>
      <div class="system" role="group" aria-labelledby="system-fonts-title">
        <p class="system-note">{{ t('settings.sources.systemNote') }}</p>
        <div class="watch">
          <span aria-hidden="true">{{ t('settings.sources.show') }}</span>
          <UiSwitch
            :checked="!system.paused"
            :aria-label="t('settings.sources.showSystemLabel')"
            @click="sources.setPaused(system.id, !system.paused)"
          />
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: var(--fm-space-4);
  margin-bottom: var(--fm-space-4);
}
.heading {
  margin: 0 0 var(--fm-space-1);
  font-size: var(--fm-text-lg);
}
.note {
  max-width: 46ch;
  margin: 0;
  color: var(--fm-muted);
}
.primary {
  display: inline-flex;
  flex: none;
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
.primary.danger {
  height: 28px;
  border-color: var(--fm-danger);
  background-color: var(--fm-danger);
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
  position: relative;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto auto auto;
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
.card.missing .tile {
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
/* Rescan · Show · Remove: in the layout always (no jump), seen on hover or keyboard focus. */
.actions {
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
.card:hover .actions,
.card:focus-within .actions {
  opacity: 1;
}
.watch {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.confirm {
  display: flex;
  grid-column: 1 / -1;
  align-items: center;
  gap: var(--fm-space-2);
  padding-top: var(--fm-space-2);
  border-top: 1px solid var(--fm-border);
}
.confirm-text {
  flex: 1;
  margin: 0;
  font-size: var(--fm-text-sm);
}
.drop {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--fm-space-2);
  width: 100%;
  min-height: 72px;
  margin-top: var(--fm-space-2);
  padding: var(--fm-space-4);
  border: 1.5px dashed var(--fm-border-strong);
  border-radius: var(--fm-radius-lg);
  background: none;
  color: var(--fm-muted);
  font: inherit;
  text-align: center;
  cursor: pointer;
}
.drop:hover {
  border-color: var(--fm-fg);
  color: var(--fm-fg);
}
.problems {
  display: flex;
  align-items: center;
  gap: var(--fm-space-3);
  margin-top: var(--fm-space-4);
  padding: var(--fm-space-3) var(--fm-space-4);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface);
  font-size: var(--fm-text-sm);
}
.dot {
  flex: none;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background-color: var(--fm-warning);
}
.problems-text {
  flex: 1;
  color: var(--fm-muted);
}
.problems-text strong {
  color: var(--fm-fg);
  font-weight: 600;
}
.link {
  color: var(--fm-fg);
  text-decoration: underline;
}
.caption {
  margin: var(--fm-space-6) 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.system {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-3);
  padding: var(--fm-space-3) var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface-2);
}
.system-note {
  margin: 0;
  color: var(--fm-muted);
}
</style>
