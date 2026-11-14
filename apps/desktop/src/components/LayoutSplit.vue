<script setup lang="ts">
// The Layout mode as a split (E50.3, board 9-split): the font list at the inline start, the
// layout pane beside it. The list is 560 px by default; the divider drags it between 400 and
// 760 px (arrow keys too), and the width is a view preference in the layout store.
//
// Below 1100 px of window the split collapses to the pane. A Fonts button at the start of the
// pane's head slides the list in over the pane (`FmSlide`, the drawer's motion); the button
// again, or Escape in the list, slides it out and gives the focus back to the button.
//
// The parent fills three slots: `list` (the font list), `head` (the pane's controls, after the
// Fonts button) and the default slot (the pane's body). No font logic lives here.
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { t } from '../i18n';
import FmSlide from '../motion/FmSlide.vue';
import { SPLIT_COLLAPSE_PX, SPLIT_LIST, useLayoutStore } from '../stores/layout';
import PaneDivider from './PaneDivider.vue';
import UiButton from './ui/UiButton.vue';

const layout = useLayoutStore();

const LIST_ID = 'fm-split-list';
const QUERY = `(max-width: ${SPLIT_COLLAPSE_PX - 0.02}px)`;

/** The window is narrower than the split needs: the pane only, the list on demand. */
const narrow = ref(false);
/** The list is slid in over the pane (narrow only). */
const listOpen = ref(false);
const fontsButton = ref<InstanceType<typeof UiButton> | null>(null);
const listEl = ref<HTMLElement | null>(null);

const media = typeof window !== 'undefined' ? window.matchMedia?.(QUERY) : undefined;
function onMedia(): void {
  narrow.value = media?.matches ?? false;
}
onMedia();
media?.addEventListener('change', onMedia);
onBeforeUnmount(() => media?.removeEventListener('change', onMedia));

// A window that widens again shows the split; the slid-in state does not outlive it.
watch(narrow, (n) => {
  if (!n) listOpen.value = false;
});

/** The font view inside the list column: the one the keys drive. */
const FONT_VIEW = '[role="tree"], [role="treegrid"], [role="listbox"]';

async function toggleList(): Promise<void> {
  listOpen.value = !listOpen.value;
  if (!listOpen.value) return;
  await nextTick();
  listEl.value?.querySelector<HTMLElement>(FONT_VIEW)?.focus({ preventScroll: true });
}

function closeList(): void {
  if (!narrow.value || !listOpen.value) return;
  listOpen.value = false;
  void nextTick(() => (fontsButton.value?.$el as HTMLElement | undefined)?.focus());
}

function onListKey(e: KeyboardEvent): void {
  if (e.key !== 'Escape' || !narrow.value || !listOpen.value) return;
  e.preventDefault();
  e.stopPropagation();
  closeList();
}
</script>

<template>
  <div class="split" :class="{ narrow }" data-testid="layout-split">
    <FmSlide v-if="narrow" from="start" distance="100%" reduced="fade">
      <section
        v-if="listOpen"
        :id="LIST_ID"
        ref="listEl"
        class="split-list overlay"
        :aria-label="t('home.split.list')"
        data-testid="split-list"
        :style="{ width: `min(${layout.splitListWidth}px, 100%)` }"
        @keydown="onListKey"
      >
        <slot name="list" />
      </section>
    </FmSlide>
    <template v-else>
      <section
        :id="LIST_ID"
        ref="listEl"
        class="split-list"
        :aria-label="t('home.split.list')"
        data-testid="split-list"
        :style="{ width: `${layout.splitListWidth}px` }"
      >
        <slot name="list" />
      </section>
      <PaneDivider
        :label="t('home.split.resize')"
        :controls="LIST_ID"
        :value="layout.splitListWidth"
        :min="SPLIT_LIST.min"
        :max="SPLIT_LIST.max"
        :direction="1"
        @resize="layout.setSplitListWidth"
      />
    </template>
    <div class="split-pane" data-testid="split-pane">
      <div class="pane-head" role="toolbar" :aria-label="t('home.split.pane')">
        <UiButton
          v-if="narrow"
          ref="fontsButton"
          class="fonts"
          :aria-expanded="listOpen"
          :aria-controls="LIST_ID"
          :title="t('home.split.fontsTitle')"
          @click="toggleList"
        >
          {{ t('home.split.fonts') }}
        </UiButton>
        <slot name="head" />
      </div>
      <slot />
    </div>
  </div>
</template>

<style scoped>
.split {
  position: relative;
  display: flex;
  flex: 1;
  min-height: 0;
}
.split-list {
  display: flex;
  flex: none;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
/* Narrow: the list lies over the pane from the inline start, as the filters drawer does. */
.split-list.overlay {
  position: absolute;
  inset-block: 0;
  inset-inline-start: 0;
  /* Over the layout's sticky head. */
  z-index: calc(var(--fm-z-sticky) + 1);
  background-color: var(--fm-bg);
  border-inline-end: 1px solid var(--fm-border);
  box-shadow: var(--fm-shadow-2);
}
.split-pane {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
.pane-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2) var(--fm-space-3);
  min-height: 48px;
  padding: var(--fm-space-2) var(--fm-space-4);
  border-bottom: 1px solid var(--fm-border);
}
</style>
