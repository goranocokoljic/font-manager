<script setup lang="ts">
// The one top bar of the library and the online catalogs (E54.5): the sidebar toggle, the
// title, the search, the view's own row (the filter chips; a catalog's count and Refresh),
// the view switch, Compare and the size slider. Every view draws it from here, so switching
// between them moves or resizes no shared control (the E50.2 rule): the title has a fixed
// width in a wide bar, the view's row takes the rest. `bare` (the Adobe Fonts note) keeps the
// toggle and the title only, at the same height: a control with nothing to act on is left out.
import { computed } from 'vue';
import { t } from '../i18n';
import { ACTIONS, chordOf, formatChord } from '../keyboard/keymap';
import type { PreviewSurface } from '../preview/applicability';
import { MAX_PINS, useCompareStore } from '../stores/compare';
import { useLayoutStore } from '../stores/layout';
import type { ViewMode } from '../stores/views';
import PreviewSizeSlider from './PreviewSizeSlider.vue';
import UiIconButton from './ui/UiIconButton.vue';
import UiSegmentedControl, { type SegmentOption } from './ui/UiSegmentedControl.vue';

const props = withDefaults(
  defineProps<{
    title: string;
    /** The heading's id, for a region it labels. */
    titleId?: string;
    /** Only the sidebar toggle and the title (a view with nothing to switch, search or size). */
    bare?: boolean;
    /** The preview view the size slider sets. */
    view?: string;
    mode?: ViewMode;
    modes?: readonly SegmentOption<ViewMode>[];
    /** The slider's range, when the view mode has its own (the wall's 24–120). */
    sizeRange?: { readonly min: number; readonly max: number; readonly step: number };
  }>(),
  {
    titleId: undefined,
    bare: false,
    view: '',
    mode: 'list',
    modes: () => [],
    sizeRange: undefined,
  },
);
const emit = defineEmits<{ 'update:mode': [mode: ViewMode] }>();

const layout = useLayoutStore();
const compare = useCompareStore();
const compareChord = chordOf(ACTIONS.compare)!;
const sizeMode = computed<PreviewSurface>(() => props.mode);
</script>

<template>
  <header class="toolbar" data-top-bar>
    <UiIconButton
      icon="sidebar"
      :label="t('home.sidebar')"
      :aria-controls="layout.sidebarShown ? 'fm-sidebar' : undefined"
      aria-keyshortcuts="["
      :aria-pressed="layout.sidebarShown"
      :title="t('home.sidebarTitle')"
      @click="layout.toggleSidebar"
    />
    <h1 :id="titleId" class="title" :title="title">
      <bdi>{{ title }}</bdi>
    </h1>
    <template v-if="!bare">
      <slot name="search" />
      <slot />
      <UiSegmentedControl
        :model-value="mode"
        :options="modes"
        :label="t('home.viewMode')"
        slide
        @update:model-value="emit('update:mode', $event)"
      />
      <!-- The toolbar never changes shape (E50.2): the same controls in every view mode.
           A control of one view lives inside that view (Columns, the layout preset). -->
      <UiIconButton
        icon="compare"
        :label="t('home.compareButton', { pinned: compare.pins.length, max: MAX_PINS })"
        :disabled="!compare.canOpen"
        :title="t('home.compareTitle', { chord: formatChord(compareChord) })"
        @click="compare.openSheet"
      >
        <span class="pins">{{ compare.pins.length }}</span>
      </UiIconButton>
      <PreviewSizeSlider :view="view" :mode="sizeMode" :size-range="sizeRange" />
    </template>
  </header>
</template>

<style scoped>
.toolbar {
  /* A stacking level of its own, above the view below: the filter popover opens over it. */
  position: relative;
  z-index: 1;
  container-type: inline-size;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-1);
  box-sizing: border-box;
  min-height: 48px;
  padding: var(--fm-space-2) var(--fm-space-3);
  border-bottom: 1px solid var(--fm-border);
}
/* One width for the short titles (All fonts, Google Fonts, Fontshare), so the search after it
   stays where it is from view to view; a longer name grows the title up to a limit, then
   shortens with its full text in the tooltip. Kept small: the filter chips need the room. */
.title {
  flex: none;
  box-sizing: border-box;
  min-width: 88px;
  max-width: 200px;
  margin: 0;
  overflow: hidden;
  font-size: var(--fm-text-md);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* The view's own row (the filter chips, a catalog's status): the rest of the bar. */
:slotted(.filters),
:slotted(.fill) {
  flex: 1 1 0;
  min-width: 120px;
}
/* A column too narrow for both: the view's row takes a row of its own under the controls. */
@container (max-width: 960px) {
  :slotted(.filters),
  :slotted(.fill) {
    order: 1;
    flex-basis: 100%;
  }
  .title {
    flex: 1;
    min-width: 0;
    max-width: none;
  }
}
.pins {
  font-size: var(--fm-text-xs);
  font-variant-numeric: tabular-nums;
}
</style>
