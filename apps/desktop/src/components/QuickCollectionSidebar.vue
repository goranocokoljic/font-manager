<script setup lang="ts">
// The Quick Collection entry of the sidebar, with its count. Pressing it opens the
// review in the center; the parent view loads the store.
import { formatNumber, t } from '../i18n';
import { ACTIONS, ariaShortcut, chordOf, formatChord } from '../keyboard/keymap';
import { useQuickCollectionStore } from '../stores/quickCollection';
import UiIcon from './ui/UiIcon.vue';

const store = useQuickCollectionStore();
const chord = chordOf(ACTIONS.quickCollection)!;
</script>

<template>
  <section class="quick" :aria-label="t('quickCollection.title')">
    <button
      type="button"
      class="side-row"
      data-testid="quick-collection"
      :aria-current="store.open ? 'true' : undefined"
      :title="t('quickCollection.entryTitle', { chord: formatChord(chord) })"
      :aria-keyshortcuts="ariaShortcut(chord)"
      @click="store.show"
    >
      <span class="side-lead"><UiIcon name="zap" /></span>
      <span class="side-label">{{ t('quickCollection.title') }}</span>
      <span class="side-count" data-testid="quick-count">{{ formatNumber(store.count) }}</span>
    </button>
  </section>
</template>

<style scoped>
.quick {
  flex: none;
  padding-inline: var(--fm-space-2);
}
</style>
