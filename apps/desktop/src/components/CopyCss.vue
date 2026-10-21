<script setup lang="ts">
// "Copy CSS" in the Axes and Features tabs: copies the declaration fm-core
// builds for what the user changed, with CopyValue's inline feedback.
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
      <template v-if="status === 'copied'">Copied</template>
      <template v-else-if="status === 'failed'">Could not copy</template>
    </span>
    <button
      type="button"
      class="button"
      :aria-label="`Copy CSS for the ${label}`"
      @click="copy(props.css())"
    >
      Copy CSS
    </button>
  </span>
</template>

<style scoped>
.copy-css {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.button {
  padding: 2px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.status {
  color: var(--fm-muted);
  font-size: 11.5px;
}
</style>
