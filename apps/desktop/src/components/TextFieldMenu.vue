<script setup lang="ts">
// E54.2: a right-click (or the Menu key, Shift+F10, Ctrl+click on macOS) in any text field
// opens the app's menu with exactly Cut, Copy, Paste and Select all, in place of the
// webview's (E54.1 blocks that one everywhere). One instance in App.vue serves every field:
// the search, the preview text, a rename, the notes, the settings, the catalog search.
//
// The menu is a `UiPopover` with the look of the app's other menus, at the pointer. Unlike
// them it never takes the focus: the field keeps the focus, its selection and its caret, and
// the arrows, Enter and Esc reach the menu through a window listener while it is open. Moving
// the focus would end what the field was doing — a block's editor and the notes save and
// close on blur, a font picker closes — and Copy would no longer have a selection to copy.
// The rules (which items are enabled, what each does) are in `textFieldMenu.ts`.
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useClient } from '../api/client';
import { copyText } from '../api/clipboard';
import { t, type MessageKey } from '../i18n';
import { ariaShortcut, formatChord } from '../keyboard/keymap';
import {
  TEXT_COMMANDS,
  TEXT_COMMAND_KEYS,
  enabledCommands,
  fromPointer,
  menuPoint,
  runTextCommand,
  textFieldOf,
  type TextCommand,
  type TextField,
} from '../textFieldMenu';
import UiPopover from './ui/UiPopover.vue';

const LABELS: Record<TextCommand, MessageKey> = {
  cut: 'textMenu.cut',
  copy: 'textMenu.copy',
  paste: 'textMenu.paste',
  selectAll: 'textMenu.selectAll',
};
const itemId = (c: TextCommand) => `fm-text-menu-${c}`;

const field = ref<TextField | null>(null);
const at = ref<{ x: number; y: number } | null>(null);
const enabled = ref<Record<TextCommand, boolean>>({
  cut: false,
  copy: false,
  paste: false,
  selectAll: false,
});
/** The highlighted item (the arrows move it), or `null`. */
const active = ref<TextCommand | null>(null);
const open = computed(() => field.value !== null);
const items = computed(() =>
  TEXT_COMMANDS.map((c) => ({
    command: c,
    label: t(LABELS[c]),
    chord: { key: TEXT_COMMAND_KEYS[c], mod: true },
  })),
);

/** The field's own `aria-activedescendant`, put back when the menu closes. */
let savedDescendant: string | null = null;

function onContextMenu(e: MouseEvent): void {
  const target = textFieldOf(e.target);
  if (!target) {
    close();
    return;
  }
  if (field.value && field.value !== target) close();
  enabled.value = enabledCommands(target);
  at.value = menuPoint(e, target);
  if (field.value !== target) {
    savedDescendant = target.getAttribute('aria-activedescendant');
    target.addEventListener('blur', onFieldBlur);
  }
  field.value = target;
  // From the keyboard the first item is highlighted at once, as the other menus focus theirs.
  active.value = fromPointer(e) ? null : (TEXT_COMMANDS.find((c) => enabled.value[c]) ?? null);
}

/** Closes the menu; the focus and the caret never left the field. */
function close(): void {
  const f = field.value;
  if (!f) return;
  f.removeEventListener('blur', onFieldBlur);
  if (savedDescendant === null) f.removeAttribute('aria-activedescendant');
  else f.setAttribute('aria-activedescendant', savedDescendant);
  savedDescendant = null;
  field.value = null;
  active.value = null;
  at.value = null;
}
function onFieldBlur(): void {
  close();
}

async function run(command: TextCommand): Promise<void> {
  const f = field.value;
  if (!f || !enabled.value[command]) return;
  close();
  await runTextCommand(command, f, {
    exec: (name, value) => document.execCommand(name, false, value),
    readClipboard: () => useClient().readClipboardText(),
    writeClipboard: copyText,
  }).catch((e: unknown) => console.warn(`the text menu's ${command} failed`, e));
}

