<script setup lang="ts">
// One row of the smart-collection rule editor (E14.1): what it tests, then its value
// from a menu the API filled (a range for weight and width, typed text for "supports
// text", a number of days for "date added"), and Remove. A part of a saved rule the
// editor has no row for shows as "Kept as saved" and is saved unchanged.
import { computed } from 'vue';
import { t } from '../i18n';
import {
  DIMENSIONS,
  choicesFor,
  useRuleEditorStore,
  type Dimension,
  type DraftRow,
} from '../stores/ruleEditor';
import UiSelect from './ui/UiSelect.vue';

// `inline` is `label` inside a sentence ("Remove rule 2").
const props = defineProps<{ row: DraftRow; label: string; inline: string }>();
const emit = defineEmits<{ remove: [] }>();
const editor = useRuleEditorStore();

const dimensionOptions = computed(() => DIMENSIONS.map((d) => ({ value: d.id, label: d.label })));
const choices = computed(() =>
  props.row.dimension === 'kept' ? [] : (choicesFor(props.row.dimension, editor.choices) ?? []),
);

function keptText(): string {
  const rule = props.row.kept;
  if (!rule) return '';
  return rule.kind === 'group' ? t('rules.nestedGroup') : Object.keys(rule.filters).join(', ');
}

/** Sets the row's `field` to what a picker or an input holds now. */
function set(field: 'value' | 'min' | 'max', value: string): void {
  editor.update(props.row, { [field]: value });
}
function setFrom(field: 'value' | 'min' | 'max', e: Event): void {
  set(field, (e.target as HTMLInputElement).value);
}

function onDimension(dimension: Dimension): void {
  editor.setDimension(props.row, dimension);
}
</script>

<template>
  <div class="row" :data-rule="label">
    <span v-if="row.dimension === 'kept'" class="kept" :data-focus="row.key" tabindex="-1">
      {{ t('rules.keptAsSaved', { what: keptText() }) }}
    </span>
    <template v-else>
      <UiSelect
        :model-value="row.dimension"
        :options="dimensionOptions"
        :data-focus="row.key"
        :aria-label="t('rules.whatItTests', { label })"
        @update:model-value="onDimension"
      />
      <template v-if="row.dimension === 'weight' || row.dimension === 'width'">
        <UiSelect
          :model-value="row.min"
          :options="choices"
          :aria-label="t('rules.fromLabel', { label })"
          @update:model-value="set('min', $event)"
        />
        <span aria-hidden="true">{{ t('rules.to') }}</span>
        <UiSelect
          :model-value="row.max"
          :options="choices"
          :aria-label="t('rules.toLabel', { label })"
          @update:model-value="set('max', $event)"
        />
      </template>
      <input
        v-else-if="row.dimension === 'text'"
        :value="row.value"
        type="text"
        :placeholder="t('rules.textPlaceholder')"
        :aria-label="t('rules.textLabel', { label })"
        @input="setFrom('value', $event)"
      />
      <span v-else-if="row.dimension === 'added'" class="days">
        <span aria-hidden="true">{{ t('rules.within') }}</span>
        <input
          :value="row.value"
          type="number"
          min="0"
          step="1"
          :aria-label="t('rules.daysLabel', { label })"
          @input="setFrom('value', $event)"
        />
        <span aria-hidden="true">{{ t('rules.days') }}</span>
      </span>
      <UiSelect
        v-else
        :model-value="row.value"
        :options="choices"
        :placeholder="t('rules.choose')"
        :aria-label="t('rules.valueLabel', { label })"
        @update:model-value="set('value', $event)"
      />
    </template>
    <button
      type="button"
      class="button"
      :aria-label="t('rules.removeLabel', { label: inline })"
      @click="emit('remove')"
    >
      {{ t('common.remove') }}
    </button>
  </div>
</template>

<style scoped>
.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.days {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.days input {
  width: 5em;
}
.kept {
  flex: 1;
  color: var(--fm-muted);
}
input {
  padding: 2px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: inherit;
  font: inherit;
}
input:focus-visible,
.button:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: inherit;
  font: inherit;
}
</style>
