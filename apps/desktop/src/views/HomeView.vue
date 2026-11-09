<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import ActivationNotice from '../components/ActivationNotice.vue';
import AdobeFontsNote from '../components/AdobeFontsNote.vue';
import CatalogBrowser from '../components/CatalogBrowser.vue';
import CollectionHeader from '../components/CollectionHeader.vue';
import CollectionsSidebar from '../components/CollectionsSidebar.vue';
import CompareSheet from '../components/CompareSheet.vue';
import DocumentFontsPanel from '../components/DocumentFontsPanel.vue';
import DragGhost from '../components/DragGhost.vue';
import FilterChips from '../components/FilterChips.vue';
import FontGrid from '../components/FontGrid.vue';
import FontLayout from '../components/FontLayout.vue';
import LayoutActions from '../components/LayoutActions.vue';
import FontList from '../components/FontList.vue';
import FontTable from '../components/FontTable.vue';
import FontWaterfall from '../components/FontWaterfall.vue';
import WallCountLine from '../components/WallCountLine.vue';
import LibraryViewsSidebar from '../components/LibraryViewsSidebar.vue';
import UiButton from '../components/ui/UiButton.vue';
import UiEmptyState from '../components/ui/UiEmptyState.vue';
import UiIconButton from '../components/ui/UiIconButton.vue';
import { hasChosenFilters } from '../search/chips';
import { useSourcesStore } from '../stores/sources';
import UiSegmentedControl from '../components/ui/UiSegmentedControl.vue';
import MainLayout from '../components/MainLayout.vue';
import OnlineCatalogsSidebar from '../components/OnlineCatalogsSidebar.vue';
import PreviewControls from '../components/PreviewControls.vue';
import PreviewSizeSlider from '../components/PreviewSizeSlider.vue';
import PreviewStrip from '../components/PreviewStrip.vue';
import UiSelect from '../components/ui/UiSelect.vue';
import ProblemsSidebar from '../components/ProblemsSidebar.vue';
import ProjectSetsSidebar from '../components/ProjectSetsSidebar.vue';
import QuickLookSheet from '../components/QuickLookSheet.vue';
import QuickCollectionReview from '../components/QuickCollectionReview.vue';
import QuickCollectionSidebar from '../components/QuickCollectionSidebar.vue';
import SearchBox from '../components/SearchBox.vue';
import SelectionBar from '../components/SelectionBar.vue';
import SimilarPanel from '../components/SimilarPanel.vue';
import SourcesSidebar from '../components/SourcesSidebar.vue';
import TableColumnsMenu from '../components/TableColumnsMenu.vue';
import SmartRuleEditor from '../components/SmartRuleEditor.vue';
import TaggingPanel from '../components/TaggingPanel.vue';
import TagsSidebar from '../components/TagsSidebar.vue';
import { errorMessage, t } from '../i18n';
import { DENSITIES } from '../list/density';
import { WALL_SIZE, wallLayout } from '../list/grid';
import FmFade from '../motion/FmFade.vue';
import FmSlide from '../motion/FmSlide.vue';
import type { ListRow } from '../list/rows';
import { ACTIONS, ariaShortcut, chordOf, formatChord } from '../keyboard/keymap';
import { useActivationStore } from '../stores/activation';
import { useAppStore } from '../stores/app';
import { useCollectionsStore } from '../stores/collections';
import { MAX_PINS, useCompareStore } from '../stores/compare';
import { useActions, useCommandsStore } from '../stores/commands';
import { useFamilyPageStore } from '../stores/familyPage';
import { useFontListStore } from '../stores/fontList';
import { useLayoutStore } from '../stores/layout';
import { useLibraryStore } from '../stores/library';
import { useLibraryViewsStore } from '../stores/libraryViews';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import { useQuickCollectionStore } from '../stores/quickCollection';
import { useQuickLookStore } from '../stores/quickLook';
import { useCatalogsStore } from '../stores/catalogs';
import { useSearchStore } from '../stores/search';
import { useSetsStore } from '../stores/sets';
import { useSettingsStore } from '../stores/settings';
import { useSimilarStore } from '../stores/similar';
import { useTaggingStore } from '../stores/tagging';
import type { ActivationMode, FaceId, LayoutPreset } from '../api/types';
import { parseFaceId, useSelectionStore } from '../stores/selection';
import { useTextLayoutsStore } from '../stores/textLayouts';
import { LAYOUT_PRESETS, PRESET_LABELS, useViewsStore, type ViewMode } from '../stores/views';
import { useFamilyNavigation } from './useFamilyNavigation';
import { useLiveLibrary } from './useLiveLibrary';

