<script setup lang="ts">
// The multi-tagging panel (E13.4, T or the palette): every tag, with whether it is on
// all, some or none of the selected fonts. Typing fuzzy-filters the names, arrows
// move, Space stages putting the tag on every font or taking it off every font, Enter
// applies the staged toggles in one go, or makes a new tag when no tag has the typed
// name. Escape closes without applying; focus returns to where it was.
import { computed, nextTick, ref, watch } from 'vue';
import { errorMessage, t } from '../i18n';
import { highlight } from '../keyboard/fuzzy';
import { useTaggingStore, type TagRow } from '../stores/tagging';

const tagging = useTaggingStore();
const input = ref<HTMLInputElement | null>(null);
const listbox = ref<HTMLElement | null>(null);
const active = ref(0);
let returnFocus: HTMLElement | null = null;

const rows = computed(() => tagging.rows);
const optionId = (i: number) => `fm-tag-option-${i}`;
const fonts = (n: number) => t('tagging.fonts', { count: n });

watch(
  () => tagging.open,
  async (open) => {
    if (open) {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      active.value = 0;
      await nextTick();
      input.value?.focus();
    } else {
      const target = returnFocus;
      returnFocus = null;
      await nextTick();
      if (
        target?.isConnected &&
        (!document.activeElement || document.activeElement === document.body)
      )
        target.focus();
    }
  },
  { immediate: true },
);

watch(
  () => tagging.query,
  () => (active.value = 0),
);

watch(active, async () => {
  await nextTick();
  listbox.value
    ?.querySelector(`#${optionId(active.value)}`)
    ?.scrollIntoView?.({ block: 'nearest' });
});

function move(by: number): void {
  const n = rows.value.length;
  if (n) active.value = (active.value + by + n) % n;
}

function toggleAt(i: number): void {
  const row = rows.value[i];
  if (row) tagging.toggle(row.tag.id);
}

async function enter(): Promise<void> {
  if (tagging.newName) await tagging.create();
  else await tagging.apply();
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
      active.value = Math.max(0, rows.value.length - 1);
      break;
    case ' ':
      // Space toggles the active tag; with no tag left to toggle it types a space, so a
      // new name can have several words.
      if (!rows.value.length) return;
      toggleAt(active.value);
      break;
    case 'Enter':
      void enter();
      break;
    case 'Escape':
      tagging.close();
      break;
    case 'Tab':
      // The input is the dialog's tab stop for the keyboard; the buttons are for the pointer.
      break;
    default:
      return;
  }
  e.preventDefault();
  e.stopPropagation();
}

/** "on some (2 of 3), changed": where the tag is, and whether Space changed it. */
function stateText(row: TagRow): string {
  let text = t(`tagging.state.${row.state}`);
  if (row.state === 'some')
    text = t('tagging.someOf', { state: text, carried: row.carried, total: tagging.faces.length });
  return row.staged ? t('tagging.changed', { state: text }) : text;
}

const summary = computed(() => {
  const n = tagging.faces.length;
  return t('tagging.title', { fonts: fonts(n) });
});
</script>

<template>
  <div v-if="tagging.open" class="backdrop" @mousedown.self="tagging.close">
    <div class="panel" role="dialog" aria-modal="true" aria-labelledby="fm-tagging-title">
      <header class="head">
        <h2 id="fm-tagging-title" class="title">{{ summary }}</h2>
        <p class="hint">{{ t('tagging.hint') }}</p>
      </header>
      <input
        ref="input"
        v-model="tagging.query"
        class="query"
        type="text"
        role="combobox"
        :aria-label="t('tagging.queryLabel')"
        aria-autocomplete="list"
        aria-expanded="true"
        aria-controls="fm-tag-options"
        :aria-activedescendant="rows.length ? optionId(active) : undefined"
        :placeholder="t('tagging.queryPlaceholder')"
        autocomplete="off"
        spellcheck="false"
        @keydown="onKeyDown"
      />
      <!-- mousedown.prevent keeps focus in the input while an option is clicked. -->
      <ul
        id="fm-tag-options"
        ref="listbox"
        class="list"
        role="listbox"
        :aria-label="t('tagging.tags')"
        aria-multiselectable="true"
        @mousedown.prevent
      >
        <li
          v-for="(row, i) in rows"
          :id="optionId(i)"
          :key="row.tag.id"
          class="item"
          :class="{ active: i === active, staged: row.staged }"
          role="option"
          :aria-selected="row.state === 'all'"
          :data-state="row.state"
          :data-tag="row.tag.name"
          @mousemove="active = i"
          @click="
            active = i;
            toggleAt(i);
          "
        >
          <span class="box" :class="row.state" aria-hidden="true">{{
            row.state === 'all' ? '✓' : row.state === 'some' ? '–' : ''
          }}</span>
          <span class="name">
            <template v-for="(part, k) in highlight(row.tag.name, row.indices)" :key="k">
              <mark v-if="part.match">{{ part.text }}</mark>
              <template v-else>{{ part.text }}</template>
            </template>
          </span>
          <span class="state">{{ stateText(row) }}</span>
        </li>
        <li v-if="tagging.newName" class="none" role="presentation">
          {{ t('tagging.makeNew', { name: tagging.newName, fonts: fonts(tagging.faces.length) }) }}
        </li>
        <li v-else-if="!rows.length" class="none" role="presentation">
          {{ t('tagging.noTags') }}
        </li>
      </ul>
      <p v-if="tagging.error" role="alert" class="error">{{ errorMessage(tagging.error) }}</p>
      <footer class="foot">
        <button type="button" class="button" @click="tagging.close">
          {{ t('common.cancel') }}
        </button>
        <button
          type="button"
          class="button primary"
          :disabled="tagging.busy"
          @click="tagging.apply"
        >
          {{
            tagging.pending
              ? t('tagging.applyCount', { count: tagging.pending })
              : t('tagging.apply')
          }}
        </button>
      </footer>
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
.panel {
  display: flex;
  flex-direction: column;
  width: min(480px, 90vw);
  max-height: 64vh;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: var(--fm-bg);
  box-shadow: 0 8px 32px rgb(0 0 0 / 24%);
  overflow: hidden;
}
.head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  padding: 12px 16px 4px;
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.hint {
  margin: 0;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.query {
  margin: 8px 16px;
  padding: 6px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 14px;
}
.query:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -1px;
}
.list {
  margin: 0;
  padding: 4px 0;
  overflow: auto;
  list-style: none;
}
.item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 16px;
  cursor: default;
}
.item.active {
  background: var(--fm-surface);
  box-shadow: inset 2px 0 0 var(--fm-accent);
}
.box {
  display: inline-grid;
  flex: none;
  place-items: center;
  width: 14px;
  height: 14px;
  border: 1px solid var(--fm-control-border);
  border-radius: 3px;
  font-size: 11px;
  line-height: 1;
}
.box.all,
.box.some {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.staged .name {
  font-weight: 600;
}
.state {
  color: var(--fm-muted);
  font-size: 11.5px;
}
mark {
  background: transparent;
  color: var(--fm-accent);
  font-weight: 600;
}
.none {
  padding: 6px 16px;
  color: var(--fm-muted);
}
.error {
  margin: 4px 16px;
  color: var(--fm-danger);
}
.foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 8px 16px 12px;
  border-top: 1px solid var(--fm-border);
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.button.primary {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
@media (prefers-reduced-motion: no-preference) {
  .panel {
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
