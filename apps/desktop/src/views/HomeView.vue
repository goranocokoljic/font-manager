<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, watch } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import { formatApiVersion } from '../api/version';
import AdobeFontsNote from '../components/AdobeFontsNote.vue';
import CatalogBrowser from '../components/CatalogBrowser.vue';
import CollectionsSidebar from '../components/CollectionsSidebar.vue';
import CompareSheet from '../components/CompareSheet.vue';
import DocumentFontsPanel from '../components/DocumentFontsPanel.vue';
import FilterChips from '../components/FilterChips.vue';
import FontGrid from '../components/FontGrid.vue';
import FontLayout from '../components/FontLayout.vue';
import LayoutActions from '../components/LayoutActions.vue';
import FontList from '../components/FontList.vue';
import FontTable from '../components/FontTable.vue';
import FontWaterfall from '../components/FontWaterfall.vue';
import InspectorPanel from '../components/InspectorPanel.vue';
import MainLayout from '../components/MainLayout.vue';
import OnlineCatalogsSidebar from '../components/OnlineCatalogsSidebar.vue';
import PreviewControls from '../components/PreviewControls.vue';
import PreviewStrip from '../components/PreviewStrip.vue';
import ProblemsSidebar from '../components/ProblemsSidebar.vue';
import ProjectSetsSidebar from '../components/ProjectSetsSidebar.vue';
import QuickLookSheet from '../components/QuickLookSheet.vue';
import QuickCollectionReview from '../components/QuickCollectionReview.vue';
import QuickCollectionSidebar from '../components/QuickCollectionSidebar.vue';
import SearchBox from '../components/SearchBox.vue';
import SimilarPanel from '../components/SimilarPanel.vue';
import SourcesSidebar from '../components/SourcesSidebar.vue';
import TableColumnsMenu from '../components/TableColumnsMenu.vue';
import SmartRuleEditor from '../components/SmartRuleEditor.vue';
import TaggingPanel from '../components/TaggingPanel.vue';
import TagsSidebar from '../components/TagsSidebar.vue';
import { errorMessage, t, type MessageKey } from '../i18n';
import { DENSITIES } from '../list/density';
import { ACTIONS, ariaShortcut, chordOf, formatChord } from '../keyboard/keymap';
import { useActivationStore } from '../stores/activation';
import { useAppStore } from '../stores/app';
import { useCollectionsStore } from '../stores/collections';
import { MAX_PINS, useCompareStore } from '../stores/compare';
import { useActions, useCommandsStore } from '../stores/commands';
import { useFontListStore } from '../stores/fontList';
import { useLayoutStore } from '../stores/layout';
import { useLibraryStore } from '../stores/library';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import { useProblemsStore } from '../stores/problems';
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

const app = useAppStore();
const layout = useLayoutStore();
const selection = useSelectionStore();
const commands = useCommandsStore();
const library = useLibraryStore();
const activation = useActivationStore();
const problems = useProblemsStore();
const preview = usePreviewStore();
const fontList = useFontListStore();
const views = useViewsStore();
const search = useSearchStore();
const collections = useCollectionsStore();
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
// The label of each is `home.modes.<mode>`.
const MODES: readonly ViewMode[] = ['list', 'table', 'grid', 'waterfall', 'layout'];

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

onMounted(() => {
  void app.load();
  void library.load();
  void activation.load();
  void problems.load();
  void search.loadLanguages();
  void sets.load();
  void quick.load();
  void compare.loadSaved();
});
onBeforeUnmount(() => {
  void library.stop();
  void activation.stop();
  void problems.stop();
});

// One style (or nothing) selected: A acts on the inspector's face, which shows the
// outcome. Several rows or a family: on all of them, with a toast that can undo it.
const bulkSelection = computed(() => {
  const rows = fontList.selectedRows;
  return rows.length > 1 || rows[0]?.kind === 'family';
});
const canToggle = () => (bulkSelection.value ? !activation.busy : selection.canActivate);
const toggle = (mode: ActivationMode) =>
  bulkSelection.value
    ? activation.toggleRows(fontList.selectedRows, mode)
    : selection.toggleActivation(mode);

// C pins the selected face to Compare, and S finds the faces similar to it: the
// inspector's once its detail has loaded, else the list cursor's (the row just moved
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

const paletteChord = chordOf(ACTIONS.palette)!;
const settingsChord = chordOf(ACTIONS.settings)!;
const compareChord = chordOf(ACTIONS.compare)!;

