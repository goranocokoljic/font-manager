<script setup lang="ts">
import { ref } from 'vue';
import { t } from '../i18n';
import { ACTIONS, ariaShortcut, chordOf } from '../keyboard/keymap';
import { useActions } from '../stores/commands';
import { useSearchStore } from '../stores/search';

const search = useSearchStore();
const input = ref<HTMLInputElement | null>(null);
const chord = chordOf(ACTIONS.search)!;

useActions([
  {
    id: ACTIONS.search,
    // Getters: the palette shows the title in the language chosen now.
    get title() {
      return t('search.label');
    },
    get group() {
      return t('palette.groups.search');
    },
    run: () => {
      input.value?.focus();
      input.value?.select();
    },
  },
]);

/** Escape empties the box, then leaves it. */
function onEscape(): void {
  if (search.text) search.setText('');
  else input.value?.blur();
}
</script>

<template>
  <input
    ref="input"
    type="search"
    class="search"
    :aria-label="t('search.label')"
    :aria-keyshortcuts="ariaShortcut(chord)"
    :placeholder="t('search.placeholder')"
    autocomplete="off"
    spellcheck="false"
    :value="search.text"
    @input="search.setText(($event.target as HTMLInputElement).value)"
    @keydown.esc.prevent="onEscape"
  />
</template>

<style scoped>
.search {
  width: 220px;
  min-width: 120px;
  padding: 4px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.search:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -1px;
}
</style>
