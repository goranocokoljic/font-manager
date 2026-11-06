<script setup lang="ts">
// An on/off switch: accent when on. Attributes (aria-label, @click) fall through to the button.
defineProps<{ checked: boolean }>();
</script>

<template>
  <button type="button" role="switch" class="ui-switch" :aria-checked="checked">
    <span class="knob" aria-hidden="true" />
  </button>
</template>

<style scoped>
.ui-switch {
  position: relative;
  box-sizing: border-box;
  flex: none;
  width: 32px;
  height: 18px;
  padding: 0;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface-2);
  transition:
    background-color var(--fm-motion-fast) var(--fm-ease),
    border-color var(--fm-motion-fast) var(--fm-ease);
}
.knob {
  position: absolute;
  top: 50%;
  inset-inline-start: 3px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--fm-control-border);
  transform: translateY(-50%);
  transition: inset-inline-start var(--fm-motion-fast) var(--fm-ease);
}
.ui-switch[aria-checked='true'] {
  border-color: var(--fm-accent);
  background: var(--fm-accent);
}
.ui-switch[aria-checked='true'] .knob {
  inset-inline-start: 17px;
  background-color: var(--fm-on-accent);
}
</style>