// The selected face is in the URL (`#/?face=3`) so a reload keeps it; the
// font list selects by changing the query.
watch(
  () => route.query.face,
  (face) => selection.select(parseFaceId(face)),
  { immediate: true },
);

/** The face under the list's cursor goes to the inspector. */
function showFace(face: FaceId): void {
  if (parseFaceId(route.query.face) === face) return;
  void router.replace({ query: { ...route.query, face: String(face) } });
}

/** Enter on a style: show it, opening the inspector when it is hidden. */
function openFace(face: FaceId): void {
  showFace(face);
  if (!layout.inspectorVisible) layout.toggleInspector();
}

// Sidebar groups from the spec's information architecture; later stories fill them.
const groups: MessageKey[] = ['home.sidebarGroups.smart'];
</script>

<template>
  <MainLayout>
    <template #sidebar>
      <SourcesSidebar />
      <QuickCollectionSidebar />
      <CollectionsSidebar />
      <TagsSidebar />
      <ProblemsSidebar />
      <ProjectSetsSidebar />
      <ul class="groups">
        <li v-for="group in groups" :key="group" class="group">{{ t(group) }}</li>
      </ul>
      <OnlineCatalogsSidebar />
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
          <h1 class="title">{{ collections.open?.name ?? t('home.allFonts') }}</h1>
          <SearchBox />
          <p class="count" role="status">
            <template v-if="fontList.selectedCount > 1">{{
              t('home.selected', { count: fontList.selectedCount })
            }}</template>
            <template v-else-if="search.hits">{{
              t('home.families', { count: search.total })
            }}</template>
          </p>
          <div class="modes" role="group" :aria-label="t('home.viewMode')">
            <button
              v-for="m in MODES"
              :key="m"
              type="button"
              class="toggle"
              :aria-pressed="mode === m"
              @click="views.setMode(ALL_FONTS_VIEW, m)"
            >
              {{ t(`home.modes.${m}`) }}
            </button>
          </div>
          <TableColumnsMenu v-if="mode === 'table'" :view="ALL_FONTS_VIEW" />
          <label v-if="mode === 'layout'" class="preset">
            {{ t('home.preset') }}
            <select
              :value="layoutId"
              @change="views.setLayout(ALL_FONTS_VIEW, ($event.target as HTMLSelectElement).value)"
            >
              <option
                v-for="e in layoutOptions"
                :key="e.id"
                :value="e.id"
                :disabled="e.broken !== null"
                :title="e.broken ?? undefined"
              >
                {{ e.broken === null ? e.name : t('home.unreadable', { name: e.name }) }}
              </option>
            </select>
          </label>
          <!-- After the controls of the view mode, so Tab from the modes reaches those first. -->
          <button
            type="button"
            class="toggle"
            :aria-pressed="settings.density === 'compact'"
            :title="t('home.densityTitle', { density: densityLabel })"
            @click="settings.setDensity(settings.density === 'compact' ? 'comfortable' : 'compact')"
          >
            {{ t('home.compactRows') }}
          </button>
          <button
            type="button"
            class="toggle"
            :disabled="!compare.canOpen"
            :title="t('home.compareTitle', { chord: formatChord(compareChord) })"
            @click="compare.openSheet"
          >
            {{ t('home.compareButton', { pinned: compare.pins.length, max: MAX_PINS }) }}
          </button>
          <button
            type="button"
            class="toggle"
            aria-controls="fm-inspector"
            aria-keyshortcuts="Control+I Meta+I"
            :aria-pressed="layout.inspectorVisible"
            :title="t('home.inspectorTitle')"
            @click="layout.toggleInspector"
          >
            {{ t('home.inspector') }}
          </button>
        </header>
        <FilterChips />
        <PreviewControls :view="ALL_FONTS_VIEW" />
        <template v-if="fontList.families.length && mode === 'table'">
          <PreviewStrip :view="ALL_FONTS_VIEW" :preview="preview.settingsFor(ALL_FONTS_VIEW)" />
          <FontTable :view="ALL_FONTS_VIEW" @cursor-face="showFace" @open="openFace" />
        </template>
        <template v-else-if="fontList.families.length && mode === 'layout'">
          <LayoutActions @opened="(id) => views.setLayout(ALL_FONTS_VIEW, id)">
            <button
              type="button"
              class="toggle studio"
              :disabled="!canOpenStudio"
              :title="t('home.openStudioTitle')"
              @click="openStudio"
            >
              {{ t('home.openStudio') }}
            </button>
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
          />
        </template>
        <FontGrid
          v-else-if="fontList.families.length && mode === 'grid'"
          :preview="preview.settingsFor(ALL_FONTS_VIEW)"
          @cursor-face="showFace"
          @open="openFace"
        />
        <FontWaterfall
          v-else-if="fontList.families.length && mode === 'waterfall'"
          :preview="preview.settingsFor(ALL_FONTS_VIEW)"
          @cursor-face="showFace"
          @open="openFace"
        />
        <FontList
          v-else-if="fontList.families.length"
          :preview="preview.settingsFor(ALL_FONTS_VIEW)"
          @cursor-face="showFace"
          @open="openFace"
        />
        <section v-else class="empty" :aria-label="t('home.fontList')">
          <p v-if="library.error" role="alert" class="error">{{ errorMessage(library.error) }}</p>
          <p v-else-if="search.hits">{{ t('home.noMatches') }}</p>
          <p v-else-if="library.loaded">{{ t('home.empty') }}</p>
        </section>
      </template>
      <SimilarPanel :preview="preview.settingsFor(ALL_FONTS_VIEW)" @select="openFace" />
      <DocumentFontsPanel @select="openFace" />
      <CompareSheet v-if="compare.open" />
      <QuickLookSheet v-if="quickLook.face" :key="quickLook.face.id" :face="quickLook.face" />
      <TaggingPanel />
      <SmartRuleEditor />
    </template>

    <template #inspector>
      <InspectorPanel />
    </template>

    <template #status>
      <button
        type="button"
        class="hint"
        :aria-keyshortcuts="ariaShortcut(paletteChord)"
        @click="commands.openPalette"
      >
        {{ t('home.commands') }} <kbd>{{ formatChord(paletteChord) }}</kbd>
      </button>
      <RouterLink
        :to="{ name: 'settings' }"
        class="hint"
        :aria-keyshortcuts="ariaShortcut(settingsChord)"
      >
        {{ t('home.settings') }} <kbd>{{ formatChord(settingsChord) }}</kbd>
      </RouterLink>
      <RouterLink :to="{ name: 'conflicts' }" class="hint">{{ t('home.conflicts') }}</RouterLink>
      <RouterLink :to="{ name: 'problems' }" class="hint" data-testid="problem-count">
        {{ t('home.problems', { count: problems.total }) }}
      </RouterLink>
      <p class="active-count" data-testid="active-count" role="status">
        {{ t('home.fontsActive', { count: activation.count }) }}
      </p>
      <p v-if="app.error" role="alert" class="error">{{ errorMessage(app.error) }}</p>
      <dl v-else-if="app.info" class="info" data-testid="app-info">
        <dt>{{ t('home.info.app') }}</dt>
        <dd>
          <bdi>{{ app.info.name }} {{ app.info.version }}</bdi>
        </dd>
        <dt>{{ t('home.info.api') }}</dt>
        <dd>{{ formatApiVersion(app.info.apiVersion) }}</dd>
        <dt>{{ t('home.info.platform') }}</dt>
        <dd>
          <bdi>{{ app.info.platform }}</bdi>
        </dd>
      </dl>
    </template>
  </MainLayout>
