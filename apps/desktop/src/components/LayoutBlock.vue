<script setup lang="ts">
// One text block of a body-text layout, set as live text in the webview: the
// block's size (pt), line height, tracking (thousandths of an em), paragraph
// spacing, first-line indent, columns and alignment map onto CSS. Columns are
// balanced by the browser (`column-fill: balance`). A wrap block breaks its
// paragraphs into lines; a clip block keeps one line per paragraph and hides
// what does not fit. The values arrive clamped by the core.
import { computed } from 'vue';
import type { TextBlock } from '../api/types';

const props = defineProps<{
  block: TextBlock;
  /** The CSS `font-family` of the face the block is set in. */
  fontFamily: string;
}>();

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
  <div
    class="block"
    :class="{ clip }"
    data-testid="layout-block"
    :data-overflow="block.overflow"
    :data-columns="block.columns"
    :data-size="block.sizePt"
    :style="blockStyle"
  >
    <p v-for="(text, i) in paragraphs" :key="i" class="paragraph" :style="paragraphStyle">
      {{ text }}
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
.block.clip .paragraph {
  overflow: hidden;
  white-space: nowrap;
}
</style>
