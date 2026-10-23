<script setup lang="ts">
// The Project sets group of the sidebar: one button per set. Pressing a set makes
// it the current set ("Deactivate everything not in the current set" keeps it
// active); pressing it again clears it. The parent view loads the sets store.
import { errorMessage, formatNumber, t } from '../i18n';
import { useSetsStore } from '../stores/sets';

const store = useSetsStore();
</script>

<template>
  <section class="sets" aria-labelledby="fm-sets-heading">
    <h2 id="fm-sets-heading" class="heading">{{ t('sets.heading') }}</h2>
    <p v-if="store.error" class="note" role="alert">{{ errorMessage(store.error) }}</p>
    <ul v-else-if="store.sets.length" class="list" aria-labelledby="fm-sets-heading">
      <li v-for="set in store.sets" :key="set.id">
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
      </li>
    </ul>
    <p v-else class="note">{{ t('sets.empty') }}</p>
  </section>
</template>

<style scoped>
.sets {
  padding: 8px;
}
.heading {
  margin: 0;
  padding: 8px;
  color: var(--fm-muted);
  font-size: 11.5px;
  font-weight: 600;
}
.list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.set {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
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
