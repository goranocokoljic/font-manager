<script setup lang="ts">
// Quick look (Space, from the spec's keyboard model): the face under the list's
// cursor in a large preview over the list, in the list's sample text. Live text in
// the webview, through the shared face loader, like the list rows.
//
// A modal dialog: the focus starts on Close and stays in the sheet, plain keys do
// not reach the list's shortcuts, and Space, Escape or Close return the focus to
// where it was (the list row).
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { FaceId, FaceSummary } from '../api/types';
import { t } from '../i18n';
import UiIconButton from './ui/UiIconButton.vue';
import { sheetReturnTarget } from '../keyboard/returnFocus';
import { useFaceLoader } from '../preview/faceLoader';
import {
  LETTERS,
  LOREM,
  NUMERALS,
  SYMBOLS,
  fontNameSample,
  sampleText,
} from '../preview/sampleText';
import { useFontListStore } from '../stores/fontList';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import { useQuickLookStore } from '../stores/quickLook';

const props = defineProps<{ face: FaceSummary }>();

const quickLook = useQuickLookStore();
const preview = usePreviewStore();
const fontList = useFontListStore();
const loader = useFaceLoader();

const dialog = ref<HTMLElement | null>(null);
const closeButton = ref<InstanceType<typeof UiIconButton> | null>(null);
const focusClose = () => (closeButton.value?.$el as HTMLElement | undefined)?.focus();
const family = ref<string | null>(null);
let returnFocus: HTMLElement | null = null;
let held: FaceId | null = null;

// The face changes while the sheet stays open (arrow keys): load the new one, then
// let the old one go, so the specimen never falls back to a system font.
watch(
  () => props.face.id,
  (id) => {
    const previous = held;
    held = id;
    loader.acquire(id).then(
      (name) => {
        if (held !== id) return;
        family.value = name;
        if (previous !== null && previous !== id) loader.release(previous);
      },
      () => undefined,
    );
  },
  { immediate: true },
);

const name = computed(() => fontNameSample(props.face));
const text = computed(() =>
  sampleText(preview.settingsFor(ALL_FONTS_VIEW), props.face, preview.pangramText),
);
const font = computed(() =>
  family.value ? `"${family.value}", var(--fm-preview-fallback, system-ui), sans-serif` : undefined,
);
const alphabet = `${LETTERS}  ${NUMERALS} ${SYMBOLS}`;
const SIZES = [12, 14, 18, 24, 36];

// The list cursor follows the arrows; the sheet shows whatever face it lands on.
watch(
  () => fontList.cursorFace,
  (f) => {
    if (f && f.id !== props.face.id) quickLook.open(f);
  },
);

onMounted(async () => {
  returnFocus = sheetReturnTarget();
  document.addEventListener('focusin', onFocusIn);
  await nextTick();
  focusClose();
});
onBeforeUnmount(() => {
  document.removeEventListener('focusin', onFocusIn);
  if (held !== null) loader.release(held);
  if (returnFocus?.isConnected) returnFocus.focus();
});

const STEP: Record<string, number> = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 };

/** Space and Escape close; arrows move to the next or previous font; Tab stays on Close; plain keys stay out of the list's shortcuts. */
function onKeyDown(e: KeyboardEvent): void {
  const space = e.key === ' ' && e.target === dialog.value;
  if (e.key === 'Escape' || space) {
    e.preventDefault();
    e.stopPropagation();
    quickLook.close();
    return;
  }
  const step = STEP[e.key];
  if (step && !e.ctrlKey && !e.metaKey && !e.altKey) {
    e.preventDefault();
    e.stopPropagation();
    fontList.moveBy(step);
    return;
  }
  if (!e.ctrlKey && !e.metaKey) e.stopPropagation();
  if (e.key === 'Tab') {
    e.preventDefault();
    focusClose();
  }
}

/** Focus that moves to the page behind the sheet is taken back to Close. */
function onFocusIn(e: FocusEvent): void {
  const target = e.target;
  if (!dialog.value || !(target instanceof Element) || dialog.value.contains(target)) return;
  if (target.closest('[role="dialog"]')) return;
  focusClose();
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
        <UiIconButton
          ref="closeButton"
          icon="close"
          :label="t('common.close')"
          @click="quickLook.close"
        />
      </header>
      <div class="body" :style="{ fontFamily: font }">
        <p class="sample" dir="auto">{{ text }}</p>
        <p class="alphabet" dir="auto">{{ alphabet }}</p>
        <ul class="ladder">
          <li v-for="size in SIZES" :key="size" dir="auto">
            <span class="size" aria-hidden="true">{{ size }}</span>
            <span class="line" :style="{ fontSize: size + 'px' }">{{ name }}</span>
          </li>
        </ul>
        <p class="paragraph" dir="auto">{{ LOREM }}</p>
      </div>
      <p id="fm-quick-look-hint" class="hint">{{ t('quickLook.hint') }}</p>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: var(--fm-z-popover);
  background: var(--fm-scrim);
}
.sheet {
  display: flex;
  flex-direction: column;
  gap: 16px;
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  padding: 16px 48px 12px;
  background-color: var(--fm-bg);
  color: var(--fm-fg);
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
  min-width: 0;
  margin: 0;
  font-size: var(--fm-text-md);
  font-weight: 500;
}
.body {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: 24px;
}
.sample {
  margin: 0;
  font-size: clamp(48px, 7vw, 104px);
  line-height: 1.1;
  overflow-wrap: anywhere;
}
.alphabet {
  margin: 0;
  font-size: 28px;
  line-height: 1.3;
  color: var(--fm-muted);
  overflow-wrap: anywhere;
}
.ladder {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 0;
  padding: 16px 0;
  border-block: 1px solid var(--fm-border);
  list-style: none;
}
.ladder li {
  display: flex;
  align-items: baseline;
  gap: 16px;
}
.size {
  flex: none;
  width: 24px;
  color: var(--fm-muted);
  font-family: var(--fm-font-ui, inherit);
  font-size: var(--fm-text-xs);
  font-variant-numeric: tabular-nums;
}
.line {
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.paragraph {
  max-width: 64ch;
  margin: 0;
  font-size: 16px;
  line-height: 1.6;
}
.hint {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
@media (prefers-reduced-motion: no-preference) {
  .sheet {
    animation: rise var(--fm-motion-move) var(--fm-ease);
  }
}
@keyframes rise {
  from {
    opacity: 0;
  }
}
</style>