const app = useAppStore();
const layout = useLayoutStore();
const selection = useSelectionStore();
const commands = useCommandsStore();
const library = useLibraryStore();
const activation = useActivationStore();
const preview = usePreviewStore();
const fontList = useFontListStore();
const views = useViewsStore();
const search = useSearchStore();
const collections = useCollectionsStore();
const libraryViews = useLibraryViewsStore();
const sources = useSourcesStore();
/** An open collection with nothing in it and nothing typed or chosen on top of it. */
const emptyCollection = computed(
  () =>
    collections.open !== null && search.query.trim() === '' && !hasChosenFilters(search.filters),
);
const sets = useSetsStore();
const textLayouts = useTextLayoutsStore();
const compare = useCompareStore();
const quickLook = useQuickLookStore();
const settings = useSettingsStore();
const densityLabel = computed(() => t(`home.density.${settings.density}`));
const quick = useQuickCollectionStore();
const catalogs = useCatalogsStore();
const tagging = useTaggingStore();
const similar = useSimilarStore();
const route = useRoute();
const router = useRouter();

// The table's sort column orders the families (behind the API); set before the
// first load so the families come in that order at once.
watch(
  () => views.orderFor(ALL_FONTS_VIEW),
  (order) => void library.setOrder(order),
  { immediate: true },
);
const mode = computed(() => views.modeFor(ALL_FONTS_VIEW));
/**
 * The wall shows its reading rows (E43.1): the Paragraph preset, or your own long text. The
 * switch is a view switch of the motion table: the wall and the rows crossfade.
 */
const wallRows = computed(() => {
  const s = preview.settingsFor(ALL_FONTS_VIEW);
  return mode.value === 'grid' && wallLayout(s.preset, s.sample, s.customText) === 'paragraph';
});
/**
 * A crossfade, not a fade out and in: the leaving list keeps its place over the new one
 * while both fade, so the switch takes the table's 200 ms once. The leaving list is gone for
 * assistive tech and the keyboard at once.
 */
function pinLeaving(el: Element): void {
  const e = el as HTMLElement;
  const { offsetTop, offsetLeft, offsetWidth, offsetHeight } = e;
  Object.assign(e.style, {
    position: 'absolute',
    top: `${offsetTop}px`,
    left: `${offsetLeft}px`,
    width: `${offsetWidth}px`,
    height: `${offsetHeight}px`,
    pointerEvents: 'none',
  });
  // Only the new list is there for a screen reader or a key.
  e.setAttribute('aria-hidden', 'true');
  e.inert = true;
}
// The label of each is `home.modes.<mode>`.
const MODES: readonly ViewMode[] = ['list', 'table', 'grid', 'waterfall', 'layout'];
const modeOptions = computed(() =>
  MODES.map((m) => ({ value: m, label: t(`home.modes.${m}`), icon: m })),
);

