<script setup lang="ts">
// The Project sets group of the sidebar: one button per set. Pressing a set makes
// it the current set ("Deactivate everything not in the current set" keeps it
// active); pressing it again clears it. The parent view loads the sets store.
// "Import set…" and each set's "Export set…" go through the native file dialog
// (E28.2); after an import, the fonts this library does not have are listed.
import { errorMessage, formatNumber, t } from '../i18n';
import { useSetsStore } from '../stores/sets';

const store = useSetsStore();
</script>

<template>
  <section class="sets" aria-labelledby="fm-sets-heading">
    <header class="head">
      <h2 id="fm-sets-heading" class="heading">{{ t('sets.heading') }}</h2>
      <button
        type="button"
        class="action"
        :title="t('sets.importTitle')"
        @click="store.importSet()"
      >
        {{ t('sets.import') }}
      </button>
    </header>
    <div v-if="store.imported" class="imported" role="status" aria-labelledby="fm-sets-imported">
      <p id="fm-sets-imported" class="imported-line">
        <span>{{
          store.imported.missing.length
            ? t('sets.imported', { name: store.imported.set.name })
            : t('sets.importedAll', { name: store.imported.set.name })
        }}</span>
        <button
          type="button"
          class="action"
          :aria-label="t('sets.dismissLabel')"
          @click="store.dismissImport()"
        >
          {{ t('common.close') }}
        </button>
      </p>
      <template v-if="store.imported.missing.length">
        <p id="fm-sets-missing" class="missing-heading">
          {{ t('sets.missing', { count: store.imported.missing.length }) }}
        </p>
        <ul class="missing" aria-labelledby="fm-sets-missing">
          <li v-for="(m, i) in store.imported.missing" :key="i">{{ m.family }} {{ m.style }}</li>
        </ul>
      </template>
    </div>
    <p v-if="store.error" class="note" role="alert">{{ errorMessage(store.error) }}</p>
    <ul v-else-if="store.sets.length" class="list" aria-labelledby="fm-sets-heading">
      <li v-for="set in store.sets" :key="set.id" class="item">
        <button
          type="button"
          class="set"
          :aria-pressed="store.current?.id === set.id"
          :data-set="set.id"
          :title="
            store.current?.id === set.id ? t('sets.currentTitle') : t('sets.makeCurrentTitle')
          "
          @click="store.pick(set.id)"
        >
          <span class="name">{{ set.name }}</span>
          <span class="count">{{ formatNumber(set.faces.length) }}</span>
        </button>
        <button
          type="button"
          class="action export"
          :aria-label="t('sets.exportLabel', { name: set.name })"
          :title="t('sets.exportTitle')"
          @click="store.exportSet(set.id)"
        >
          {{ t('sets.export') }}
        </button>
      </li>
    </ul>
    <p v-else class="note">{{ t('sets.empty') }}</p>
  </section>
</template>

<style scoped>
.sets {
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
.list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.item {
  display: flex;
  align-items: center;
  gap: 4px;
}
.action {
  flex: none;
  padding: 2px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 11.5px;
}
/* A set's "Export set…" shows under the pointer and under keyboard focus. */
.export {
  opacity: 0;
}
.item:hover .export,
.item:focus-within .export {
  opacity: 1;
}
.imported {
  margin: 0 8px 4px;
  padding: 6px 8px;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
}
.imported-line {
  display: flex;
  align-items: start;
  justify-content: space-between;
  gap: 6px;
  margin: 0;
}
.missing-heading {
  margin: 6px 0 2px;
  font-weight: 600;
}
.missing {
  margin: 0;
  padding-inline-start: 16px;
  color: var(--fm-muted);
}
.set {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 6px;
  min-width: 0;
  padding: 4px 8px;
  border: 0;
  border-radius: 6px;
  background: none;
  color: inherit;
  font: inherit;
  text-align: start;
  cursor: pointer;
}
.set:hover,
.set:focus-visible {
  background: var(--fm-bg);
}
.set[aria-pressed='true'] {
  color: var(--fm-accent);
  font-weight: 600;
}
.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.count,
.note {
  color: var(--fm-muted);
  font-variant-numeric: tabular-nums;
}
.note {
  margin: 0;
  padding: 4px 8px;
}
</style>
