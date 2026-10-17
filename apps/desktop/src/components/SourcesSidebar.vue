<script setup lang="ts">
// The Sources group of the sidebar: add a folder with the native picker,
// pause, resume, rescan or remove it, and see scan progress inline. The
// System source is listed read-only: the app never pauses or removes it.
import { onBeforeUnmount, onMounted } from 'vue';
import type { Source } from '../api/types';
import { useSourcesStore, type ScanProgress } from '../stores/sources';

const store = useSourcesStore();
onMounted(() => store.load());
onBeforeUnmount(() => store.stop());

/** The folder's own name; the full path is in the tooltip. */
function folderName(source: Source): string {
  if (source.kind === 'system') return 'System fonts';
  const parts = source.path.split(/[\\/]/).filter(Boolean);
  return parts.at(-1) ?? source.path;
}

function progressText(p: ScanProgress): string {
  return `${p.done} of ${p.total}`;
}
</script>

<template>
  <section class="sources" aria-labelledby="fm-sources-heading">
    <header class="head">
      <h2 id="fm-sources-heading" class="heading">Sources</h2>
      <button type="button" class="add" title="Add a folder to the library" @click="store.add">
        Add folder
      </button>
    </header>
    <p v-if="store.error" role="alert" class="error">{{ store.error.message }}</p>
    <div v-if="store.otherScan" class="scan library-scan" data-testid="library-scan">
      <progress
        :value="store.otherScan.done"
        :max="store.otherScan.total || 1"
        aria-label="Scanning the library"
      />
      <span class="scan-text">Scanning {{ progressText(store.otherScan) }}</span>
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
          <span v-if="source.kind === 'system'" class="badge">Read-only</span>
          <span v-else-if="source.paused" class="badge">Paused</span>
          <span class="count" :aria-label="`${source.faceCount} fonts`">{{
            source.faceCount
          }}</span>
        </div>
        <div v-if="store.progressOf(source.id)" class="scan">
          <progress
            :value="store.progressOf(source.id)!.done"
            :max="store.progressOf(source.id)!.total || 1"
            :aria-label="`Scanning ${folderName(source)}`"
          />
          <span class="scan-text">{{ progressText(store.progressOf(source.id)!) }}</span>
        </div>
        <div v-if="source.kind !== 'system'" class="actions">
          <button
            type="button"
            :aria-label="`Rescan ${folderName(source)}`"
            @click="store.rescan(source.id)"
          >
            Rescan
          </button>
          <button
            type="button"
            :aria-label="`${source.paused ? 'Resume' : 'Pause'} ${folderName(source)}`"
            @click="store.setPaused(source.id, !source.paused)"
          >
            {{ source.paused ? 'Resume' : 'Pause' }}
          </button>
          <button
            type="button"
            :aria-label="`Remove ${folderName(source)} from the library`"
            title="Remove from the library. The files stay where they are."
            @click="store.remove(source.id)"
          >
            Remove
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
.error {
  margin: 4px 8px;
  color: var(--fm-danger);
}
</style>