/** The next enabled item from `from` in direction `step`, wrapping. */
function move(step: 1 | -1, from: TextCommand | null): TextCommand | null {
  const list = TEXT_COMMANDS.filter((c) => enabled.value[c]);
  if (!list.length) return null;
  if (from === null) return step === 1 ? list[0]! : list[list.length - 1]!;
  const i = list.indexOf(from);
  return list[(i + step + list.length) % list.length]!;
}

/** A modifier pressed alone (the start of a chord) leaves the menu open. */
const MODIFIERS = new Set(['Shift', 'Control', 'Alt', 'Meta', 'AltGraph']);

/** The menu's keys, before the field and the app's shortcuts see them. */
function onKey(e: KeyboardEvent): void {
  if (!open.value || MODIFIERS.has(e.key)) return;
  let next: TextCommand | null | undefined;
  if (e.key === 'ArrowDown') next = move(1, active.value);
  else if (e.key === 'ArrowUp') next = move(-1, active.value);
  else if (e.key === 'Home') next = move(1, null);
  else if (e.key === 'End') next = move(-1, null);
  else if (e.key === 'Escape') {
    e.preventDefault();
    e.stopImmediatePropagation();
    close();
    return;
  } else if (e.key === 'Enter') {
    e.preventDefault();
    e.stopImmediatePropagation();
    if (active.value) void run(active.value);
    else close();
    return;
  } else {
    // Any other key (Tab, typing, a shortcut such as Ctrl+C) closes the menu and does its job.
    close();
    return;
  }
  e.preventDefault();
  e.stopImmediatePropagation();
  active.value = next;
}

// The field announces the highlighted item while it keeps the focus.
watch(active, (c) => {
  const f = field.value;
  if (!f) return;
  if (c) f.setAttribute('aria-activedescendant', itemId(c));
  else if (savedDescendant === null) f.removeAttribute('aria-activedescendant');
  else f.setAttribute('aria-activedescendant', savedDescendant);
});

watch(open, (isOpen) => {
  if (isOpen) window.addEventListener('keydown', onKey, true);
  else window.removeEventListener('keydown', onKey, true);
});

onMounted(() => {
  window.addEventListener('contextmenu', onContextMenu);
  window.addEventListener('blur', close);
});
onBeforeUnmount(() => {
  close();
  window.removeEventListener('contextmenu', onContextMenu);
  window.removeEventListener('blur', close);
  window.removeEventListener('keydown', onKey, true);
});
</script>

<template>
  <UiPopover :open="open" :anchor="null" :at="at" @close="close">
    <div
      v-if="open"
      class="menu"
      role="menu"
      data-testid="text-field-menu"
      :aria-label="t('textMenu.label')"
    >
      <button
        v-for="item in items"
        :id="itemId(item.command)"
        :key="item.command"
        type="button"
        role="menuitem"
        tabindex="-1"
        :class="{ active: active === item.command }"
        :disabled="!enabled[item.command]"
        :aria-keyshortcuts="ariaShortcut(item.chord)"
        @mousedown.prevent
        @pointerenter="active = enabled[item.command] ? item.command : active"
        @click="run(item.command)"
      >
        <span class="label">{{ item.label }}</span>
        <span class="chord" aria-hidden="true">{{ formatChord(item.chord) }}</span>
      </button>
    </div>
  </UiPopover>
</template>

<style scoped>
.menu {
  display: flex;
  flex-direction: column;
  min-width: 180px;
}
.menu button {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-4);
  width: 100%;
  height: 28px;
  padding: 0 var(--fm-space-2);
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  text-align: start;
  white-space: nowrap;
}
.menu button:hover:not(:disabled),
.menu button.active {
  background-color: var(--fm-hover);
}
/* The field keeps the focus, so the highlighted item draws the 2 px accent ring itself,
   inside, so the layer does not clip it. */
.menu button.active {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.menu button:disabled {
  color: var(--fm-muted);
}
.chord {
  color: var(--fm-muted);
}
</style>
