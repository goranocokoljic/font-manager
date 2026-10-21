<script setup lang="ts">
// The Quick Collection entry of the sidebar, with its count. Pressing it opens the
// review in the center; the parent view loads the store.
import { formatNumber, t } from '../i18n';
import { ACTIONS, ariaShortcut, chordOf, formatChord } from '../keyboard/keymap';
import { useQuickCollectionStore } from '../stores/quickCollection';

const store = useQuickCollectionStore();
const chord = chordOf(ACTIONS.quickCollection)!;
</script>

<template>
  <section class="quick" :aria-label="t('quickCollection.title')">
    <button
      type="button"
      class="entry"
      data-testid="quick-collection"
      :aria-current="store.open ? 'true' : undefined"
      :title="t('quickCollection.entryTitle', { chord: formatChord(chord) })"
      :aria-keyshortcuts="ariaShortcut(chord)"
      @click="store.show"
    >
      <span class="name">{{ t('quickCollection.title') }}</span>
      <span class="count" data-testid="quick-count">{{ formatNumber(store.count) }}</span>
    </button>
  </section>
</template>

<style scoped>
.quick {
  padding: 8px 8px 0;
}
.entry {
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
.entry:hover,
.entry:focus-visible {
  background: var(--fm-bg);
}
.entry[aria-current='true'] {
  color: var(--fm-accent);
  font-weight: 600;
}
.name {
  flex: 1;
}
.count {
  color: var(--fm-muted);
  font-variant-numeric: tabular-nums;
}
</style>
