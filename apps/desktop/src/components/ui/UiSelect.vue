<script setup lang="ts" generic="T extends string | number">
// The Select: a button that shows the chosen option and opens a listbox under it in a
// `UiPopover`. The focus stays on the button (the "select-only combobox" of the ARIA
// Authoring Practices): Up and Down move the active option, Enter or Space takes it,
// Escape closes, a typed letter jumps to a matching option — and in a long list (more
// than eight options) typing filters the list instead. Closed, a typed letter changes
// the value like the native control did. Every <select> in the app is one of these.
//
// Attributes fall through to the button: `id` (for a `<label for>`), `aria-label`,
// `name`, `class`, `data-testid`. The button sizes to its longest option, so a change
// of value never moves the controls beside it.
import { computed, nextTick, ref, useAttrs, useId, watch } from 'vue';
import { t } from '../../i18n';
import type { SelectOption } from './types';
import UiPopover from './UiPopover.vue';

defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    modelValue: T | null | undefined;
    options: readonly SelectOption<T>[];
    disabled?: boolean;
    /** Shown while no option has the value. */
    placeholder?: string;
    /** A filter line in the menu; by default when the list is long. */
    searchable?: boolean;
    /** The button is as wide as its longest option (off for a picker that only adds). */
    fit?: boolean;
  }>(),
  { disabled: false, placeholder: '', searchable: undefined, fit: true },
);
const emit = defineEmits<{
  'update:modelValue': [value: T];
  /** The open menu's current option was chosen again: no change, but the user asked for it. */
  repick: [value: T];
}>();
const attrs = useAttrs();

const uid = useId();
const id = computed(() => (typeof attrs.id === 'string' ? attrs.id : uid));
const listId = computed(() => `${id.value}-list`);
/** The button's attributes, less the id the component places itself. */
const buttonAttrs = computed(() =>
  Object.fromEntries(Object.entries(attrs).filter(([k]) => k !== 'id')),
);

const trigger = ref<HTMLButtonElement | null>(null);
const list = ref<HTMLUListElement | null>(null);
const open = ref(false);
const active = ref(0);
const query = ref('');

const SEARCH_FROM = 9;
const canSearch = computed(() => props.searchable ?? props.options.length >= SEARCH_FROM);
const selected = computed(() => props.options.find((o) => o.value === props.modelValue));
const visible = computed(() => {
  const q = query.value.trim().toLowerCase();
  return q ? props.options.filter((o) => o.label.toLowerCase().includes(q)) : [...props.options];
});
const activeId = computed(() =>
  open.value && visible.value[active.value] ? `${listId.value}-${active.value}` : undefined,
);

function scrollActiveIntoView(): void {
  const row = list.value?.children[active.value];
  if (row instanceof HTMLElement) row.scrollIntoView?.({ block: 'nearest' });
}

function show(): void {
  if (props.disabled || open.value) return;
  query.value = '';
  const at = props.options.findIndex((o) => o.value === props.modelValue);
  active.value =
    at >= 0
      ? at
      : Math.max(
          0,
          props.options.findIndex((o) => !o.disabled),
        );
  open.value = true;
  void nextTick(scrollActiveIntoView);
}
function hide(): void {
  open.value = false;
  query.value = '';
}
function toggle(): void {
  if (open.value) hide();
  else show();
}

function choose(option: SelectOption<T> | undefined): void {
  if (!option || option.disabled) return;
  if (option.value !== props.modelValue) emit('update:modelValue', option.value);
  else if (open.value) emit('repick', option.value);
  hide();
}

/** Moves the active option by `step`, skipping disabled ones, staying inside the list. */
function move(step: number): void {
  const rows = visible.value;
  let i = active.value;
  for (let n = 0; n < rows.length; n++) {
    i = Math.min(rows.length - 1, Math.max(0, i + step));
    if (!rows[i]?.disabled || i === 0 || i === rows.length - 1) break;
  }
  active.value = i;
  scrollActiveIntoView();
}

