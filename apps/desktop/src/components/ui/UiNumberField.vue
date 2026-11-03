<script setup lang="ts">
// A number field drawn by us: a text input that accepts digits, with quiet − / +
// buttons. Empty is allowed (`''`). It replaces the browser's `type="number"`.
import { t } from '../../i18n';
import UiIcon from './UiIcon.vue';

const props = withDefaults(
  defineProps<{ modelValue: number | ''; id?: string; min?: number; step?: number }>(),
  { id: undefined, min: undefined, step: 1 },
);
const emit = defineEmits<{ 'update:modelValue': [value: number | ''] }>();

function parse(raw: string): number | '' {
  const trimmed = raw.trim();
  if (trimmed === '') return '';
  const n = Number(trimmed);
  return Number.isFinite(n) ? n : props.modelValue;
}
function bump(dir: 1 | -1) {
  const base =
    props.modelValue === '' ? (props.min ?? 0) - (dir === 1 ? props.step : 0) : props.modelValue;
  const next = base + dir * props.step;
  emit('update:modelValue', props.min !== undefined ? Math.max(props.min, next) : next);
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
      inputmode="numeric"
      autocomplete="off"
      :value="modelValue"
      @input="emit('update:modelValue', parse(($event.target as HTMLInputElement).value))"
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
