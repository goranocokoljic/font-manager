<script setup lang="ts">
// A number field drawn by us: a text input that accepts numbers (decimals too), with
// quiet − / + buttons. Empty is allowed (`''`). It replaces the browser's `type="number"`.
// `commit` fires when the user leaves the field or presses Enter, with the final value.
import { nextTick, ref, watch } from 'vue';
import { t } from '../../i18n';
import UiIcon from './UiIcon.vue';

const props = withDefaults(
  defineProps<{
    modelValue: number | '';
    id?: string;
    min?: number;
    max?: number;
    step?: number;
    ariaLabel?: string;
    ariaDescribedby?: string;
  }>(),
  {
    id: undefined,
    min: undefined,
    max: undefined,
    step: 1,
    ariaLabel: undefined,
    ariaDescribedby: undefined,
  },
);
const emit = defineEmits<{
  'update:modelValue': [value: number | ''];
  commit: [value: number | ''];
}>();

/** What the input shows: kept apart so "0." and "-" survive on the way to a number. */
const text = ref(String(props.modelValue));
watch(
  () => props.modelValue,
  (value) => {
    const shown = text.value.trim() === '' ? '' : Number(text.value);
    if (shown !== value) text.value = String(value);
  },
);

function parse(raw: string): number | '' | null {
  const trimmed = raw.trim();
  if (trimmed === '') return '';
  const n = Number(trimmed);
  return Number.isFinite(n) ? n : null;
}
/** Digits after the point in `step`, so 0.05 steps do not drift to 0.15000000000000002. */
function places(): number {
  return (String(props.step).split('.')[1] ?? '').length;
}
function bump(dir: 1 | -1) {
  const base =
    props.modelValue === '' ? (props.min ?? 0) - (dir === 1 ? props.step : 0) : props.modelValue;
  let next = Number((base + dir * props.step).toFixed(places()));
  if (props.min !== undefined) next = Math.max(props.min, next);
  if (props.max !== undefined) next = Math.min(props.max, next);
  text.value = String(next);
  emit('update:modelValue', next);
  emit('commit', next);
}
function onInput(e: Event) {
  const raw = (e.target as HTMLInputElement).value;
  text.value = raw;
  const n = parse(raw);
  if (n !== null) emit('update:modelValue', n);
}
async function onChange(e: Event) {
  const n = parse((e.target as HTMLInputElement).value);
  emit('commit', n ?? props.modelValue);
  await nextTick();
  text.value = String(props.modelValue);
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
    e.preventDefault();
    bump(e.key === 'ArrowUp' ? 1 : -1);
  }
}
</script>

<template>
  <span class="num">
    <input
      :id="id"
      class="in"
      type="text"
      inputmode="decimal"
      autocomplete="off"
      :aria-label="ariaLabel"
      :aria-describedby="ariaDescribedby"
      :value="text"
      @input="onInput"
      @change="onChange"
      @keydown="onKey"
    />
    <button type="button" class="b" tabindex="-1" :aria-label="t('ui.decrease')" @click="bump(-1)">
      <UiIcon name="minus" />
    </button>
    <button type="button" class="b" tabindex="-1" :aria-label="t('ui.increase')" @click="bump(1)">
      <UiIcon name="plus" />
    </button>
  </span>
</template>

<style scoped>
.num {
  display: flex;
  align-items: center;
  gap: 2px;
  width: 100%;
  padding-inline-end: 2px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
}
.num:focus-within {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.in {
  flex: 1;
  min-width: 0;
  padding: 2px 6px;
  border: 0;
  outline: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  font-variant-numeric: tabular-nums;
}
.b {
  display: grid;
  place-items: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: 0;
  border-radius: var(--fm-radius-sm);
  background: transparent;
  color: var(--fm-muted);
  transition: background-color var(--fm-motion-fast) var(--fm-ease);
}
.b:hover {
  background-color: var(--fm-hover);
  color: var(--fm-fg);
}
</style>
