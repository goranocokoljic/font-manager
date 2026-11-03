<script setup lang="ts">
// The shortcut sheet (`?`): every key of the keyboard model in three quiet columns.
// A modal dialog; `?` or Escape closes it and focus returns to where it was.
import { computed, nextTick, ref, watch } from 'vue';
import { t } from '../i18n';
import { ACTIONS, chordOf, formatChord } from '../keyboard/keymap';
import { useActions } from '../stores/commands';

const open = ref(false);
const dialog = ref<HTMLElement | null>(null);
let returnFocus: HTMLElement | null = null;

useActions([
  {
    id: ACTIONS.shortcuts,
    get title() {
      return t('shortcuts.toggle');
    },
    get group() {
      return t('palette.groups.general');
    },
    run: () => (open.value = !open.value),
  },
]);

const chord = (action: string) => {
  const c = chordOf(action);
  return c ? formatChord(c) : '';
};

const groups = computed(() => [
  {
    title: t('shortcuts.groups.go'),
    rows: [
      { label: t('shortcuts.rows.move'), keys: ['↑↓', 'J', 'K'] },
      { label: t('shortcuts.rows.extend'), keys: ['Shift+↑↓'] },
      { label: t('shortcuts.rows.search'), keys: [chord(ACTIONS.search)] },
      { label: t('shortcuts.rows.palette'), keys: [chord(ACTIONS.palette)] },
    ],
  },
  {
    title: t('shortcuts.groups.fonts'),
    rows: [
      { label: t('shortcuts.rows.quickLook'), keys: [chord(ACTIONS.quickLook)] },
      { label: t('shortcuts.rows.activate'), keys: [chord(ACTIONS.activateSession)] },
      { label: t('shortcuts.rows.activatePermanent'), keys: [chord(ACTIONS.activatePermanent)] },
      { label: t('shortcuts.rows.compare'), keys: [chord(ACTIONS.compare)] },
      { label: t('shortcuts.rows.quick'), keys: [chord(ACTIONS.quickCollection)] },
      { label: t('shortcuts.rows.tag'), keys: [chord(ACTIONS.tag)] },
      { label: t('shortcuts.rows.similar'), keys: [chord(ACTIONS.similar)] },
    ],
  },
  {
    title: t('shortcuts.groups.view'),
    rows: [
      { label: t('shortcuts.rows.views'), keys: ['1–5'] },
      { label: t('shortcuts.rows.sidebar'), keys: ['['] },
      { label: t('shortcuts.rows.inspector'), keys: [']'] },
      { label: t('shortcuts.rows.settings'), keys: [chord(ACTIONS.settings)] },
    ],
  },
]);

watch(open, async (isOpen) => {
  if (isOpen) {
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    await nextTick();
    dialog.value?.focus();
  } else {
    const target = returnFocus;
    returnFocus = null;
    await nextTick();
    if (target?.isConnected) target.focus();
  }
});

function onKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape' || e.key === '?') {
    open.value = false;
    e.preventDefault();
    e.stopPropagation();
  } else if (e.key === 'Tab') {
    // The dialog is the only tab stop; focus stays inside the modal.
    e.preventDefault();
  }
}
</script>

<template>
  <div v-if="open" class="backdrop" @mousedown.self="open = false">
    <div
      ref="dialog"
      class="sheet"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      :aria-label="t('shortcuts.label')"
      @keydown="onKeyDown"
    >
      <div class="cols">
        <section v-for="g in groups" :key="g.title" class="group">
          <h2>{{ g.title }}</h2>
          <dl>
            <div v-for="r in g.rows" :key="r.label" class="row">
              <dt>{{ r.label }}</dt>
              <dd>
                <kbd v-for="k in r.keys" :key="k">{{ k }}</kbd>
              </dd>
            </div>
          </dl>
        </section>
      </div>
      <p class="hint">{{ t('shortcuts.hint') }}</p>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: var(--fm-z-popover);
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 12vh;
  background: var(--fm-scrim);
}
.sheet {
  width: min(760px, 92vw);
  padding: 24px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bg);
  box-shadow: var(--fm-shadow-2);
}
.sheet:focus-visible {
  outline: none;
}
.cols {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 32px;
}
h2 {
  margin: 0 0 8px;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 500;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
dl {
  margin: 0;
}
.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 32px;
  border-bottom: 1px solid var(--fm-border);
}
dt {
  min-width: 0;
  white-space: nowrap;
  font-size: var(--fm-text-sm);
}
dd {
  display: flex;
  flex: none;
  gap: 4px;
  margin: 0;
}
kbd {
  min-width: 20px;
  padding: 1px 6px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-sm);
  color: var(--fm-muted);
  font: inherit;
  font-size: var(--fm-text-xs);
  font-variant-numeric: tabular-nums;
  text-align: center;
}
.hint {
  margin: 16px 0 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
</style>