// The layout mode sets the selected face (else the list cursor's, else the first
// family's) in the picked layout file, with the editor's unsaved changes.
const layoutId = computed(() => views.layoutFor(ALL_FONTS_VIEW));
const shownLayout = computed(() => textLayouts.draft);
// Before the list arrives the picker offers the presets, so the pick shows at once.
const layoutOptions = computed(() =>
  textLayouts.entries.length
    ? textLayouts.entries
    : LAYOUT_PRESETS.map((p) => ({ id: p, name: PRESET_LABELS[p], preset: p, broken: null })),
);
/** The picker's options: an unreadable layout is listed, named so, and cannot be chosen. */
const presetOptions = computed(() =>
  layoutOptions.value.map((e) => ({
    value: e.id,
    label: e.broken === null ? e.name : t('home.unreadable', { name: e.name }),
    disabled: e.broken !== null,
    title: e.broken ?? undefined,
  })),
);
const layoutFace = computed(() => {
  const first = fontList.families[0];
  return (
    selection.detail?.face ??
    fontList.cursorFace ??
    (first ? fontList.previewFace(first.id) : undefined) ??
    null
  );
});
// With no list on screen nothing else loads the styles of the fallback family.
const layoutFamily = computed(
  () => fontList.cursorRow?.family.id ?? fontList.families[0]?.id ?? null,
);
watch(
  [mode, layoutFamily],
  ([m, family]) => {
    if (m !== 'layout') return;
    if (family !== null) void library.facesOf(family).catch(() => undefined);
  },
  { immediate: true },
);
// Entering the layout mode reads the layouts folder again (a file edited by hand shows
// its new name); picking a layout opens its file.
watch(
  mode,
  (m) => {
    if (m === 'layout') {
      void textLayouts.loadList();
      void textLayouts.loadBounds();
    }
  },
  { immediate: true },
);
watch(
  [mode, layoutId],
  ([m, id]) => {
    if (m === 'layout' && textLayouts.saved?.id !== id) void textLayouts.open(id);
  },
  { immediate: true },
);

function showPreset(p: LayoutPreset): void {
  views.setPreset(ALL_FONTS_VIEW, p);
  views.setMode(ALL_FONTS_VIEW, 'layout');
}

// Pairing Studio (E24.1) opens the picked layout with the face the layout mode shows.
const canOpenStudio = computed(() => mode.value === 'layout' && layoutFace.value !== null);
function openStudio(): void {
  const face = layoutFace.value;
  if (!face) return;
  void router.push({
    name: 'studio',
    query: { layout: layoutId.value, face: String(face.id) },
  });
}

useLiveLibrary('family');
onMounted(() => {
  void app.load();
  void search.loadLanguages();
  void sets.load();
  void quick.load();
  void compare.loadSaved();
});

// The family page (E42.1, E41.5): Enter, a click on a name, O, the palette or a double-click on
// a row or card, in any view. The Layout mode has no rows; it opens the family it shows.
const familyNav = useFamilyNavigation();
const familyPage = useFamilyPageStore();
const openableFamily = computed(() => {
  if (quick.open || catalogs.current) return null;
  return mode.value === 'layout' ? layoutFamily.value : (fontList.cursorRow?.family.id ?? null);
});

/** Opens the row's family page, on the style when the row is one. */
async function openRow(row: ListRow): Promise<void> {
  // The click that got here also moved the cursor: its URL change must end before the push,
  // or the router drops the push for the newer navigation.
  await nextTick();
  await faceNavigation;
  await familyNav.open(row.family.id, row.kind === 'face' ? row.face.id : undefined);
}

function onNameClick(e: MouseEvent): void {
  if (e.shiftKey || e.ctrlKey || e.metaKey || e.altKey) return;
  const el = e.target instanceof Element ? e.target : null;
  if (!el?.closest('[data-family-name]')) return;
  const key = el.closest('[data-key]')?.getAttribute('data-key');
  const row = fontList.rows.find((r) => r.key === key);
  if (row) void openRow(row);
}

function onDoubleClick(e: MouseEvent): void {
  const el = e.target instanceof Element ? e.target : null;
  if (!el || el.closest('button, input, a, .twisty, [role="slider"], [contenteditable="true"]'))
    return;
  const key = el.closest('[data-key]')?.getAttribute('data-key');
  const row = fontList.rows.find((r) => r.key === key);
  if (row) void openRow(row);
}

// One style selected: A acts on its face, and the count line shows the new state. Several rows
// or a family: on all of them, with a toast that can undo it. Nothing selected: nothing happens,
// though `?face=` may still name a face: its row is gone, and nothing here would show the change.
const bulkSelection = computed(() => {
  const rows = fontList.selectedRows;
  return rows.length > 1 || rows[0]?.kind === 'family';
});
const canToggle = () => {
  if (!fontList.selectedRows.length) return false;
  return bulkSelection.value ? !activation.busy : selection.canActivate;
};
const toggle = (mode: ActivationMode) =>
  bulkSelection.value
    ? activation.toggleRows(fontList.selectedRows, mode)
    : selection.toggleActivation(mode);