// Type-ahead: the letters typed in quick succession; the same letter again cycles
// through the options that start with it, as the native control does.
let typed = '';
let typedAt = 0;
function typeahead(key: string, from: number, rows: readonly SelectOption<T>[]): number {
  const now = Date.now();
  typed = now - typedAt < 600 ? typed + key : key;
  typedAt = now;
  const prefix = typed.toLowerCase();
  const cycling = prefix.length > 1 && [...prefix].every((c) => c === prefix[0]);
  const needle = cycling ? prefix[0]! : prefix;
  const start = cycling || prefix.length === 1 ? from + 1 : from;
  for (let n = 0; n < rows.length; n++) {
    const i = (start + n) % rows.length;
    const row = rows[i]!;
    if (!row.disabled && row.label.toLowerCase().startsWith(needle)) return i;
  }
  return -1;
}

const isPrintable = (e: KeyboardEvent) =>
  e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey;

function onKey(e: KeyboardEvent): void {
  if (props.disabled) return;
  const k = e.key;
  if (!open.value) {
    if (k === 'ArrowDown' || k === 'ArrowUp' || k === 'Enter' || k === ' ') {
      e.preventDefault();
      e.stopPropagation();
      show();
    } else if (isPrintable(e)) {
      e.stopPropagation();
      const at = props.options.findIndex((o) => o.value === props.modelValue);
      const hit = typeahead(k, at, props.options);
      if (hit >= 0) choose(props.options[hit]);
    }
    return;
  }
  switch (k) {
    case 'ArrowDown':
      move(1);
      break;
    case 'ArrowUp':
      move(-1);
      break;
    case 'Home':
      active.value = 0;
      scrollActiveIntoView();
      break;
    case 'End':
      active.value = Math.max(0, visible.value.length - 1);
      scrollActiveIntoView();
      break;
    case 'PageDown':
      move(10);
      break;
    case 'PageUp':
      move(-10);
      break;
    case 'Enter':
      choose(visible.value[active.value]);
      break;
    case ' ':
      if (canSearch.value && query.value) query.value += ' ';
      else choose(visible.value[active.value]);
      break;
    case 'Escape':
      hide();
      break;
    case 'Tab':
      // The focus moves on; the key is not consumed.
      hide();
      return;
    case 'Backspace':
      if (!canSearch.value) return;
      query.value = query.value.slice(0, -1);
      active.value = 0;
      break;
    default:
      if (!isPrintable(e)) return;
      if (canSearch.value) {
        query.value += k;
        active.value = 0;
      } else {
        const hit = typeahead(k, active.value, visible.value);
        if (hit >= 0) active.value = hit;
      }
      scrollActiveIntoView();
  }
  e.preventDefault();
  e.stopPropagation();
}

watch(
  () => props.disabled,
  (disabled) => {
    if (disabled) hide();
  },
);
watch(visible, (rows) => {
  if (active.value >= rows.length) active.value = Math.max(0, rows.length - 1);
});

defineExpose({ focus: () => trigger.value?.focus() });
</script>

