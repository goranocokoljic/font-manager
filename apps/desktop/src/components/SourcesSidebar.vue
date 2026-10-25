<script setup lang="ts">
// The Sources group of the sidebar: add a folder with the native picker,
// pause, resume, rescan or remove it, and see scan progress inline. The
// System source is listed read-only: the app never pauses or removes it.
// A live region that is always in the page reads the scan progress to screen
// readers (the progress bars come and go with the scan, too late to announce).
import { computed, onBeforeUnmount, onMounted } from 'vue';
import type { Source } from '../api/types';
import { errorMessage, t } from '../i18n';
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
</script>

<template>
  <section class="sources" aria-labelledby="fm-sources-heading">
    <header class="head">
      <h2 id="fm-sources-heading" class="heading">{{ t('sources.title') }}</h2>
      <button type="button" class="add" :title="t('sources.addFolderTitle')" @click="store.add">
        {{ t('sources.addFolder') }}
      </button>
    </header>
    <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
    <p class="announce" role="status" data-testid="scan-status">{{ announcement }}</p>
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
    <ul class="list" aria-labelledby="fm-sources-heading">
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
          <span v-if="source.kind === 'system'" class="badge">{{ t('sources.readOnly') }}</span>
          <span v-else-if="source.paused" class="badge">{{ t('sources.paused') }}</span>
          <span class="count" :aria-label="t('sources.fonts', { count: source.faceCount })">{{
            source.faceCount
          }}</span>
        </div>
        <div v-if="store.progressOf(source.id)" class="scan">
          <progress
            :value="store.progressOf(source.id)!.done"
            :max="store.progressOf(source.id)!.total || 1"
            :aria-label="t('sources.scanningSource', { name: folderName(source) })"
          />
          <span class="scan-text">{{ progressText(store.progressOf(source.id)!) }}</span>
        </div>
        <div v-if="source.kind !== 'system'" class="actions">
          <button
            type="button"
            :aria-label="t('sources.rescanLabel', { name: folderName(source) })"
            @click="store.rescan(source.id)"
          >
            {{ t('sources.rescan') }}
          </button>
          <button
            type="button"
            :aria-label="
              source.paused
                ? t('sources.resumeLabel', { name: folderName(source) })
                : t('sources.pauseLabel', { name: folderName(source) })
            "
            @click="store.setPaused(source.id, !source.paused)"
          >
            {{ source.paused ? t('sources.resume') : t('sources.pause') }}
          </button>
          <button
            type="button"
            :aria-label="t('sources.removeLabel', { name: folderName(source) })"
            :title="t('sources.removeTitle')"
            @click="store.remove(source.id)"
          >
            {{ t('common.remove') }}
          </button>
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
  font-size: 11.5px;
  font-weight: 600;
}
.add,
.actions button {
  padding: 2px 6px;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 11.5px;
}
.list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.source {
  padding: 4px 8px;
  border-radius: 6px;
}
.source:hover,
.source:focus-within {
  background: var(--fm-bg);
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
.badge {
  padding: 0 4px;
  border: 1px solid var(--fm-border);
  border-radius: 4px;
  color: var(--fm-muted);
  font-size: 10.5px;
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
  font-size: 11px;
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
.actions {
  display: flex;
  gap: 4px;
  padding-top: 4px;
}
/* Read by screen readers only; the progress bars show the same. */
.announce {
  position: absolute;
  width: 1px;
  height: 1px;
  margin: -1px;
  padding: 0;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}
.error {
  margin: 4px 8px;
  color: var(--fm-danger);
}
</style>
