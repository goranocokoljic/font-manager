<script setup lang="ts">
// The Waterfall section of the family page (E50.10, board 2): the hero's face in the hero's text
// on the family size ladder, smallest first, each line labelled with its size at the start.
// Every line is the wall's waterfall line (`FontPreviewRow`), so the hero's instance (the axes
// store), features and palette apply as they do on the wall; this component only lays out the
// ladder.
import type { FaceSummary } from '../api/types';
import { t } from '../i18n';
import { FAMILY_WATERFALL_SIZES } from '../list/waterfall';
import FontPreviewRow from './FontPreviewRow.vue';

defineProps<{
  /** The hero's face: the family's default style, or the style chosen. Null until it has loaded. */
  face: FaceSummary | null;
  /** The preview text, as the hero shows it (the rail's language, else the user's text). */
  text: string;
  /** Characters of the text the face lacks: drawn as tofu, as on the hero. */
  missing: string[];
}>();

const from = FAMILY_WATERFALL_SIZES[0];
const to = FAMILY_WATERFALL_SIZES[FAMILY_WATERFALL_SIZES.length - 1];
</script>

<template>
  <div class="family-waterfall" data-testid="family-waterfall">
    <p class="hint">{{ t('family.waterfallHint', { from, to }) }}</p>
    <div
      v-if="face"
      class="lines fm-true-size"
      role="group"
      :aria-label="t('family.waterfallLines', { from, to })"
    >
      <FontPreviewRow
        v-for="size in FAMILY_WATERFALL_SIZES"
        :key="size"
        class="line"
        data-testid="family-waterfall-line"
        :data-size="size"
        :face="face"
        :label="String(size)"
        :text="text"
        :size-px="size"
        :missing="missing"
        :show-variable="false"
      />
    </div>
  </div>
</template>

<style scoped>
.family-waterfall {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
}
.hint {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.lines {
  display: flex;
  flex-direction: column;
}
/* The size at the start of each line, in 10 px mono, on the line's baseline (board 2). */
.line {
  flex-direction: row;
  align-items: baseline;
  gap: var(--fm-space-4);
  height: auto;
  padding-block: 16px 4px;
  padding-inline: 0;
}
.line :deep(.label) {
  flex: none;
  justify-content: flex-end;
  width: 3ch;
  font-family: var(--fm-font-mono);
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}
/* The hero lists the missing characters once, under it; each line only draws them as tofu. */
.line :deep(.label .missing-glyphs) {
  display: none;
}
.line :deep(.sample) {
  flex: 1;
  min-width: 0;
}
</style>