// C pins the selected face to Compare, and S finds the faces similar to it: the
// selected face once its detail has loaded, else the list cursor's (the row just moved
// to, whose detail is still on its way).
const pinTarget = computed(() => {
  const shown = selection.detail?.face;
  if (shown && shown.id === selection.faceId) return shown;
  return fontList.cursorFace ?? shown ?? null;
});

// B adds the selected rows (or the cursor row) to the Quick Collection, or removes them.
const quickTargets = computed(() => {
  if (fontList.selectedRows.length) return fontList.selectedRows;
  return fontList.cursorRow ? [fontList.cursorRow] : [];
});

// Getters: the palette shows the titles in the language chosen now.
useActions([
  {
    id: ACTIONS.quickCollection,
    get title() {
      return t('home.actions.quickToggle');
    },
    get group() {
      return t('palette.groups.quickCollection');
    },
    enabled: () => !quick.open && quickTargets.value.length > 0,
    run: () => quick.toggleRows(quickTargets.value),
  },
  {
    id: 'quickCollection.open',
    get title() {
      return t('home.actions.quickOpen');
    },
    get group() {
      return t('palette.groups.quickCollection');
    },
    enabled: () => !quick.open,
    run: () => quick.show(),
  },
  {
    id: ACTIONS.tag,
    get title() {
      return t('home.actions.tag');
    },
    get group() {
      return t('palette.groups.tags');
    },
    enabled: () =>
      !quick.open &&
      !tagging.open &&
      tagging.selectionPick.faces.length + tagging.selectionPick.families.length > 0,
    run: () => tagging.openFor(),
  },
  {
    id: ACTIONS.similar,
    get title() {
      return t('home.actions.findSimilar');
    },
    get group() {
      return t('palette.groups.search');
    },
    enabled: () => pinTarget.value !== null,
    run: () => (pinTarget.value ? similar.openFor(pinTarget.value) : undefined),
  },
  {
    id: ACTIONS.openFamily,
    get title() {
      return t('home.actions.openFamily');
    },
    get group() {
      return t('palette.groups.view');
    },
    enabled: () => openableFamily.value !== null,
    run: () => {
      const cursor = fontList.cursorRow;
      if (mode.value !== 'layout' && cursor) return openRow(cursor);
      return openableFamily.value !== null ? familyNav.open(openableFamily.value) : undefined;
    },
  },
  {
    id: ACTIONS.quickLook,
    get title() {
      return t('home.actions.quickLook');
    },
    get group() {
      return t('palette.groups.view');
    },
    enabled: () => pinTarget.value !== null && !compare.open,
    run: () => (pinTarget.value ? quickLook.toggle(pinTarget.value) : undefined),
  },
  {
    id: ACTIONS.compare,
    get title() {
      return t('home.actions.pin');
    },
    get group() {
      return t('palette.groups.compare');
    },
    enabled: () => pinTarget.value !== null && !compare.open,
    run: () => (pinTarget.value ? compare.pin(pinTarget.value) : undefined),
  },
  {
    id: 'compare.open',
    get title() {
      return t('home.actions.openCompare');
    },
    get group() {
      return t('palette.groups.compare');
    },
    enabled: () => compare.canOpen,
    run: () => compare.openSheet(),
  },
  {
    id: ACTIONS.activateSession,
    get title() {
      return t('home.actions.activateSession');
    },
    get group() {
      return t('palette.groups.activation');
    },
    enabled: canToggle,
    run: () => toggle('session'),
  },
  {
    id: ACTIONS.activatePermanent,
    get title() {
      return t('home.actions.activatePermanent');
    },
    get group() {
      return t('palette.groups.activation');
    },
    enabled: canToggle,
    run: () => toggle('permanent'),
  },
  {
    id: 'activation.deactivateAll',
    get title() {
      return t('home.actions.deactivateAll');
    },
    get group() {
      return t('palette.groups.activation');
    },
    enabled: () => activation.count > 0 && !activation.busy,
    run: () => activation.deactivateAll(),
  },
  {
    id: 'activation.deactivateOutsideSet',
    get title() {
      return t('home.actions.deactivateOutsideSet');
    },
    get group() {
      return t('palette.groups.activation');
    },
    // The current set is the one picked under Project sets in the sidebar.
    enabled: () => sets.current != null && activation.count > 0 && !activation.busy,
    run: () => (sets.current ? activation.deactivateAllExcept(sets.current.id) : undefined),
  },
  // Keys 1–5 switch the view mode, in the order of the toolbar buttons.
  ...MODES.map((m, i) => ({
    id: ACTIONS.viewMode((i + 1) as 1 | 2 | 3 | 4 | 5),
    get title() {
      return t(`home.actions.showAs.${m}`);
    },
    get group() {
      return t('palette.groups.view');
    },
    run: () => views.setMode(ALL_FONTS_VIEW, m),
  })),
  // The row density of the list and the table: the same setting as in Settings.
  ...DENSITIES.map((d) => ({
    id: `view.density.${d.value}`,
    get title() {
      return t(`home.actions.density.${d.value}`);
    },
    get group() {
      return t('palette.groups.view');
    },
    enabled: () => settings.density !== d.value,
    run: () => settings.setDensity(d.value),
  })),
  ...LAYOUT_PRESETS.map((p) => ({
    id: `view.layoutPreset.${p}`,
    get title() {
      return t('home.actions.showPreset', { preset: PRESET_LABELS[p] });
    },
    get group() {
      return t('palette.groups.view');
    },
    run: () => showPreset(p),
  })),
  {
    id: 'view.openStudio',
    get title() {
      return t('home.actions.openStudio');
    },
    get group() {
      return t('palette.groups.view');
    },
    enabled: () => canOpenStudio.value,
    run: openStudio,
  },
]);

