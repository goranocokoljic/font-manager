<script setup lang="ts">
import UiFoldButton from './ui/UiFoldButton.vue';
import UiIconButton from './ui/UiIconButton.vue';
import { useFolded } from './useFolded';
// The Project sets group of the sidebar: one button per set. Pressing a set makes
// it the current set ("Deactivate everything not in the current set" keeps it
// active); pressing it again clears it. The parent view loads the sets store.
// "Import set…" and each set's "Export set…" go through the native file dialog
// (E28.2); after an import, the fonts this library does not have are listed.
import { errorMessage, formatNumber, t } from '../i18n';
import { useSetsStore } from '../stores/sets';

const store = useSetsStore();
const { folded, toggle } = useFolded('sets');
</script>

<template>
  <section class="sets" aria-labelledby="fm-sets-heading">
    <header class="head">
      <h2 id="fm-sets-heading" class="heading">
        <UiFoldButton :folded="folded" controls="fm-sets-body" icon="box" @toggle="toggle">
          {{ t('sets.heading') }}
        </UiFoldButton>
      </h2>
      <UiIconButton
        icon="import"
        :label="t('sets.import')"
        :title="t('sets.importTitle')"
        @click="store.importSet()"
      />
    </header>
    <div v-show="!folded" id="fm-sets-body">
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
          <UiIconButton
            class="export"
            icon="export"
            :label="t('sets.exportLabel', { name: set.name })"
            :title="t('sets.exportTitle')"
            @click="store.exportSet(set.id)"
          />
        </li>
      </ul>
      <p v-else class="note">{{ t('sets.empty') }}</p>
    </div>
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
  font-size: var(--fm-text-xs);
  font-weight: 600;
}
.list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.item {
  position: relative;
  display: flex;
  align-items: center;
}
.action {
  flex: none;
  padding: 2px 6px;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: inherit;
  font: inherit;
  font-size: var(--fm-text-xs);
}
/* A set's export icon takes the count's place under the pointer and under keyboard focus. */
.export {
  position: absolute;
  inset-inline-end: 4px;
  opacity: 0;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
.item:hover .export,
.item:focus-within .export {
  opacity: 1;
}
.item:hover .count,
.item:focus-within .count {
  visibility: hidden;
}
.imported {
  margin: 0 8px 4px;
  padding: 6px 8px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-sm);
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
  border-radius: var(--fm-radius-sm);
  background: none;
  color: inherit;
  font: inherit;
  text-align: start;
  cursor: pointer;
}
.set:hover,
.set:focus-visible {
  background-color: var(--fm-bg);
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