<template>
  <button
    v-bind="buttonAttrs"
    :id="id"
    ref="trigger"
    type="button"
    class="ui-select"
    role="combobox"
    aria-haspopup="listbox"
    :aria-expanded="open"
    :aria-controls="open ? listId : undefined"
    :aria-activedescendant="activeId"
    :disabled="disabled"
    :data-value="modelValue == null ? undefined : String(modelValue)"
    @click="toggle"
    @keydown="onKey"
  >
    <span class="body">
      <template v-if="fit">
        <span v-for="o in options" :key="o.value" class="sizer" aria-hidden="true">
          {{ o.label }}
        </span>
      </template>
      <span class="value" :class="{ placeholder: !selected }">
        {{ selected?.label ?? placeholder }}
      </span>
    </span>
    <svg class="chevron" viewBox="0 0 16 16" aria-hidden="true">
      <path d="M4 6l4 4 4-4" />
    </svg>
  </button>
  <UiPopover :open="open" :anchor="trigger" match-width @close="hide">
    <!-- pointerdown.prevent keeps the focus on the button while an option is clicked. -->
    <div class="menu" @pointerdown.prevent>
      <p v-if="canSearch" class="filter" :data-typing="query !== ''">
        <svg class="icon" viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="7" cy="7" r="4.5" />
          <path d="M10.5 10.5L14 14" />
        </svg>
        <span>{{ query || t('ui.select.search') }}</span>
      </p>
      <ul
        :id="listId"
        ref="list"
        class="list"
        role="listbox"
        :aria-labelledby="id"
        :aria-activedescendant="activeId"
      >
        <li
          v-for="(o, i) in visible"
          :id="`${listId}-${i}`"
          :key="o.value"
          class="option"
          role="option"
          :class="{ active: i === active }"
          :aria-selected="o.value === modelValue"
          :aria-disabled="o.disabled || undefined"
          :data-value="String(o.value)"
          :title="o.title"
          @pointermove="active = i"
          @click="choose(o)"
        >
          <svg class="check" viewBox="0 0 16 16" aria-hidden="true">
            <path d="M3 8.5l3 3 7-7" />
          </svg>
          <span class="label">{{ o.label }}</span>
        </li>
        <li v-if="!visible.length" class="empty" role="presentation">
          {{ t('ui.select.noMatches') }}
        </li>
      </ul>
    </div>
  </UiPopover>
</template>

<style scoped>
.ui-select {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-1);
  box-sizing: border-box;
  height: var(--fm-control-height);
  max-width: 100%;
  min-width: 0;
  padding: 0;
  padding-inline: var(--fm-space-2) var(--fm-space-1);
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
  text-align: start;
  cursor: default;
}
.ui-select[aria-expanded='true'] {
  border-color: var(--fm-accent);
}
.ui-select:disabled {
  opacity: 0.55;
}
/* Every label in one grid cell: the button is as wide as the longest, the chosen one shows. */
.body {
  display: grid;
  flex: 1 1 auto;
  min-width: 0;
}
.body > span {
  grid-area: 1 / 1;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.sizer {
  visibility: hidden;
}
.placeholder {
  color: var(--fm-muted);
}
.chevron,
.icon,
.check {
  flex: none;
  width: var(--fm-icon-size);
  height: var(--fm-icon-size);
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.chevron {
  color: var(--fm-muted);
  transition: transform var(--fm-motion-fast) var(--fm-ease);
}
.ui-select[aria-expanded='true'] .chevron {
  transform: rotate(180deg);
}

.menu {
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.filter {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  margin: 0 0 var(--fm-space-1);
  padding: var(--fm-space-1) var(--fm-space-2);
  border-bottom: 1px solid var(--fm-border);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.filter[data-typing='true'] {
  color: var(--fm-fg);
}
.filter span {
  overflow: hidden;
  white-space: pre;
  text-overflow: ellipsis;
}
.list {
  flex: 1 1 auto;
  min-height: 0;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: thin;
  list-style: none;
}
.option {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  min-height: var(--fm-control-height);
  padding: 0 var(--fm-space-2);
  border-radius: var(--fm-radius-sm);
  white-space: nowrap;
}
.option.active {
  background-color: var(--fm-hover);
}
.option[aria-selected='true'] {
  font-weight: 500;
}
.option[aria-disabled='true'] {
  opacity: 0.55;
}
.check {
  visibility: hidden;
}
.option[aria-selected='true'] .check {
  visibility: visible;
}
.label {
  overflow: hidden;
  text-overflow: ellipsis;
}
.empty {
  padding: var(--fm-space-1) var(--fm-space-2);
  color: var(--fm-muted);
}
</style>
