<script setup lang="ts">
// The properties editor of one layout block (E12.3): size, line height, tracking,
// paragraph spacing, first-line indent, columns, alignment and wrap/clip. The fields
// take their limits and steps from the core (`layoutBounds`); every change goes to the
// parent's `apply`, which clamps it through the core, and the field then shows the
// value the block really has. A number field commits on change (a spinner step, Enter
// or leaving it), so typing "14" is not clamped at "1" on the way.
//
// Escape closes the editor; the parent returns focus to the block's size badge.
import { computed, nextTick, onMounted, ref, useId } from 'vue';
import type { BlockAlign, BlockOverflow, LayoutBounds, TextBlock } from '../api/types';
import { t, type MessageKey } from '../i18n';
import type { BlockProps } from '../stores/textLayouts';
import UiSelect from './ui/UiSelect.vue';

const props = defineProps<{
  block: TextBlock;
  bounds: LayoutBounds;
  /** The accessible name of the block ("Block 1"). */
  label: string;
  /** Applies a change; answers the block as it is after the core clamped it. */
  apply: (change: BlockProps) => Promise<TextBlock | null>;
}>();

const emit = defineEmits<{ close: [] }>();

type NumberKey = 'sizePt' | 'lineHeight' | 'tracking' | 'paragraphSpacingPt' | 'firstLineIndentEm';

// Labels are catalog keys, read where they are shown, so they follow the language.
const numbers: { key: NumberKey; label: string; unit: string }[] = [
  { key: 'sizePt', label: 'blocks.fields.size', unit: 'pt' },
  { key: 'lineHeight', label: 'blocks.fields.lineHeight', unit: '×' },
  { key: 'tracking', label: 'blocks.fields.tracking', unit: '/1000 em' },
  { key: 'paragraphSpacingPt', label: 'blocks.fields.paragraphSpacing', unit: 'pt' },
  { key: 'firstLineIndentEm', label: 'blocks.fields.firstLineIndent', unit: 'em' },
];

const aligns: BlockAlign[] = ['start', 'center', 'end', 'justify'];
const alignOptions = computed(() =>
  aligns.map((value) => ({ value, label: t(`blocks.aligns.${value}`) })),
);

const overflows: BlockOverflow[] = ['wrap', 'clip'];
const overflowOptions = computed(() =>
  overflows.map((value) => ({ value, label: t(`blocks.overflows.${value}`) })),
);

const columnOptions = computed(() => {
  const out: { value: number; label: string }[] = [];
  for (let c = props.bounds.columns.min; c <= props.bounds.columns.max; c++) {
    out.push({ value: c, label: String(c) });
  }
  return out;
});

const root = ref<HTMLElement | null>(null);
const id = useId();

onMounted(() => {
  void nextTick(() => root.value?.querySelector<HTMLElement>('input, [role="combobox"]')?.focus());
});

async function setNumber(key: NumberKey, e: Event): Promise<void> {
  const input = e.target as HTMLInputElement;
  // An empty or unreadable field is "not a number": the core gives it the default.
  const value = input.value.trim() === '' ? Number.NaN : Number(input.value);
  const block = await props.apply({ [key]: value });
  // Show the value the block has now, even when clamping left it where it was.
  if (block) input.value = String(block[key]);
}

async function setChoice(change: BlockProps): Promise<void> {
  await props.apply(change);
}
</script>

<template>
  <div
    ref="root"
    class="props"
    role="dialog"
    :aria-label="t('blocks.properties', { label })"
    data-testid="block-properties"
    @keydown.esc.prevent.stop="emit('close')"
  >
    <div v-for="n in numbers" :key="n.key" class="field">
      <label :for="`${id}-${n.key}`">{{ t(n.label as MessageKey) }}</label>
      <input
        :id="`${id}-${n.key}`"
        type="number"
        :name="n.key"
        :value="block[n.key]"
        :min="bounds[n.key].min ?? undefined"
        :max="bounds[n.key].max ?? undefined"
        :step="bounds[n.key].step ?? 'any'"
        @change="setNumber(n.key, $event)"
      />
      <span class="unit" aria-hidden="true">{{ n.unit }}</span>
    </div>
    <div class="field">
      <label :for="`${id}-columns`">{{ t('blocks.fields.columns') }}</label>
      <UiSelect
        :id="`${id}-columns`"
        name="columns"
        :model-value="block.columns"
        :options="columnOptions"
        @update:model-value="setChoice({ columns: $event })"
      />
    </div>
    <div class="field">
      <label :for="`${id}-align`">{{ t('blocks.fields.alignment') }}</label>
      <UiSelect
        :id="`${id}-align`"
        name="align"
        :model-value="block.align"
        :options="alignOptions"
        @update:model-value="setChoice({ align: $event })"
      />
    </div>
    <div class="field">
      <label :for="`${id}-overflow`">{{ t('blocks.fields.overflow') }}</label>
      <UiSelect
        :id="`${id}-overflow`"
        name="overflow"
        :model-value="block.overflow"
        :options="overflowOptions"
        @update:model-value="setChoice({ overflow: $event })"
      />
    </div>
    <button type="button" class="close" @click="emit('close')">{{ t('common.close') }}</button>
  </div>
</template>

<style scoped>
.props {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 16px;
  align-items: center;
  margin: 4px 0 12px;
  padding: 8px 12px;
  border: 1px solid var(--fm-control-border);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  font-family: var(--fm-font-ui);
  font-size: var(--fm-text-sm);
}
.field {
  display: flex;
  gap: 4px;
  align-items: center;
}
.field input {
  width: 6em;
}
.field input {
  border: 1px solid var(--fm-control-border);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
}
.unit {
  color: var(--fm-muted);
}
.close {
  margin-inline-start: auto;
}
.props :focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
