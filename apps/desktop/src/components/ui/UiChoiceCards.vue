<script setup lang="ts" generic="T extends string">
// Choice cards (E57.3, board 15): one of a few choices drawn as cards. A radio group with
// one tab stop: the chosen card (or the first card that is on) takes Tab; the arrow keys move
// the choice and choose, as native radios do; Space and Enter choose the focused card. Each
// card has a title, at most one line and, where it helps, a small figure (the `figure` slot,
// given the option). The chosen card carries the accent ring and the filled dot. Two columns
// for two choices, three for three short ones, one when the lines are long (DESIGN.md,
// "Settings"). A local copy of a choice card is a bug.
import { computed, nextTick, ref, useId } from 'vue';

export interface ChoiceCard<V extends string> {
  value: V;
  title: string;
  /** The card's one line under its title. */
  line?: string;
  /** Off: dimmed, `aria-disabled`, skipped by the arrow keys and not chosen by a click. */
  disabled?: boolean;
}

const props = withDefaults(
  defineProps<{
    modelValue: T | null;
    options: readonly ChoiceCard<T>[];
    /** The group's name; also the prefix of the cards' ids. */
    name: string;
    /** The group's accessible name. */
    label: string;
    columns?: 1 | 2 | 3;
  }>(),
  { columns: 2 },
);
const emit = defineEmits<{ 'update:modelValue': [value: T] }>();

defineSlots<{ figure?: (props: { option: ChoiceCard<T>; checked: boolean }) => unknown }>();

const uid = useId();
const root = ref<HTMLElement | null>(null);
const cardId = (o: ChoiceCard<T>) => `${uid}-${props.name}-${o.value}`;

/** The card on the Tab path: the chosen one, else the first that is on. */
const tabStop = computed(
  () =>
    props.options.find((o) => o.value === props.modelValue && !o.disabled)?.value ??
    props.options.find((o) => !o.disabled)?.value,
);

function choose(o: ChoiceCard<T>): void {
  if (o.disabled || o.value === props.modelValue) return;
  emit('update:modelValue', o.value);
}

async function focusCard(o: ChoiceCard<T>): Promise<void> {
  await nextTick();
  root.value?.querySelector<HTMLElement>(`[data-value="${CSS.escape(o.value)}"]`)?.focus();
}

function onKey(e: KeyboardEvent, from: ChoiceCard<T>): void {
  if (e.key === ' ' || e.key === 'Enter') {
    e.preventDefault();
    choose(from);
    return;
  }
  const rtl = getComputedStyle(e.currentTarget as Element).direction === 'rtl';
  const next = ['ArrowDown', rtl ? 'ArrowLeft' : 'ArrowRight'];
  const prev = ['ArrowUp', rtl ? 'ArrowRight' : 'ArrowLeft'];
  const step = next.includes(e.key) ? 1 : prev.includes(e.key) ? -1 : 0;
  if (!step) return;
  e.preventDefault();
  const on = props.options.filter((o) => !o.disabled);
  if (!on.length) return;
  const i = on.findIndex((o) => o.value === from.value);
  const to = on[(i + step + on.length) % on.length];
  choose(to);
  void focusCard(to);
}
</script>

<template>
  <div
    ref="root"
    role="radiogroup"
    class="ui-choice-cards"
    :class="`cols-${columns}`"
    :aria-label="label"
    :data-name="name"
  >
    <div
      v-for="o in options"
      :id="cardId(o)"
      :key="o.value"
      role="radio"
      class="card"
      :data-value="o.value"
      :aria-checked="o.value === modelValue"
      :aria-disabled="o.disabled ? 'true' : undefined"
      :aria-labelledby="`${cardId(o)}-title`"
      :aria-describedby="o.line ? `${cardId(o)}-line` : undefined"
      :tabindex="o.value === tabStop ? 0 : -1"
      @click="choose(o)"
      @keydown="onKey($event, o)"
    >
      <span class="dot" aria-hidden="true" />
      <span class="text">
        <span :id="`${cardId(o)}-title`" class="title">{{ o.title }}</span>
        <span v-if="o.line" :id="`${cardId(o)}-line`" class="line">{{ o.line }}</span>
      </span>
      <span v-if="$slots.figure" class="figure" aria-hidden="true">
        <slot name="figure" :option="o" :checked="o.value === modelValue" />
      </span>
    </div>
  </div>
</template>

<style scoped>
.ui-choice-cards {
  display: grid;
  gap: var(--fm-space-2);
}
.cols-1 {
  grid-template-columns: minmax(0, 1fr);
}
.cols-2 {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
.cols-3 {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}
.card {
  display: flex;
  align-items: flex-start;
  gap: var(--fm-space-3);
  padding: var(--fm-space-3) var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bg);
  cursor: default;
}
.card:hover:not([aria-disabled='true']) {
  border-color: var(--fm-border-strong);
}
/* The accent ring: a 2 px border drawn as a 1 px border plus a 1 px inset shadow, so the
   chosen card does not grow. */
.card[aria-checked='true'] {
  border-color: var(--fm-accent);
  box-shadow: inset 0 0 0 1px var(--fm-accent);
}
.card[aria-disabled='true'] {
  opacity: 0.5;
}
.dot {
  box-sizing: border-box;
  flex: none;
  width: 16px;
  height: 16px;
  margin-top: 2px;
  border: 1px solid var(--fm-control-border);
  border-radius: 50%;
  background-color: var(--fm-bg);
}
.card[aria-checked='true'] .dot {
  border: 5px solid var(--fm-accent);
  background-color: var(--fm-fg);
}
.text {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.title {
  font-weight: 500;
}
.line {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.figure {
  display: flex;
  flex: none;
  align-items: center;
}
</style>