// One palette entry per saved comparison (E13.2), kept in step with the saved list.
let unregisterSaved: (() => void)[] = [];
watch(
  () => compare.saved,
  (saved) => {
    unregisterSaved.forEach((u) => u());
    unregisterSaved = saved.map((c) =>
      commands.register({
        id: `compare.reopen.${c.id}`,
        get title() {
          return t('home.actions.reopenComparison', { name: c.name });
        },
        get group() {
          return t('palette.groups.compare');
        },
        run: () => compare.reopen(c.id),
      }),
    );
  },
  { immediate: true },
);
onBeforeUnmount(() => unregisterSaved.splice(0).forEach((u) => u()));

const settingsChord = chordOf(ACTIONS.settings)!;
const compareChord = chordOf(ACTIONS.compare)!;

// The selected face is in the URL (`#/?face=3`) so a reload keeps it; the
// font list selects by changing the query.
watch(
  () => route.query.face,
  (face) => selection.select(parseFaceId(face)),
  { immediate: true },
);

// A face that arrives through the URL (a reload, a link) has no row selected yet:
// once its detail and the family list are in, the list selects it (once).
let revealedFromUrl = false;
watch(
  () => [selection.detail?.face, fontList.families.length] as const,
  ([face, count]) => {
    if (revealedFromUrl || !face || face.id !== selection.faceId || count === 0) return;
    revealedFromUrl = true;
    // A row the user already moved to wins over the URL.
    if (fontList.cursor === null) void fontList.reveal(face);
  },
  { immediate: true },
);

/** The newest change of `?face=`, so a push can wait for it. */
let faceNavigation: Promise<unknown> = Promise.resolve();

/** The face under the list's cursor goes to the URL, and from there to the selection store. */
function showFace(face: FaceId): void {
  if (parseFaceId(route.query.face) === face) return;
  faceNavigation = router.replace({ query: { ...route.query, face: String(face) } });
}

const browse = ref<HTMLElement | null>(null);

