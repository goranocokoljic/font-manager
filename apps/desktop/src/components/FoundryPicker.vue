<script setup lang="ts">
// A searchable list of foundries under an anchor (E50.12): the sidebar's "All N foundries…"
// and its "Merge into…" both open one. The field filters the names as you type (the full
// names the API gave, without case); Enter or a click chooses one. ArrowDown and ArrowUp
// walk the list. Escape, Tab and a press outside close it; the owner puts the focus back.
import { computed, nextTick, ref, useId, watch } from 'vue';
import type { FoundryInfo } from '../api/types';
import { formatNumber, t } from '../i18n';
import UiPopover from './ui/UiPopover.vue';

const props = defineProps<{
  open: boolean;
  anchor: HTMLElement | null;
  /** What the list is for: the field's accessible name. */
  label: string;
  foundries: readonly FoundryInfo[];
  /** The foundry chosen now, marked pressed; null for none. */
  chosen?: string | null;
}>();
const emit = defineEmits<{ choose: [name: string]; close: [] }>();

const uid = useId();
const field = ref<HTMLInputElement | null>(null);
const list = ref<HTMLElement | null>(null);
const text = ref('');

watch(
  () => props.open,
  (open) => {
    if (open) text.value = '';
  },
);

const shown = computed(() => {
  const q = text.value.trim().toLowerCase();
  return q ? props.foundries.filter((f) => f.name.toLowerCase().includes(q)) : props.foundries;
});

const isChosen = (f: FoundryInfo) =>
  props.chosen != null && props.chosen.toLowerCase() === f.name.toLowerCase();

async function focusField(): Promise<void> {
  await nextTick();
  field.value?.focus();
}

function options(): HTMLButtonElement[] {
  return [...(list.value?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
}

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' || e.key === 'Tab') {
    // The layer ends <body>: Tab from it would leave the page, so it closes instead.
    e.preventDefault();
    e.stopPropagation();
    emit('close');
    return;
  }
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    e.stopPropagation();
    const all = options();
    const at = all.indexOf(document.activeElement as HTMLButtonElement);
    const next = e.key === 'ArrowDown' ? at + 1 : at - 1;
    if (next < 0) field.value?.focus();
    else all[Math.min(next, all.length - 1)]?.focus();
    return;
  }
  // Plain keys stay here: the app's one-key shortcuts must not act on the list behind it.
  if (!e.ctrlKey && !e.metaKey && !e.altKey) e.stopPropagation();
}

function onEnter(): void {
  const first = shown.value[0];
  if (first) emit('choose', first.name);
}
</script>

<template>
  <UiPopover :open="open" :anchor="anchor" @close="emit('close')" @placed="focusField">
    <div class="picker" data-testid="foundry-picker" @keydown="onKey">
      <input
        ref="field"
        v-model="text"
        class="field"
        type="search"
        :aria-label="label"
        :placeholder="t('foundries.searchPlaceholder')"
        :aria-controls="`${uid}-list`"
        @keydown.enter.prevent="onEnter"
      />
      <ul :id="`${uid}-list`" ref="list" class="list" :aria-label="label">
        <li v-for="f in shown" :key="f.name">
          <button
            type="button"
            class="option"
            :aria-pressed="chosen === undefined ? undefined : isChosen(f)"
            @click="emit('choose', f.name)"
          >
            <span class="name">{{ f.name }}</span>
            <span class="n" aria-hidden="true">{{ formatNumber(f.families) }}</span>
            <span class="sr-only">{{ t('home.families', { count: f.families }) }}</span>
          </button>
        </li>
        <li v-if="!shown.length" class="quiet">{{ t('foundries.noMatch') }}</li>
      </ul>
    </div>
  </UiPopover>
</template>

<style scoped>
.picker {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-1);
  width: 240px;
  min-height: 0;
}
.field {
  box-sizing: border-box;
  width: 100%;
  padding: 4px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-surface);
  color: inherit;
  font: inherit;
  font-size: var(--fm-text-sm);
}
.list {
  overflow-y: auto;
  max-height: 240px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.option {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  width: 100%;
  padding: 4px 8px;
  border: 0;
  border-radius: var(--fm-radius-sm);
  background: none;
  color: inherit;
  font: inherit;
  font-size: var(--fm-text-sm);
  text-align: start;
}
.option:hover,
.option:focus-visible {
  background-color: var(--fm-hover);
}
.option[aria-pressed='true'] {
  box-shadow: inset 2px 0 0 var(--fm-accent);
  font-weight: 600;
}
.name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.n {
  color: var(--fm-muted);
  font-variant-numeric: tabular-nums;
}
.quiet {
  padding: 4px 8px;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
</style>
