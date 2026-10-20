<script setup lang="ts">
// The toast at the bottom of the window, e.g. after a bulk activation, with its
// Undo button. The live region is always in the page, so a screen reader reads
// each new message; the toast stays while the pointer or the focus is on it.
import { useToastStore } from '../stores/toast';

const toasts = useToastStore();
</script>

<template>
  <div class="region" role="status" aria-live="polite">
    <div
      v-if="toasts.toast"
      :key="toasts.toast.id"
      class="toast"
      data-testid="toast"
      @pointerenter="toasts.hold()"
      @pointerleave="toasts.release()"
      @focusin="toasts.hold()"
      @focusout="toasts.release()"
    >
      <span class="message">{{ toasts.toast.message }}</span>
      <button v-if="toasts.toast.action" type="button" class="action" @click="toasts.act()">
        {{ toasts.toast.action.label }}
      </button>
      <button type="button" class="close" aria-label="Dismiss" @click="toasts.dismiss()">×</button>
    </div>
  </div>
</template>

<style scoped>
.region {
  position: fixed;
  bottom: 40px;
  left: 50%;
  z-index: 9;
  transform: translateX(-50%);
}
.toast {
  display: flex;
  align-items: center;
  gap: 12px;
  max-width: min(560px, 90vw);
  padding: 8px 8px 8px 16px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  box-shadow: 0 8px 32px rgb(0 0 0 / 24%);
}
.message {
  flex: 1;
}
button {
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
button:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.action {
  padding: 2px 12px;
  border-color: var(--fm-accent);
  color: var(--fm-accent);
  font-weight: 600;
}
.close {
  width: 24px;
  height: 24px;
  color: var(--fm-muted);
}
@media (prefers-reduced-motion: no-preference) {
  .toast {
    animation: rise 120ms ease-out;
  }
}
@keyframes rise {
  from {
    opacity: 0;
    transform: translateY(4px);
  }
}
</style>
