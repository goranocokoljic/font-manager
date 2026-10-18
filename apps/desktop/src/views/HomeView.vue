<script setup lang="ts">
import { onBeforeUnmount, onMounted, watch } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import { formatApiVersion } from '../api/version';
import FontList from '../components/FontList.vue';
import InspectorPanel from '../components/InspectorPanel.vue';
import MainLayout from '../components/MainLayout.vue';
import PreviewControls from '../components/PreviewControls.vue';
import SourcesSidebar from '../components/SourcesSidebar.vue';
import { ACTIONS, ariaShortcut, chordOf, formatChord } from '../keyboard/keymap';
import { useAppStore } from '../stores/app';
import { useActions, useCommandsStore } from '../stores/commands';
import { useFontListStore } from '../stores/fontList';
import { useLayoutStore } from '../stores/layout';
import { useLibraryStore } from '../stores/library';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import type { FaceId } from '../api/types';
import { parseFaceId, useSelectionStore } from '../stores/selection';

const app = useAppStore();
const layout = useLayoutStore();
const selection = useSelectionStore();
const commands = useCommandsStore();
const library = useLibraryStore();
const preview = usePreviewStore();
const fontList = useFontListStore();
const route = useRoute();
const router = useRouter();
onMounted(() => {
  void app.load();
  void library.load();
});
onBeforeUnmount(() => void library.stop());

useActions([
  {
    id: ACTIONS.activateSession,
    title: 'Activate or deactivate for this session',
    group: 'Activation',
    enabled: () => selection.canActivate,
    run: () => selection.toggleActivation('session'),
  },
  {
    id: ACTIONS.activatePermanent,
    title: 'Activate or deactivate permanently',
    group: 'Activation',
    enabled: () => selection.canActivate,
    run: () => selection.toggleActivation('permanent'),
  },
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
const groups = [
  'Collections',
  'Smart collections',
  'Project sets',
  'Tags',
  'Problems',
  'Online catalogs',
];
</script>

<template>
  <MainLayout>
    <template #sidebar>
      <SourcesSidebar />
      <ul class="groups">
        <li v-for="group in groups" :key="group" class="group">{{ group }}</li>
      </ul>
    </template>

    <template #list>
      <header class="toolbar">
        <h1 class="title">All fonts</h1>
        <p class="count" role="status">
          <template v-if="fontList.selectedCount > 1"
            >{{ fontList.selectedCount }} selected</template
          >
        </p>
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
      <PreviewControls :view="ALL_FONTS_VIEW" />
      <FontList
        v-if="library.families.length"
        :preview="preview.settingsFor(ALL_FONTS_VIEW)"
        @cursor-face="showFace"
        @open="openFace"
      />
      <section v-else class="empty" aria-label="Font list">
        <p v-if="library.error" role="alert" class="error">{{ library.error.message }}</p>
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
