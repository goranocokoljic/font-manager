<script setup lang="ts">
// "Copy CSS" in the Axes and Features tabs: copies the declaration fm-core
// builds for what the user changed, with CopyValue's inline feedback.
import { t } from '../i18n';
import UiButton from './ui/UiButton.vue';
import { useCopy } from './useCopy';

const props = defineProps<{
  /** What the CSS is for, in the accessible name ("axes", "features"). */
  label: string;
  /** The declaration to copy (`font-variation-settings: "wght" 650;`). */
  css: () => Promise<string>;
}>();

const { status, copy } = useCopy();
</script>

<template>
  <span class="copy-css">
    <span class="status" role="status">
      <template v-if="status === 'copied'">{{ t('common.copied') }}</template>
      <template v-else-if="status === 'failed'">{{ t('copy.failed') }}</template>
    </span>
    <UiButton :aria-label="t('copy.cssFor', { label })" @click="copy(props.css())">
      {{ t('copy.css') }}
    </UiButton>
  </span>
</template>

<style scoped>
.copy-css {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.status {
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
</style>
