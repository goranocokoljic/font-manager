<script setup lang="ts">
// A value that copies itself to the clipboard on click (names and paths).
import { t } from '../i18n';
import { useCopy } from './useCopy';

const props = defineProps<{ value: string; label: string }>();

const { status, copy: copyValue } = useCopy();
const copy = () => copyValue(props.value);
</script>

<template>
  <button
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
  background: transparent;
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
  font-size: 11.5px;
}
.status:empty {
  margin: 0;
}
</style>
