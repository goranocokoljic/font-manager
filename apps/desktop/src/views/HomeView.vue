<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, watch } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import { formatApiVersion } from '../api/version';
import CollectionsSidebar from '../components/CollectionsSidebar.vue';
import FilterChips from '../components/FilterChips.vue';
import FontGrid from '../components/FontGrid.vue';
import FontLayout from '../components/FontLayout.vue';
import LayoutActions from '../components/LayoutActions.vue';
import FontList from '../components/FontList.vue';
import FontTable from '../components/FontTable.vue';
import FontWaterfall from '../components/FontWaterfall.vue';
import InspectorPanel from '../components/InspectorPanel.vue';
import MainLayout from '../components/MainLayout.vue';
import PreviewControls from '../components/PreviewControls.vue';
import PreviewStrip from '../components/PreviewStrip.vue';
import ProblemsSidebar from '../components/ProblemsSidebar.vue';
import ProjectSetsSidebar from '../components/ProjectSetsSidebar.vue';
import SearchBox from '../components/SearchBox.vue';
import SourcesSidebar from '../components/SourcesSidebar.vue';
import TableColumnsMenu from '../components/TableColumnsMenu.vue';
import { ACTIONS, ariaShortcut, chordOf, formatChord } from '../keyboard/keymap';
import { useActivationStore } from '../stores/activation';
import { useAppStore } from '../stores/app';
import { useCollectionsStore } from '../stores/collections';
import { useActions, useCommandsStore } from '../stores/commands';
import { useFontListStore } from '../stores/fontList';
import { useLayoutStore } from '../stores/layout';
import { useLibraryStore } from '../stores/library';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import { useProblemsStore } from '../stores/problems';
import { useSearchStore } from '../stores/search';
import { useSetsStore } from '../stores/sets';
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
const MODES: { value: ViewMode; label: string }[] = [
  { value: 'list', label: 'List' },
  { value: 'table', label: 'Table' },
  { value: 'grid', label: 'Grid' },
  { value: 'waterfall', label: 'Waterfall' },
  { value: 'layout', label: 'Layout' },
];

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

