<script setup lang="ts">
import UiBadge from './ui/UiBadge.vue';
import UiFoldButton from './ui/UiFoldButton.vue';
import UiIconButton from './ui/UiIconButton.vue';
// The Sources group of the sidebar: add a folder with the native picker,
// pause, resume, rescan or remove it, and see scan progress inline. The
// System source is listed read-only: the app never pauses or removes it.
// A live region that is always in the page reads the scan progress to screen
// readers (the progress bars come and go with the scan, too late to announce).
import { computed, onBeforeUnmount, onMounted } from 'vue';
import type { Source } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useFolded } from './useFolded';
import { useSourcesStore, type ScanProgress } from '../stores/sources';

const store = useSourcesStore();
onMounted(() => store.load());
onBeforeUnmount(() => store.stop());

/** The folder's own name; the full path is in the tooltip. */
function folderName(source: Source): string {
  if (source.kind === 'system') return t('sources.systemFonts');
  const parts = source.path.split(/[\\/]/).filter(Boolean);
  return parts.at(-1) ?? source.path;
}

function progressText(p: ScanProgress): string {
  return t('sources.progress', { done: p.done, total: p.total });
}

/** What the live region says: the first running scan, or nothing. */
const announcement = computed(() => {
  for (const source of store.sources) {
    const p = store.progressOf(source.id);
    if (p)
      return t('sources.scanningSourceProgress', {
        name: folderName(source),
        progress: progressText(p),
      });
  }
  const other = store.otherScan;
  return other ? t('sources.scanning', { progress: progressText(other) }) : '';
});

const { folded, toggle } = useFolded('sources');
</script>

<template>
  <section class="sources" aria-labelledby="fm-sources-heading">
    <header class="head">
      <h2 id="fm-sources-heading" class="heading">
        <UiFoldButton :folded="folded" controls="fm-sources-list" icon="folder" @toggle="toggle">{{
          t('sources.title')
        }}</UiFoldButton>
      </h2>
      <UiIconButton
        icon="plus"
        :label="t('sources.addFolder')"
        :title="t('sources.addFolderTitle')"
        @click="store.add"
      />
    </header>
    <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
    <p class="sr-only" role="status" data-testid="scan-status">{{ announcement }}</p>
    <div v-if="store.otherScan" class="scan library-scan" data-testid="library-scan">
      <progress
        :value="store.otherScan.done"
        :max="store.otherScan.total || 1"
        :aria-label="t('sources.scanningLibrary')"
      />
      <span class="scan-text">{{
        t('sources.scanning', { progress: progressText(store.otherScan) })
      }}</span>
    </div>
    <ul v-show="!folded" id="fm-sources-list" class="list" aria-labelledby="fm-sources-heading">
      <li
        v-for="source in store.sources"
        :key="source.id"
        class="source"
        :class="{ paused: source.paused }"
        :data-kind="source.kind"
        :aria-label="folderName(source)"
      >
        <div class="row">
          <span class="name" :title="source.path">{{ folderName(source) }}</span>
          <UiBadge v-if="source.kind === 'system'">{{ t('sources.readOnly') }}</UiBadge>
          <UiBadge v-else-if="source.paused">{{ t('sources.paused') }}</UiBadge>
          <span class="trail">
            <span class="count" :aria-label="t('sources.fonts', { count: source.faceCount })">{{
              source.faceCount
            }}</span>
            <span v-if="source.kind !== 'system'" class="actions">
              <UiIconButton
                icon="rescan"
                :label="t('sources.rescanLabel', { name: folderName(source) })"
                :title="t('sources.rescan')"
                @click="store.rescan(source.id)"
              />
              <UiIconButton
                :icon="source.paused ? 'play' : 'pause'"
                :label="
                  source.paused
                    ? t('sources.resumeLabel', { name: folderName(source) })
                    : t('sources.pauseLabel', { name: folderName(source) })
                "
                :title="source.paused ? t('sources.resume') : t('sources.pause')"
                @click="store.setPaused(source.id, !source.paused)"
              />
              <UiIconButton
                icon="trash"
                :label="t('sources.removeLabel', { name: folderName(source) })"
                :title="t('sources.removeTitle')"
                @click="store.remove(source.id)"
              />
            </span>
          </span>
        </div>
        <div v-if="store.progressOf(source.id)" class="scan">
          <progress
            :value="store.progressOf(source.id)!.done"
            :max="store.progressOf(source.id)!.total || 1"
            :aria-label="t('sources.scanningSource', { name: folderName(source) })"
          />
          <span class="scan-text">{{ progressText(store.progressOf(source.id)!) }}</span>
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.sources {
  padding: 8px;
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
}
.heading {
  margin: 0;
  padding: 8px 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
}
.list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.source {
  padding: 4px 8px;
  border-radius: var(--fm-radius-sm);
}
.source:hover,
.source:focus-within {
  background-color: var(--fm-bg);
}
.row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.paused .name {
  color: var(--fm-muted);
}
.count {
  color: var(--fm-muted);
  font-variant-numeric: tabular-nums;
}
.scan {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
.library-scan {
  padding: 4px 8px;
}
.scan progress {
  flex: 1;
  min-width: 0;
  height: 6px;
}
.scan-text {
  font-variant-numeric: tabular-nums;
}
.trail {
  display: grid;
  flex: none;
  justify-items: end;
  align-items: center;
}
.trail > * {
  grid-area: 1 / 1;
}
.actions {
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
.source:hover .actions,
.source:focus-within .actions {
  opacity: 1;
}
.source:hover .trail:has(.actions) .count,
.source:focus-within .trail:has(.actions) .count {
  visibility: hidden;
}
.error {
  margin: 4px 8px;
  color: var(--fm-danger);
}
</style>