// The Layout view scrolls by itself, without the rows' anchor: its place goes to the store's
// held scroll as it leaves for a family page (E42.1). It has no lines, so 0 keeps it apart
// from a view's place. The place comes back on mount and again once the font has set the
// text's height, which can be too short for it before that.
const LAYOUT_LINE = 0;
const layoutViewport = () => browse.value?.querySelector<HTMLElement>('.layout') ?? null;
let layoutTop = fontList.heldScrollTop(LAYOUT_LINE, LAYOUT_LINE);
if (layoutTop !== null) fontList.dropHeldScroll();

function restoreLayoutScroll(): void {
  const el = layoutViewport();
  if (el && layoutTop !== null) el.scrollTop = layoutTop;
}
function layoutSettled(): void {
  restoreLayoutScroll();
  layoutTop = null;
}
onMounted(restoreLayoutScroll);
onBeforeUnmount(() => {
  const el = layoutViewport();
  if (el) fontList.viewLeft(el.scrollTop, LAYOUT_LINE, LAYOUT_LINE);
});

// The selection bar (E44.1) rises on the first selection, over the views with rows.
const showSelectionBar = computed(
  () =>
    fontList.selectedCount > 0 &&
    fontList.families.length > 0 &&
    mode.value !== 'layout' &&
    !quick.open &&
    !catalogs.current,
);

/** Escape in the search box or the preview field: back to the font list the keys drive. */
function focusList(): void {
  browse.value
    ?.querySelector<HTMLElement>('[role="listbox"], [role="tree"], [role="treegrid"]')
    ?.focus({ preventScroll: true });
}

// A panel that came back with the library has no element to give the focus back to.
watch(
  () => similar.open,
  async (open) => {
    if (open) return;
    await nextTick();
    if (document.activeElement === document.body) focusList();
  },
);

/** A face picked outside the rows (a Similar result, a document's font): its family page. */
async function openFace(face: FaceId): Promise<void> {
  const family = await familyPage.familyOf(face);
  if (family !== null) await familyNav.open(family, face);
}
</script>

