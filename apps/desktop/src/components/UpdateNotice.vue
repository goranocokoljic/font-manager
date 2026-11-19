<script setup lang="ts">
// The quiet notice of an automatic update find (E55.8): "Font Fu 0.1.1 is available" with
// "See what's new", a card in the style of the app's notices (the AI assistant's question,
// E27.6), at the bottom end of the window, on every page.
// It never opens the dialog by itself and never takes the focus. It stays until the user
// dismisses it (then until the next start) or opens the dialog; the dot on Settings stays
// while the update is on offer. The live region is always in the page, so a screen reader
// reads the notice when it comes; Escape inside it dismisses it.
import { t } from '../i18n';
import { FmSlide } from '../motion';
import { useUpdatesStore } from '../stores/updates';
import UiIconButton from './ui/UiIconButton.vue';
import { retireToast } from './retireToast';

const updates = useUpdatesStore();

/** The focus goes to the main view when the notice it was in goes away; never to body. */
function giveBack(from: Element): void {
  if (!from.contains(document.activeElement)) return;
  const main = document.querySelector<HTMLElement>('main');
  if (!main) return;
  if (!main.hasAttribute('tabindex')) main.tabIndex = -1;
  main.focus();
}

function dismiss(e: Event): void {
  const notice = (e.currentTarget as Element).closest('.notice');
  updates.dismissNotice();
  if (notice) giveBack(notice);
}
</script>

<template>
  <div class="update-region" role="status" aria-live="polite">
    <FmSlide from="bottom" distance="8px" speed="fast" reduced="fade" @before-leave="retireToast">
      <section
        v-if="updates.notice && updates.update"
        class="notice"
        data-testid="update-notice"
        :aria-label="t('updates.notice.label')"
        @keydown.esc.stop="dismiss"
      >
        <span class="message">{{
          t('updates.notice.message', { version: updates.update.version })
        }}</span>
        <button type="button" class="action" @click="updates.openDialog()">
          {{ t('updates.notice.seeWhatsNew') }}
        </button>
        <UiIconButton
          icon="close"
          size="sm"
          :label="t('updates.notice.dismiss')"
          @click="dismiss"
        />
      </section>
    </FmSlide>
  </div>
</template>

<style scoped>
.update-region {
  position: fixed;
  inset-inline-end: var(--fm-space-4);
  bottom: 48px;
  z-index: var(--fm-z-toast);
}
.notice {
  display: flex;
  align-items: center;
  gap: 12px;
  max-width: min(420px, calc(50vw - 48px));
  min-height: 36px;
  padding-block: 6px;
  padding-inline: 16px 6px;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  box-shadow: var(--fm-shadow-2);
}
.message {
  flex: 1;
}
.action {
  padding: 2px 12px;
  border: 1px solid var(--fm-accent);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font: inherit;
  font-weight: 600;
}
.action:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
