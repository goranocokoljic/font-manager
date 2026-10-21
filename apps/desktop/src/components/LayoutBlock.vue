<script setup lang="ts">
// One text block of a body-text layout, set as live text in the webview: the
// block's size (pt), line height, tracking (thousandths of an em), paragraph
// spacing, first-line indent, columns and alignment map onto CSS. Columns are
// balanced by the browser (`column-fill: balance`). A wrap block breaks its
// paragraphs into lines; a clip block keeps one line per paragraph and hides
// what does not fit. The values arrive clamped by the core.
//
// An editable block opens its text in a box set in the same face on a double click or
// Enter; leaving the box (or Ctrl+Enter) hands the text to the parent, Escape drops it.
import { computed, nextTick, ref } from 'vue';
import type { TextBlock } from '../api/types';

const props = defineProps<{
  block: TextBlock;
  /** The CSS `font-family` of the face the block is set in. */
  fontFamily: string;
  /** The block's text can be edited; the parent gets it through `edit`. */
  editable?: boolean;
  /** The accessible name of the block and its text box ("Block 1"). */
  label?: string;
}>();

const emit = defineEmits<{ edit: [text: string] }>();

const editing = ref(false);
const text = ref('');
const box = ref<HTMLTextAreaElement | null>(null);
const host = ref<HTMLDivElement | null>(null);

async function startEdit(): Promise<void> {
  if (!props.editable || editing.value) return;
  text.value = props.block.text;
  editing.value = true;
  await nextTick();
  box.value?.focus();
}

function finish(keep: boolean): void {
  if (!editing.value) return;
  editing.value = false;
  if (keep && text.value !== props.block.text) emit('edit', text.value);
  void nextTick(() => host.value?.focus());
}

const clip = computed(() => props.block.overflow === 'clip');

const blockStyle = computed(() => ({
  fontFamily: props.fontFamily,
  fontSize: `${props.block.sizePt}pt`,
  lineHeight: String(props.block.lineHeight),
  letterSpacing: `${(props.block.tracking ?? 0) / 1000}em`,
  textAlign: props.block.align,
  columnCount: props.block.columns > 1 ? props.block.columns : undefined,
}));

const paragraphStyle = computed(() => ({
  marginBottom: `${props.block.paragraphSpacingPt}pt`,
  textIndent: props.block.firstLineIndentEm ? `${props.block.firstLineIndentEm}em` : undefined,
}));

const paragraphs = computed(() => props.block.text.split('\n'));
</script>

<template>
  <textarea
    v-if="editing"
    ref="box"
    v-model="text"
    class="edit"
    data-testid="layout-block-editor"
    :aria-label="`${label ?? 'Block'} text`"
    :rows="Math.max(2, text.split('\n').length)"
    :style="{ fontFamily, fontSize: `${block.sizePt}pt`, lineHeight: String(block.lineHeight) }"
    @blur="finish(true)"
    @keydown.esc.prevent.stop="finish(false)"
    @keydown.enter.ctrl.exact.prevent="finish(true)"
    @keydown.enter.meta.exact.prevent="finish(true)"
  />
  <div
    v-else
    ref="host"
    class="block"
    :class="{ clip, editable }"
    data-testid="layout-block"
    :data-overflow="block.overflow"
    :data-columns="block.columns"
    :data-size="block.sizePt"
    :style="blockStyle"
    :tabindex="editable ? 0 : undefined"
    :role="editable ? 'group' : undefined"
    :aria-label="editable ? label : undefined"
    :title="editable ? 'Double-click or press Enter to edit the text' : undefined"
    @dblclick="startEdit"
    @keydown.enter.exact.self.prevent="startEdit"
  >
    <p v-for="(line, i) in paragraphs" :key="i" class="paragraph" :style="paragraphStyle">
      {{ line }}
    </p>
  </div>
</template>

<style scoped>
.block {
  column-gap: 2em;
  column-fill: balance;
  overflow-wrap: break-word;
}
.paragraph {
  margin-top: 0;
}
.block.clip {
  overflow: hidden;
}
.block.editable:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.edit {
  display: block;
  box-sizing: border-box;
  width: 100%;
  margin-bottom: 8px;
  padding: 4px;
  border: 1px solid var(--fm-control-border);
  background: var(--fm-bg);
  color: var(--fm-fg);
  resize: vertical;
}
.block.clip .paragraph {
  overflow: hidden;
  white-space: nowrap;
}
</style>
