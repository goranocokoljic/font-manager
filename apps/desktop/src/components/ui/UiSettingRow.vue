<script setup lang="ts">
// One setting row (E57.3, board 15) inside a `UiSettingGroup`: a title, at most one line
// under it, an optional value in the mono face (a path, an address), and exactly one control
// at its end. The row names its control: the default slot gets `control`, the attributes to
// bind on it (`v-bind="control"`), which make the title its label (`aria-labelledby`) and the
// line its description (`aria-describedby`). No caller writes those ids by hand; a local copy
// of a setting row is a bug (DESIGN.md, "Settings"). `error` (E57.4) is a failed change's
// message, said on the row under its line (an alert) and added to the control's description.
// `empty` (E57.5) is said, muted and in the text face, where the value goes while there is
// none ("Not chosen yet."); the `value` slot (E57.5) replaces the value's text, for a caller
// that marks it up (a test id), and stays in the mono face. `live` (E57.7) makes the title a
// status region, for a row whose title is a state (About's "Font Fu is up to date."): a change
// of the state is announced once, the line under it is not.
import { computed, useId } from 'vue';

const props = defineProps<{
  title: string;
  line?: string;
  value?: string;
  empty?: string;
  error?: string;
  live?: boolean;
}>();

defineSlots<{
  value?: () => unknown;
  default?: (props: {
    control: { 'aria-labelledby': string; 'aria-describedby': string | undefined };
  }) => unknown;
}>();

const uid = useId();
const titleId = `${uid}-title`;
const lineId = `${uid}-line`;
const errorId = `${uid}-error`;
const control = computed(() => ({
  'aria-labelledby': titleId,
  'aria-describedby':
    [props.line ? lineId : '', props.error ? errorId : ''].filter(Boolean).join(' ') || undefined,
}));
</script>

<template>
  <div class="ui-setting-row">
    <div class="text">
      <span :id="titleId" class="title" :role="live ? 'status' : undefined">{{ title }}</span>
      <span v-if="line" :id="lineId" class="line">{{ line }}</span>
      <bdi v-if="value || $slots.value" class="value"
        ><slot name="value">{{ value }}</slot></bdi
      >
      <span v-else-if="empty" class="line empty">{{ empty }}</span>
      <span v-if="error" :id="errorId" role="alert" class="error">{{ error }}</span>
    </div>
    <div v-if="$slots.default" class="control">
      <slot :control="control" />
    </div>
  </div>
</template>

<style scoped>
.ui-setting-row {
  display: flex;
  align-items: center;
  gap: var(--fm-space-4);
  padding: var(--fm-space-3) var(--fm-space-4);
}
.text {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.title {
  font-weight: 500;
}
.line {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.error {
  color: var(--fm-danger);
  font-size: var(--fm-text-sm);
}
.value {
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-sm);
  overflow-wrap: anywhere;
}
.control {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--fm-space-2);
}
</style>