<template>
  <MainLayout>
    <template #sidebar>
      <LibraryViewsSidebar />
      <QuickCollectionSidebar />
      <CollectionsSidebar />
      <TagsSidebar />
      <ProjectSetsSidebar />
      <SourcesSidebar />
      <OnlineCatalogsSidebar />
      <ProblemsSidebar />
    </template>

    <template #list>
      <AdobeFontsNote v-if="catalogs.open === 'adobeFonts'" :level="1" />
      <CatalogBrowser v-else-if="catalogs.current" />
      <QuickCollectionReview
        v-else-if="quick.open"
        :preview="preview.settingsFor(ALL_FONTS_VIEW)"
      />
      <template v-else>
        <header class="toolbar">
          <UiIconButton
            icon="sidebar"
            :label="t('home.sidebar')"
            aria-controls="fm-sidebar"
            aria-keyshortcuts="["
            :aria-pressed="layout.sidebarOpen"
            :title="t('home.sidebarTitle')"
            @click="layout.toggleSidebar"
          />
          <h1 class="title">{{ collections.open?.name ?? libraryViews.currentLabel }}</h1>
          <SearchBox @leave="focusList" />
          <FilterChips v-show="!similar.open" class="filters" />
          <UiSegmentedControl
            :model-value="mode"
            :options="modeOptions"
            :label="t('home.viewMode')"
            slide
            @update:model-value="views.setMode(ALL_FONTS_VIEW, $event)"
          />
          <TableColumnsMenu v-if="mode === 'table'" :view="ALL_FONTS_VIEW" />
          <label v-if="mode === 'layout'" class="preset">
            {{ t('home.preset') }}
            <UiSelect
              :model-value="layoutId"
              :options="presetOptions"
              @update:model-value="views.setLayout(ALL_FONTS_VIEW, $event)"
            />
          </label>
          <!-- After the controls of the view mode, so Tab from the modes reaches those first. -->
          <UiIconButton
            icon="density"
            :label="t('home.compactRows')"
            :aria-pressed="settings.density === 'compact'"
            :title="t('home.densityTitle', { density: densityLabel })"
            @click="settings.setDensity(settings.density === 'compact' ? 'comfortable' : 'compact')"
          />
          <UiIconButton
            icon="compare"
            :label="t('home.compareButton', { pinned: compare.pins.length, max: MAX_PINS })"
            :disabled="!compare.canOpen"
            :title="t('home.compareTitle', { chord: formatChord(compareChord) })"
            @click="compare.openSheet"
          >
            <span class="pins">{{ compare.pins.length }}</span>
          </UiIconButton>
          <PreviewSizeSlider
            :view="ALL_FONTS_VIEW"
            :size-range="mode === 'grid' ? WALL_SIZE : undefined"
          />
        </header>
        <div
          v-show="!similar.open"
          ref="browse"
          class="browse"
          @click="onNameClick"
          @dblclick="onDoubleClick"
        >
          <CollectionHeader />
          <PreviewControls
            :view="ALL_FONTS_VIEW"
            :size-range="mode === 'grid' ? WALL_SIZE : undefined"
            :font-count="search.total"
            @leave="focusList"
          />
          <WallCountLine :view="ALL_FONTS_VIEW" :rows="wallRows" />
          <ActivationNotice />
          <!-- One fade over every view mode (E46.2): switching crossfades the two lists. The
               leaving one is pinned where it was, so the new one takes its place at once. Inside
               the families test, so the first list of a start does not fade in. -->
          <FmFade v-if="fontList.families.length" @before-leave="pinLeaving">
            <div v-if="mode === 'table'" key="table" class="pane">
              <PreviewStrip :view="ALL_FONTS_VIEW" :preview="preview.settingsFor(ALL_FONTS_VIEW)" />
              <FontTable :view="ALL_FONTS_VIEW" @cursor-face="showFace" @open="openRow" />
            </div>
            <div v-else-if="mode === 'layout'" key="layout" class="pane">
              <LayoutActions @opened="(id) => views.setLayout(ALL_FONTS_VIEW, id)">
                <UiButton
                  class="studio"
                  :disabled="!canOpenStudio"
                  :title="t('home.openStudioTitle')"
                  @click="openStudio"
                >
                  {{ t('home.openStudio') }}
                </UiButton>
              </LayoutActions>
              <FontLayout
                :layout="shownLayout"
                :face="layoutFace"
                editable
                :bounds="textLayouts.bounds"
                :set-props="textLayouts.setBlockProps"
                :width="textLayouts.width"
                :set-width-lock="textLayouts.setWidthLock"
                @edit-text="textLayouts.setBlockText"
                @settled="layoutSettled"
              />
            </div>
            <FontGrid
              v-else-if="mode === 'grid'"
              :key="wallRows ? 'grid-rows' : 'grid-wall'"
              :preview="preview.settingsFor(ALL_FONTS_VIEW)"
              @cursor-face="showFace"
              @open="openRow"
            />
            <FontWaterfall
              v-else-if="mode === 'waterfall'"
              key="waterfall"
              :preview="preview.settingsFor(ALL_FONTS_VIEW)"
              @cursor-face="showFace"
              @open="openRow"
            />
            <FontList
              v-else
              key="list"
              :preview="preview.settingsFor(ALL_FONTS_VIEW)"
              @cursor-face="showFace"
              @open="openRow"
            />
          </FmFade>
          <section v-else class="empty" :aria-label="t('home.fontList')">
            <p v-if="library.error" role="alert" class="error">{{ errorMessage(library.error) }}</p>
            <UiEmptyState
              v-else-if="search.hits && emptyCollection"
              :title="t('home.emptyCollection.title')"
              :body="t('home.emptyCollection.body')"
            >
              <button type="button" data-testid="empty-action" @click="collections.show(null)">
                {{ t('home.emptyCollection.action') }}
              </button>
            </UiEmptyState>
            <UiEmptyState
              v-else-if="search.hits"
              :title="t('home.noMatchesTitle')"
              :body="t('home.noMatches')"
            >
              <button type="button" data-testid="empty-action" @click="search.clear()">
                {{ t('home.noMatchesAction') }}
              </button>
            </UiEmptyState>
            <UiEmptyState
              v-else-if="library.loaded"
              :title="t('home.emptyLibrary.title')"
              :body="t('home.emptyLibrary.body')"
            >
              <button type="button" data-testid="empty-action" @click="sources.add()">
                {{ t('home.emptyLibrary.action') }}
              </button>
            </UiEmptyState>
          </section>
          <FmSlide from="bottom" distance="16px">
            <SelectionBar v-if="showSelectionBar" @cleared="focusList" />
          </FmSlide>
          <DragGhost />
        </div>
      </template>
      <SimilarPanel :preview="preview.settingsFor(ALL_FONTS_VIEW)" @select="openFace" />
      <DocumentFontsPanel @select="openFace" />
      <CompareSheet v-if="compare.open" />
      <QuickLookSheet v-if="quickLook.face" :face="quickLook.face" />
      <TaggingPanel />
      <SmartRuleEditor />
    </template>

    <template #status>
      <p class="active-count" data-testid="active-count" role="status">
        {{ t('home.fontsActive', { count: activation.count }) }}
      </p>
      <RouterLink :to="{ name: 'conflicts' }" class="hint">{{ t('home.conflicts') }}</RouterLink>
      <p v-if="app.error" role="alert" class="error">{{ errorMessage(app.error) }}</p>
      <span class="status-end">
        <RouterLink
          :to="{ name: 'settings' }"
          class="hint settings-link"
          data-testid="status-settings"
          :title="t('home.settingsTitle', { chord: formatChord(settingsChord) })"
          :aria-keyshortcuts="ariaShortcut(settingsChord)"
        >
          {{ t('home.settings') }} <kbd>{{ formatChord(settingsChord) }}</kbd>
        </RouterLink>
      </span>
    </template>
  </MainLayout>
