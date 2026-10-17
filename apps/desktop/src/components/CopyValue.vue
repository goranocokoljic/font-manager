<script setup lang="ts">
// A value that copies itself to the clipboard on click (names and paths).
import { onBeforeUnmount, ref } from 'vue';
import { copyText } from '../api/clipboard';

const props = defineProps<{ value: string; label: string }>();

const status = ref<'copied' | 'failed' | null>(null);
let timer: ReturnType<typeof setTimeout> | undefined;

async function copy(): Promise<void> {
  try {
    await copyText(props.value);
    status.value = 'copied';
  } catch {
    status.value = 'failed';
  }
  clearTimeout(timer);
  timer = setTimeout(() => (status.value = null), 1500);
}

onBeforeUnmount(() => clearTimeout(timer));
</script>

<template>
  <button
    type="button"
    class="copy"
    :aria-label="`Copy ${label}: ${value}`"
    title="Click to copy"
    @click="copy"
  >
    {{ value }}
  </button>
  <span class="status" role="status">
    <template v-if="status === 'copied'">Copied</template>
    <template v-else-if="status === 'failed'">Could not copy</template>
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
