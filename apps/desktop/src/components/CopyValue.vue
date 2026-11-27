<script setup lang="ts">
// A value that copies itself to the clipboard on click (names and paths). With `text`
// (E57.5) it is a quiet button with that text ("Copy path") instead of the value itself:
// for a setting row, which shows the value in the mono face and has one control at its end.
import { t } from '../i18n';
import UiButton from './ui/UiButton.vue';
import { useCopy } from './useCopy';

const props = defineProps<{ value: string; label: string; text?: string }>();

const { status, copy: copyValue } = useCopy();
const copy = () => copyValue(props.value);
</script>

<template>
  <UiButton v-if="text" :aria-label="t('copy.value', { label, value })" @click="copy">
    {{ text }}
  </UiButton>
  <button
    v-else
    type="button"
    class="copy"
    :aria-label="t('copy.value', { label, value })"
    :title="t('copy.clickToCopy')"
    @click="copy"
  >
    <bdi>{{ value }}</bdi>
  </button>
  <span class="status" role="status">
    <template v-if="status === 'copied'">{{ t('common.copied') }}</template>
    <template v-else-if="status === 'failed'">{{ t('copy.failed') }}</template>
  </span>
</template>

<style scoped>
.copy {
  padding: 0;
  border: 0;
  background-color: transparent;
  color: inherit;
  font: inherit;
  text-align: start;
  overflow-wrap: anywhere;
  cursor: copy;
}
.copy:hover {
  text-decoration: underline dotted;
}
.status {
  margin-inline-start: 8px;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
.status:empty {
  margin: 0;
}
</style>
