<script setup lang="ts">
// The foot of the sidebar, pinned below its groups: Problems, the one place the app counts
// them (a dot and the total, which opens the Problems view; the view filters by kind), and
// the command palette's key. The parent view loads the problems store.
import { RouterLink } from 'vue-router';
import { t } from '../i18n';
import { ACTIONS, ariaShortcut, chordOf, formatChord } from '../keyboard/keymap';
import { useCommandsStore } from '../stores/commands';
import { useProblemsStore } from '../stores/problems';

const store = useProblemsStore();
const commands = useCommandsStore();
const chord = chordOf(ACTIONS.palette)!;
</script>

<template>
  <div class="foot">
    <section class="problems" aria-labelledby="fm-problems-heading">
      <h2 id="fm-problems-heading" class="sr-only">{{ t('problems.title') }}</h2>
      <RouterLink :to="{ name: 'problems' }" class="link" data-testid="problem-count">
        <span v-if="store.total" class="dot" data-testid="problem-dot" aria-hidden="true"></span>
        <template v-if="store.total">{{ t('home.problems', { count: store.total }) }}</template>
        <template v-else>{{ t('problems.noneShort') }}</template>
      </RouterLink>
    </section>
    <button
      type="button"
      class="palette"
      :aria-keyshortcuts="ariaShortcut(chord)"
      @click="commands.openPalette"
    >
      <span class="sr-only">{{ t('home.commands') }}</span>
      <kbd>{{ formatChord(chord) }}</kbd>
    </button>
  </div>
</template>

<style scoped>
/* After the groups at the foot of a short sidebar, and in sight while a long one scrolls. */
.foot {
  position: sticky;
  bottom: 0;
  z-index: var(--fm-z-sticky);
  display: flex;
  flex: none;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-2);
  margin-block-start: auto;
  padding: var(--fm-space-2) var(--fm-space-4);
  background-color: var(--fm-surface);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.link {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-2);
  min-height: 24px;
  color: inherit;
  text-decoration: none;
}
.link:hover,
.link:focus-visible {
  color: var(--fm-fg);
}
.dot {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: var(--fm-danger);
}
.palette {
  padding: 0;
  border: 0;
  background-color: transparent;
  color: inherit;
}
</style>
