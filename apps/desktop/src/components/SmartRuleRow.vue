<script setup lang="ts">
// One row of the smart-collection rule editor (E14.1): what it tests, then its value
// from a menu the API filled (a range for weight and width, typed text for "supports
// text", a number of days for "date added"), and Remove. A part of a saved rule the
// editor has no row for shows as "Kept as saved" and is saved unchanged.
import { t } from '../i18n';
import {
  DIMENSIONS,
  choicesFor,
  useRuleEditorStore,
  type Dimension,
  type DraftRow,
} from '../stores/ruleEditor';

// `inline` is `label` inside a sentence ("Remove rule 2").
const props = defineProps<{ row: DraftRow; label: string; inline: string }>();
const emit = defineEmits<{ remove: [] }>();
const editor = useRuleEditorStore();

function keptText(): string {
  const rule = props.row.kept;
  if (!rule) return '';
  return rule.kind === 'group' ? t('rules.nestedGroup') : Object.keys(rule.filters).join(', ');
}

/** Sets the row's `field` to what a select or an input holds now. */
function set(field: 'value' | 'min' | 'max', e: Event): void {
  editor.update(props.row, { [field]: (e.target as HTMLInputElement | HTMLSelectElement).value });
}

function onDimension(e: Event): void {
  editor.setDimension(props.row, (e.target as HTMLSelectElement).value as Dimension);
}
</script>

<template>
  <div class="row" :data-rule="label">
    <span v-if="row.dimension === 'kept'" class="kept" :data-focus="row.key" tabindex="-1">
      {{ t('rules.keptAsSaved', { what: keptText() }) }}
    </span>
    <template v-else>
      <select
        :value="row.dimension"
        :data-focus="row.key"
        :aria-label="t('rules.whatItTests', { label })"
        @change="onDimension"
      >
        <option v-for="d in DIMENSIONS" :key="d.id" :value="d.id">{{ d.label }}</option>
      </select>
      <template v-if="row.dimension === 'weight' || row.dimension === 'width'">
        <select
          :value="row.min"
          :aria-label="t('rules.fromLabel', { label })"
          @change="set('min', $event)"
        >
          <option
            v-for="c in choicesFor(row.dimension, editor.choices) ?? []"
            :key="c.value"
            :value="c.value"
          >
            {{ c.label }}
          </option>
        </select>
        <span aria-hidden="true">{{ t('rules.to') }}</span>
        <select
          :value="row.max"
          :aria-label="t('rules.toLabel', { label })"
          @change="set('max', $event)"
        >
          <option
            v-for="c in choicesFor(row.dimension, editor.choices) ?? []"
            :key="c.value"
            :value="c.value"
          >
            {{ c.label }}
          </option>
        </select>
      </template>
      <input
        v-else-if="row.dimension === 'text'"
        :value="row.value"
        type="text"
        :placeholder="t('rules.textPlaceholder')"
        :aria-label="t('rules.textLabel', { label })"
        @input="set('value', $event)"
      />
      <span v-else-if="row.dimension === 'added'" class="days">
        <span aria-hidden="true">{{ t('rules.within') }}</span>
        <input
          :value="row.value"
          type="number"
          min="0"
          step="1"
          :aria-label="t('rules.daysLabel', { label })"
          @input="set('value', $event)"
        />
        <span aria-hidden="true">{{ t('rules.days') }}</span>
      </span>
      <select
        v-else
        :value="row.value"
        :aria-label="t('rules.valueLabel', { label })"
        @change="set('value', $event)"
      >
        <option value="" disabled>{{ t('rules.choose') }}</option>
        <option
          v-for="c in choicesFor(row.dimension, editor.choices) ?? []"
          :key="c.value"
          :value="c.value"
        >
          {{ c.label }}
        </option>
      </select>
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
select,
input {
  padding: 2px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
select:focus-visible,
input:focus-visible,
.button:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
</style>
