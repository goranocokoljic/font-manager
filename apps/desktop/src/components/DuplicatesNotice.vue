<script setup lang="ts">
// The notice after a scan that found copies (E56.15, board 14's entry sheet): "Fonts added ·
// 120 fonts", how many of the folder's files are copies, "Review" and dismiss — a card in
// the style of the app's notices (UpdateNotice), at the bottom end of the window, on every
// page. One per scan; it never opens the module by itself and never takes the focus.
// Review opens the module's overview. The live region is always in the page, so a screen
// reader reads the notice when it comes; Escape inside it dismisses it.
import { useRouter } from 'vue-router';
import { t } from '../i18n';
import { FmSlide } from '../motion';
import { useDuplicateSummaryStore } from '../stores/duplicateSummary';
import UiIconButton from './ui/UiIconButton.vue';
import { retireToast } from './retireToast';

const store = useDuplicateSummaryStore();
const router = useRouter();

/** The focus goes to the main view when the notice it was in goes away; never to body. */
function giveBack(from: Element | null): void {
  if (!from?.contains(document.activeElement)) return;
  const main = document.querySelector<HTMLElement>('main');
  if (!main) return;
  if (!main.hasAttribute('tabindex')) main.tabIndex = -1;
  main.focus();
}

function dismiss(e: Event): void {
  const notice = (e.currentTarget as Element).closest('.notice');
  store.dismissNotice();
  giveBack(notice);
}

async function review(): Promise<void> {
  store.dismissNotice();
  await router.push({ name: 'duplicates' });
}
</script>

<template>
  <div class="duplicates-region" role="status" aria-live="polite">
    <FmSlide from="bottom" distance="8px" speed="fast" reduced="fade" @before-leave="retireToast">
      <section
        v-if="store.notice"
        :key="store.notice.job"
        class="notice"
        data-testid="duplicates-notice"
        :aria-label="t('duplicatesNotice.label')"
        @keydown.esc.stop="dismiss"
      >
        <span class="message">
          <span>{{
            t('duplicatesNotice.added', {
              folder: store.notice.folder,
              count: store.notice.added,
            })
          }}</span>
          <span class="muted">{{
            t('duplicatesNotice.copies', { count: store.notice.copies })
          }}</span>
        </span>
        <button type="button" class="action" @click="review">
          {{ t('duplicatesNotice.review') }}
        </button>
        <UiIconButton
          icon="close"
          size="sm"
          :label="t('duplicatesNotice.dismiss')"
          @click="dismiss"
        />
      </section>
    </FmSlide>
  </div>
</template>

<style scoped>
.duplicates-region {
  position: fixed;
  inset-inline-end: var(--fm-space-4);
  bottom: 96px;
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
  display: grid;
  flex: 1;
  gap: 2px;
}
.muted {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
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