</template>

<style scoped>
.browse {
  position: relative;
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}
/* The table and the layout mode have two parts each: one box, so the fade has one child. It
   lays them out as the browse column does. */
.pane {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}
/* A name in a row or card opens the family page. */
.browse :deep([data-family-name]) {
  cursor: pointer;
}
.browse :deep([data-family-name]:hover) {
  text-decoration: underline;
}
.toolbar {
  /* A stacking level of its own, above the view below: the Columns menu opens over it. */
  position: relative;
  z-index: 1;
  container-type: inline-size;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-1);
  min-height: 48px;
  padding: var(--fm-space-2) var(--fm-space-3);
  border-bottom: 1px solid var(--fm-border);
}
/* The rest of the bar: Filters and its chips, one row. */
.filters {
  flex: 1 1 0;
  min-width: 120px;
}
/* A column too narrow for both: the filters take a row of their own under the controls. */
@container (max-width: 960px) {
  .filters {
    order: 1;
    flex-basis: 100%;
  }
  .title {
    flex: 1;
  }
}
.title {
  margin: 0;
  font-size: var(--fm-text-md);
  font-weight: 600;
  white-space: nowrap;
}
/* In the layout file bar: at its own end. */
.studio {
  margin-inline-start: auto;
}
.pins {
  font-size: var(--fm-text-xs);
  font-variant-numeric: tabular-nums;
}
.preset {
  display: flex;
  align-items: center;
  gap: 4px;
}
.empty {
  display: grid;
  flex: 1;
  place-items: center;
  color: var(--fm-muted);
}
.hint {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-1);
  min-height: 0;
  padding: 0;
  border: 0;
  background-color: transparent;
  color: inherit;
  font: inherit;
  text-decoration: none;
}
/* Settings reads as a link: the full text colour (theme.css calls it --fm-fg), not
   the muted status-bar colour, underlined on hover and focus. */
.settings-link {
  color: var(--fm-fg);
  cursor: pointer;
}
.settings-link:hover,
.settings-link:focus-visible {
  text-decoration: underline;
}
.active-count {
  margin: 0;
}
.status-end {
  display: flex;
  align-items: center;
  gap: var(--fm-space-4);
  margin-inline-start: auto;
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
</style>