</template>

<style scoped>
.groups {
  margin: 0;
  padding: 8px;
  list-style: none;
}
.group {
  padding: 8px;
  color: var(--fm-muted);
  font-size: 11.5px;
  font-weight: 600;
}
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  height: 48px;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.count {
  flex: 1;
  margin: 0;
  color: var(--fm-muted);
}
.toggle {
  padding: 4px 8px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
/* In the layout file bar: its own end, the bar's size. */
.toggle.studio {
  margin-inline-start: auto;
  padding: 2px 8px;
}
.modes {
  display: flex;
  gap: 4px;
}
.preset {
  display: flex;
  align-items: center;
  gap: 4px;
}
.preset select {
  padding: 2px 4px;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  background: var(--fm-bg);
  color: inherit;
  font: inherit;
}
.toggle:disabled {
  color: var(--fm-muted);
}
.toggle[aria-pressed='true'] {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
.empty {
  display: grid;
  flex: 1;
  place-items: center;
  color: var(--fm-muted);
}
.hint {
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  text-decoration: none;
}
.hint kbd {
  font: inherit;
}
.active-count {
  margin: 0;
}
.info {
  display: flex;
  gap: 4px 8px;
  margin: 0;
}
.info dt::after {
  content: ':';
}
.info dd {
  margin-block: 0;
  margin-inline: 0 8px;
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
</style>
