<script setup lang="ts">
// The command palette (Ctrl/Cmd+K): every registered action, filtered by a fuzzy
// query. A modal dialog with a combobox: arrows move, Enter runs, Escape closes and
// focus returns to where it was.
import { computed, nextTick, ref, watch } from 'vue';
import { highlight } from '../keyboard/fuzzy';
import { ACTIONS, ariaShortcut, formatChord } from '../keyboard/keymap';
import { useActions, useCommandsStore, type PaletteItem } from '../stores/commands';

const commands = useCommandsStore();
const input = ref<HTMLInputElement | null>(null);
const listbox = ref<HTMLElement | null>(null);
const active = ref(0);
let returnFocus: HTMLElement | null = null;

useActions([
  {
    id: ACTIONS.palette,
    title: 'Show all commands',
    group: 'General',
    run: commands.togglePalette,
  },
]);

const items = computed(() => commands.items);
const optionId = (i: number) => `fm-command-${i}`;

watch(
  () => commands.open,
  async (open) => {
    if (open) {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      active.value = 0;
      await nextTick();
      input.value?.focus();
    } else {
      const target = returnFocus;
      returnFocus = null;
      // The action may have moved focus on purpose (e.g. "Go to sidebar"); only put it
      // back when it was left on the closed dialog or nowhere.
      await nextTick();
      if (
        target?.isConnected &&
        (!document.activeElement || document.activeElement === document.body)
      )
        target.focus();
    }
  },
);

watch(
  () => commands.query,
  () => (active.value = 0),
);

watch(active, async () => {
  await nextTick();
  listbox.value
    ?.querySelector(`#${optionId(active.value)}`)
    ?.scrollIntoView?.({ block: 'nearest' });
});

function choose(item: PaletteItem | undefined): void {
  if (!item?.enabled) return;
  commands.closePalette();
  commands.run(item.action.id);
}

function move(by: number): void {
  const n = items.value.length;
  if (n) active.value = (active.value + by + n) % n;
}

function onKeyDown(e: KeyboardEvent): void {
  switch (e.key) {
    case 'ArrowDown':
      move(1);
      break;
    case 'ArrowUp':
      move(-1);
      break;
    case 'Home':
      active.value = 0;
      break;
    case 'End':
      active.value = Math.max(0, items.value.length - 1);
      break;
    case 'Enter':
      choose(items.value[active.value]);
      break;
    case 'Escape':
      commands.closePalette();
      break;
    case 'Tab':
      // The input is the dialog's only tab stop; focus stays inside the modal.
      break;
    default:
      return;
  }
  e.preventDefault();
  e.stopPropagation();
}
</script>

<template>
  <div v-if="commands.open" class="backdrop" @mousedown.self="commands.closePalette">
    <div class="palette" role="dialog" aria-modal="true" aria-label="Command palette">
      <input
        ref="input"
        v-model="commands.query"
        class="query"
        type="text"
        role="combobox"
        aria-label="Search commands"
        aria-autocomplete="list"
        aria-expanded="true"
        aria-controls="fm-command-list"
        :aria-activedescendant="items.length ? optionId(active) : undefined"
        placeholder="Type a command"
        autocomplete="off"
        spellcheck="false"
        @keydown="onKeyDown"
      />
      <!-- mousedown.prevent keeps focus in the input while an option is clicked. -->
      <ul
        id="fm-command-list"
        ref="listbox"
        class="list"
        role="listbox"
        aria-label="Commands"
        @mousedown.prevent
      >
        <li
          v-for="(item, i) in items"
          :id="optionId(i)"
          :key="item.action.id"
          class="item"
          :class="{ active: i === active }"
          role="option"
          :aria-selected="i === active"
          :aria-disabled="!item.enabled || undefined"
          :aria-keyshortcuts="item.chord ? ariaShortcut(item.chord) : undefined"
          @mousemove="active = i"
          @click="choose(item)"
        >
          <span class="group">{{ item.action.group }}</span>
          <span class="title">
            <template v-for="(part, k) in highlight(item.action.title, item.indices)" :key="k">
              <mark v-if="part.match">{{ part.text }}</mark>
              <template v-else>{{ part.text }}</template>
            </template>
          </span>
          <kbd v-if="item.chord" class="chord">{{ formatChord(item.chord) }}</kbd>
        </li>
        <li v-if="!items.length" class="none" role="presentation">No matching commands</li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 10;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 12vh;
  background: rgb(0 0 0 / 20%);
}
.palette {
  display: flex;
  flex-direction: column;
  width: min(560px, 90vw);
  max-height: 60vh;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: var(--fm-bg);
  box-shadow: 0 8px 32px rgb(0 0 0 / 24%);
  overflow: hidden;
}
.query {
  padding: 12px 16px;
  border: 0;
  border-bottom: 1px solid var(--fm-border);
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 15px;
}
.query:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.list {
  margin: 0;
  padding: 8px 0;
  overflow: auto;
  list-style: none;
}
.item {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 8px 16px;
  cursor: default;
}
.item.active {
  background: var(--fm-surface);
  box-shadow: inset 2px 0 0 var(--fm-accent);
}
.item[aria-disabled='true'] {
  color: var(--fm-muted);
}
.group {
  flex: none;
  width: 88px;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.title {
  flex: 1;
}
mark {
  background: transparent;
  color: var(--fm-accent);
  font-weight: 600;
}
.chord {
  padding: 0 4px;
  border: 1px solid var(--fm-border);
  border-radius: 4px;
  color: var(--fm-muted);
  font: inherit;
  font-size: 11.5px;
}
.none {
  padding: 8px 16px;
  color: var(--fm-muted);
}
@media (prefers-reduced-motion: no-preference) {
  .palette {
    animation: rise 120ms ease-out;
  }
}
@keyframes rise {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
}
</style>
