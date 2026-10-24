<script setup lang="ts">
// Quick look (Space, from the spec's keyboard model): the face under the list's
// cursor in a large preview over the list, in the list's sample text. Live text in
// the webview, through the shared face loader, like the list rows.
//
// A modal dialog: the focus starts on Close and stays in the sheet, plain keys do
// not reach the list's shortcuts, and Space, Escape or Close return the focus to
// where it was (the list row).
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import type { FaceSummary } from '../api/types';
import { t } from '../i18n';
import { sheetReturnTarget } from '../keyboard/returnFocus';
import { useFaceLoader } from '../preview/faceLoader';
import { fontNameSample, sampleText } from '../preview/sampleText';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import { useQuickLookStore } from '../stores/quickLook';

const props = defineProps<{ face: FaceSummary }>();

const quickLook = useQuickLookStore();
const preview = usePreviewStore();
const loader = useFaceLoader();

const dialog = ref<HTMLElement | null>(null);
const closeButton = ref<HTMLButtonElement | null>(null);
const family = ref<string | null>(null);
let returnFocus: HTMLElement | null = null;
const held = props.face.id;

loader.acquire(held).then(
  (name) => {
    family.value = name;
  },
  () => undefined,
);

const name = computed(() => fontNameSample(props.face));
const text = computed(() =>
  sampleText(preview.settingsFor(ALL_FONTS_VIEW), props.face, preview.pangramText),
);
const font = computed(() =>
  family.value ? `"${family.value}", var(--fm-preview-fallback, system-ui), sans-serif` : undefined,
);

onMounted(async () => {
  returnFocus = sheetReturnTarget();
  document.addEventListener('focusin', onFocusIn);
  await nextTick();
  closeButton.value?.focus();
});
onBeforeUnmount(() => {
  document.removeEventListener('focusin', onFocusIn);
  loader.release(held);
  if (returnFocus?.isConnected) returnFocus.focus();
});

/** Space and Escape close; Tab stays on Close; plain keys stay out of the list's shortcuts. */
function onKeyDown(e: KeyboardEvent): void {
  const space = e.key === ' ' && e.target === dialog.value;
  if (e.key === 'Escape' || space) {
    e.preventDefault();
    e.stopPropagation();
    quickLook.close();
    return;
  }
  if (!e.ctrlKey && !e.metaKey) e.stopPropagation();
  if (e.key === 'Tab') {
    e.preventDefault();
    closeButton.value?.focus();
  }
}

/** Focus that moves to the page behind the sheet is taken back to Close. */
function onFocusIn(e: FocusEvent): void {
  const target = e.target;
  if (!dialog.value || !(target instanceof Element) || dialog.value.contains(target)) return;
  if (target.closest('[role="dialog"]')) return;
  closeButton.value?.focus();
}
</script>

<template>
  <div class="backdrop" @mousedown.self="quickLook.close">
    <div
      ref="dialog"
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="fm-quick-look-title"
      aria-describedby="fm-quick-look-hint"
      tabindex="-1"
      data-testid="quick-look"
      @keydown="onKeyDown"
    >
      <header class="head">
        <h2 id="fm-quick-look-title">{{ t('quickLook.title', { name }) }}</h2>
        <button ref="closeButton" type="button" @click="quickLook.close">
          {{ t('common.close') }}
        </button>
      </header>
      <p class="sample" dir="auto" :style="{ fontFamily: font }">{{ text }}</p>
      <p id="fm-quick-look-hint" class="hint">{{ t('quickLook.hint') }}</p>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 10;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  background: rgb(0 0 0 / 20%);
}
.sheet {
  display: flex;
  flex-direction: column;
  gap: 12px;
  width: min(1100px, 96vw);
  max-height: 90vh;
  padding: 16px;
  border: 1px solid var(--fm-border);
  border-top: 0;
  border-radius: 0 0 8px 8px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  box-shadow: 0 8px 32px rgb(0 0 0 / 24%);
  overflow: auto;
}
.sheet:focus {
  outline: none;
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
h2 {
  margin: 0;
  font-size: 15px;
}
.sample {
  margin: 0;
  font-size: 96px;
  line-height: 1.15;
  overflow-wrap: anywhere;
}
.hint {
  margin: 0;
  color: var(--fm-muted);
  font-size: 11.5px;
}
@media (prefers-reduced-motion: no-preference) {
  .sheet {
    animation: rise 150ms ease-out;
  }
}
@keyframes rise {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
}
</style>
