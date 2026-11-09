<script setup lang="ts">
// The toast at the bottom of the window, e.g. after a bulk activation, with its
// Undo button. The live region is always in the page, so a screen reader reads
// each new message; the toast stays while the pointer or the focus is on it.
import { t } from '../i18n';
import { useToastStore } from '../stores/toast';
import { FmSlide } from '../motion';
import { retireToast } from './retireToast';

const toasts = useToastStore();
</script>

<template>
  <div class="region" role="status" aria-live="polite">
    <FmSlide from="bottom" distance="16px" reduced="fade" @before-leave="retireToast">
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
        <button
          type="button"
          class="close"
          :aria-label="t('toast.dismiss')"
          @click="toasts.dismiss()"
        >
          ×
        </button>
      </div>
    </FmSlide>
  </div>
</template>

<style scoped>
.region {
  position: fixed;
  bottom: 40px;
  /* Centered in either direction: both insets 0, the box as wide as its toast. */
  inset-inline: 0;
  z-index: var(--fm-z-toast);
  width: fit-content;
  margin-inline: auto;
}
.toast {
  display: flex;
  align-items: center;
  gap: 12px;
  max-width: min(560px, 90vw);
  padding-block: 8px;
  padding-inline: 16px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  box-shadow: var(--fm-shadow-2);
}
.message {
  flex: 1;
}
button {
  border: 1px solid transparent;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
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
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font-weight: 600;
}
.close {
  width: 24px;
  height: 24px;
  color: var(--fm-muted);
}
/* A leaving one fades out on top of the next, without moving it. */
.toast.fm-slide-leave-active {
  position: absolute;
  inset-inline: 0;
  bottom: 0;
}
</style>
