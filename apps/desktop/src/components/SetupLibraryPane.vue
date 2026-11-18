<script setup lang="ts">
// The right pane of the setup's folders step (board 13, E55.4): the number of families found
// in the chosen folders so far and the first of them, each name drawn in its own face. The
// families and their number come from `search` (fm-core) through the setup store, which
// re-reads them as the scan events arrive; this file only lays them out.
import { computed } from 'vue';
import { t } from '../i18n';
import { useSetupStore } from '../stores/setup';
import { useSourcesStore } from '../stores/sources';
import FamilySample from './FamilySample.vue';

const setup = useSetupStore();
const sources = useSourcesStore();

/** The first chosen folder still being read, named in "Still reading …". */
const reading = computed(() => setup.folders.find((s) => sources.progressOf(s.id)));
const readingName = computed(() => {
  const path = reading.value?.path ?? '';
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
});
</script>

<template>
  <aside class="library-pane" data-testid="setup-library">
    <h2 class="caption">
      {{ setup.folders.length ? t('setup.library.titleSoFar') : t('setup.library.title') }}
    </h2>

    <div v-if="!setup.folders.length" class="empty">
      <p class="empty-title">{{ t('setup.library.emptyTitle') }}</p>
      <p class="note">{{ t('setup.library.emptyNote') }}</p>
    </div>

    <template v-else>
      <p class="count" aria-live="polite" data-testid="setup-found">
        {{ t('setup.library.found', { count: setup.found.total }) }}
      </p>
      <p class="note">
        {{ t('setup.library.inFolders', { count: setup.folders.length }) }}
        <template v-if="reading">{{
          t('setup.library.stillReading', { name: readingName })
        }}</template>
      </p>
      <ul class="families" :aria-label="t('setup.library.listLabel')">
        <li v-for="hit in setup.found.families" :key="hit.family.id" class="family">
          <span class="specimen">
            <template v-if="hit.faces[0] != null">
              <FamilySample
                :face="hit.faces[0]"
                :text="hit.family.name"
                variation=""
                aria-hidden="true"
              />
              <span class="sr-only">{{ hit.family.name }}</span>
            </template>
            <template v-else>{{ hit.family.name }}</template>
          </span>
          <span class="styles">{{
            hit.family.isVariable
              ? t('setup.library.variable')
              : t('setup.library.styles', { count: hit.family.styleCount })
          }}</span>
        </li>
      </ul>
      <p class="note footer">{{ t('setup.library.footer') }}</p>
    </template>
  </aside>
</template>

<style scoped>
.library-pane {
  display: flex;
  flex-direction: column;
  padding: 40px 64px 32px;
  overflow-y: auto;
  background-color: var(--fm-surface);
}
.caption {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.empty {
  margin: auto 0;
  max-width: 360px;
}
.empty-title {
  margin: 0 0 var(--fm-space-2);
  font-size: var(--fm-text-lg);
  font-weight: 500;
}
.count {
  margin: var(--fm-space-8) 0 var(--fm-space-1);
  font-size: var(--fm-text-lg);
  font-variant-numeric: tabular-nums;
}
.note {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  line-height: 1.55;
}
.families {
  margin: var(--fm-space-6) 0 0;
  padding: 0;
  border-top: 1px solid var(--fm-border);
  list-style: none;
}
.family {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--fm-space-4);
  padding: var(--fm-space-4) 0;
  border-bottom: 1px solid var(--fm-border);
}
.specimen {
  overflow: hidden;
  font-size: 40px;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.styles {
  flex: none;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.footer {
  margin-top: var(--fm-space-4);
}
</style>
