<script setup lang="ts">
// An on/off switch: accent when on. Attributes (aria-label, @click) fall through to the button.
// The knob slides by a transform over `--fm-motion-fast` (E46.4, motion table "Activate").
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
  transform: translate(0, -50%);
  transition: transform var(--fm-motion-fast) var(--fm-ease);
}
.ui-switch[aria-checked='true'] {
  border-color: var(--fm-accent);
  background: var(--fm-accent);
}
/* 14 px: from 3 px to 17 px inside the 32 px track. */
.ui-switch[aria-checked='true'] .knob {
  background-color: var(--fm-on-accent);
  transform: translate(14px, -50%);
}
[dir='rtl'] .ui-switch[aria-checked='true'] .knob {
  transform: translate(-14px, -50%);
}
</style>