onMounted(() => {
  void app.load();
  void library.load();
  void activation.load();
  void problems.load();
  void search.loadLanguages();
  void sets.load();
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

useActions([
  {
    id: ACTIONS.activateSession,
    title: 'Activate or deactivate for this session',
    group: 'Activation',
    enabled: canToggle,
    run: () => toggle('session'),
  },
  {
    id: ACTIONS.activatePermanent,
    title: 'Activate or deactivate permanently',
    group: 'Activation',
    enabled: canToggle,
    run: () => toggle('permanent'),
  },
  {
    id: 'activation.deactivateAll',
    title: 'Deactivate all',
    group: 'Activation',
    enabled: () => activation.count > 0 && !activation.busy,
    run: () => activation.deactivateAll(),
  },
  {
    id: 'activation.deactivateOutsideSet',
    title: 'Deactivate everything not in the current set',
    group: 'Activation',
    // The current set is the one picked under Project sets in the sidebar.
    enabled: () => sets.current != null && activation.count > 0 && !activation.busy,
    run: () => (sets.current ? activation.deactivateAllExcept(sets.current.id) : undefined),
  },
  // Keys 1–5 switch the view mode, in the order of the toolbar buttons.
  ...MODES.map((m, i) => ({
    id: ACTIONS.viewMode((i + 1) as 1 | 2 | 3 | 4 | 5),
    title: `Show fonts as a ${m.label.toLowerCase()}`,
    group: 'View',
    run: () => views.setMode(ALL_FONTS_VIEW, m.value),
  })),
  ...LAYOUT_PRESETS.map((p) => ({
    id: `view.layoutPreset.${p}`,
    title: `Show the ${PRESET_LABELS[p]} layout`,
    group: 'View',
    run: () => showPreset(p),
  })),
]);

const paletteChord = chordOf(ACTIONS.palette)!;
const settingsChord = chordOf(ACTIONS.settings)!;

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
const groups = ['Smart collections', 'Tags', 'Online catalogs'];
</script>

<template>
  <MainLayout>
    <template #sidebar>
      <SourcesSidebar />
      <CollectionsSidebar />
      <ProblemsSidebar />
      <ProjectSetsSidebar />
      <ul class="groups">
        <li v-for="group in groups" :key="group" class="group">{{ group }}</li>
      </ul>
    </template>

    <template #list>
      <header class="toolbar">
        <h1 class="title">{{ collections.open?.name ?? 'All fonts' }}</h1>
        <SearchBox />
        <p class="count" role="status">
          <template v-if="fontList.selectedCount > 1"
            >{{ fontList.selectedCount }} selected</template
          >
          <template v-else-if="search.hits"
            >{{ search.total }} {{ search.total === 1 ? 'family' : 'families' }}</template
          >
        </p>
        <div class="modes" role="group" aria-label="View mode">
          <button
            v-for="m in MODES"
            :key="m.value"
            type="button"
            class="toggle"
            :aria-pressed="mode === m.value"
            @click="views.setMode(ALL_FONTS_VIEW, m.value)"
          >
            {{ m.label }}
          </button>
        </div>
        <TableColumnsMenu v-if="mode === 'table'" :view="ALL_FONTS_VIEW" />
        <label v-if="mode === 'layout'" class="preset">
          Preset
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
              {{ e.broken === null ? e.name : `${e.name} (cannot be read)` }}
            </option>
          </select>
        </label>
        <button
          type="button"
          class="toggle"
          aria-controls="fm-inspector"
          aria-keyshortcuts="Control+I Meta+I"
          :aria-pressed="layout.inspectorVisible"
          title="Show or hide the inspector (Ctrl+I / ⌘I)"
          @click="layout.toggleInspector"
        >
          Inspector
        </button>
      </header>
      <FilterChips />
      <PreviewControls :view="ALL_FONTS_VIEW" />
      <template v-if="fontList.families.length && mode === 'table'">
        <PreviewStrip :view="ALL_FONTS_VIEW" :preview="preview.settingsFor(ALL_FONTS_VIEW)" />
        <FontTable :view="ALL_FONTS_VIEW" @cursor-face="showFace" @open="openFace" />
      </template>
      <template v-else-if="fontList.families.length && mode === 'layout'">
        <LayoutActions @opened="(id) => views.setLayout(ALL_FONTS_VIEW, id)" />
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
      <section v-else class="empty" aria-label="Font list">
        <p v-if="library.error" role="alert" class="error">{{ library.error.message }}</p>
        <p v-else-if="search.hits">No fonts match the search and filters.</p>
        <p v-else-if="library.loaded">No fonts yet. Add a folder to start your library.</p>
      </section>
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
        Commands <kbd>{{ formatChord(paletteChord) }}</kbd>
      </button>
      <RouterLink
        :to="{ name: 'settings' }"
        class="hint"
        :aria-keyshortcuts="ariaShortcut(settingsChord)"
      >
        Settings <kbd>{{ formatChord(settingsChord) }}</kbd>
      </RouterLink>
      <RouterLink :to="{ name: 'conflicts' }" class="hint">Conflicts</RouterLink>
      <RouterLink :to="{ name: 'problems' }" class="hint" data-testid="problem-count">
        {{ problems.total === 1 ? '1 problem' : `${problems.total} problems` }}
      </RouterLink>
      <p class="active-count" data-testid="active-count" role="status">
        {{ activation.count === 1 ? '1 font active' : `${activation.count} fonts active` }}
      </p>
      <p v-if="app.error" role="alert" class="error">{{ app.error.message }}</p>
      <dl v-else-if="app.info" class="info" data-testid="app-info">
        <dt>App</dt>
        <dd>{{ app.info.name }} {{ app.info.version }}</dd>
        <dt>API</dt>
        <dd>{{ formatApiVersion(app.info.apiVersion) }}</dd>
        <dt>Platform</dt>
        <dd>{{ app.info.platform }}</dd>
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
  margin: 0 8px 0 0;
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
</style>
