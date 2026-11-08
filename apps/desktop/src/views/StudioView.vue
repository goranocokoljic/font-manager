<script setup lang="ts">
// Pairing Studio (E24.1): body-text layouts set in one face, at
// `#/studio?layout=<id>&face=<id>`. The blocks are the Layout mode's live text
// (`FontLayout` / `LayoutBlock`), edited through the text layouts store, so Save writes the
// same template file. Back or Escape goes back to the library: to the entry the studio was
// opened from when there is one, so the selection and view come back as they were.
//
// Each block has a font picker beside its size badge (E24.2): a block can be set in a face
// of its own; the others use the panel's. The choice lives in the studio store for the
// session and never goes into the layout file.
//
// One toolbar (E45.1, board 5): Back · the title · the pairing ("<headline> + <body>", and
// "unsaved" while the layout has unsaved edits) · the context switch · Compare 2 pairings ·
// Shuffle unlocked · Save · Export PNG. A context is a layout template the core ships with a
// page of its own (Magazine and Poster on A4, a 1440 × 900 web hero, a 390 × 844 phone, a
// 1920 × 1080 video title); picking one shows that file. Without a layout in the URL the Studio
// opens on the Magazine context. A layout of another kind (from the library's Layout mode)
// shows with no context pressed.
//
// "Compare 2 pairings" puts a copy of the first panel beside it, with its own store and block
// fonts (E24.3). Print (E24.4) is in the command palette for the first panel and in the second
// panel's own bar.
//
// A pairing card of the family page (E42.7) opens `?face=<body>&headline=<face>`: the panel's
// face sets the body and the first panel's headline block is set in the headline face, once
// for each layout the panel shows.
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { StudioContext } from '../api/types';
import StudioPanel from '../components/StudioPanel.vue';
import UiButton from '../components/ui/UiButton.vue';
import UiIcon from '../components/ui/UiIcon.vue';
import UiSegmentedControl, { type SegmentOption } from '../components/ui/UiSegmentedControl.vue';
import { errorMessage, t } from '../i18n';
import { useActions, useCommandsStore } from '../stores/commands';
import { parseFaceId, useSelectionStore } from '../stores/selection';
import {
  FIRST_PANEL,
  blockScope,
  headlineBlock,
  layoutsOf,
  useStudioStore,
} from '../stores/studio';

const route = useRoute();
const router = useRouter();
const commands = useCommandsStore();
const selection = useSelectionStore();
const studio = useStudioStore();
const firstLayouts = layoutsOf(FIRST_PANEL);

/** The context layouts the core ships, in its order: the first file of each context. */
const contextEntries = computed(() => {
  const seen = new Set<StudioContext>();
  return firstLayouts.entries.filter((e) => {
    if (!e.context || e.broken !== null || seen.has(e.context)) return false;
    seen.add(e.context);
    return true;
  });
});
const contextOptions = computed<SegmentOption<StudioContext>[]>(() =>
  contextEntries.value.map((e) => ({
    value: e.context!,
    label: t(`studio.context.${e.context!}`),
  })),
);

/** The first panel's layout, from the URL; without one, the Magazine context's file. */
const layoutId = computed(() => {
  const raw = Array.isArray(route.query.layout) ? route.query.layout[0] : route.query.layout;
  if (typeof raw === 'string' && raw) return raw;
  return contextEntries.value.find((e) => e.context === 'magazine')?.id ?? '';
});
/** The context the first panel shows; none for a layout of another kind. */
const context = computed<StudioContext | ''>(() =>
  firstLayouts.saved?.id === layoutId.value ? (firstLayouts.draft?.context ?? '') : '',
);

const faceId = computed(() => parseFaceId(route.query.face));

// The panels' face is the selected face, so the library shows the same one after Back.
watch(faceId, (id) => void selection.select(id), { immediate: true });
const face = computed(() => {
  const shown = selection.detail?.face;
  return shown && shown.id === faceId.value ? shown : null;
});
const fontName = computed(() =>
  face.value ? `${face.value.familyName} ${face.value.style}` : t('studio.noFont'),
);

/** The block-font scope of the first panel's open layout. */
const firstScope = computed(() =>
  firstLayouts.saved ? blockScope(FIRST_PANEL, firstLayouts.saved.id) : null,
);
/** "<headline> + <body>": the headline block's face (its own, else the panel's) and the panel's. */
const pairing = computed(() => {
  const body = face.value;
  if (!body) return t('studio.noFont');
  const index = headlineBlock(firstLayouts.draft?.blocks ?? []);
  const headline = (index !== null ? studio.faceOf(firstScope.value, index) : null) ?? body;
  return t('studio.pairing', { headline: headline.familyName, body: body.familyName });
});

// The pairing's headline face, once the first panel's layout has loaded.
const headline = computed(() => parseFaceId(route.query.headline));
let pairingLoaded: string | null = null;
watch(
  () => [headline.value, firstLayouts.saved?.id, firstLayouts.draft] as const,
  ([face, id, draft]) => {
    if (face === null || !id || !draft) return;
    const key = `${face}@${id}`;
    if (pairingLoaded === key) return;
    pairingLoaded = key;
    void studio.loadPairing(id, draft.blocks, face);
  },
  { immediate: true },
);

/** Shows another context in the first panel; the library's Layout mode keeps its own pick. */
function pickContext(c: StudioContext): void {
  const id = contextEntries.value.find((e) => e.context === c)?.id;
  if (id) pick(id);
}

/** Shows another layout in the first panel. */
function pick(id: string): void {
  void router.replace({ query: { ...route.query, layout: id } });
}

const comparing = computed(() => studio.panels.length > 1);
const compareButton = ref<InstanceType<typeof UiButton> | null>(null);

/** Closes the compared panel; the focus goes to "Compare 2 pairings", which brings it back. */
function closePanel(panel: number): void {
  studio.closePanel(panel);
  (compareButton.value?.$el as HTMLElement | undefined)?.focus();
}

/** What the last Save or Export did, read in the language shown now. */
const status = ref<(() => string) | null>(null);
const saveError = computed(() => (firstLayouts.error ? errorMessage(firstLayouts.error) : null));
const message = computed(() => saveError.value ?? status.value?.() ?? '');

async function save(): Promise<void> {
  if (!firstLayouts.dirty || firstLayouts.busy) return;
  status.value = null;
  if (await firstLayouts.save()) status.value = () => t('studio.saved');
}

const canExport = computed(() => Boolean(face.value && firstLayouts.saved && !firstLayouts.dirty));
async function exportPng(): Promise<void> {
  if (!canExport.value || !face.value) return;
  status.value = null;
  const path = await studio.exportPng(FIRST_PANEL, face.value.id);
  if (path) status.value = () => t('studio.exported', { path });
  else if (studio.exported && 'error' in studio.exported) {
    const { error } = studio.exported;
    status.value = () => errorMessage(error);
  }
}

// Getters: the palette shows the title in the language chosen now.
useActions([
  {
    id: 'studio.print',
    get title() {
      return t('studio.printPage');
    },
    get group() {
      return t('studio.title');
    },
    enabled: () => Boolean(firstLayouts.draft),
    run: () => studio.print(FIRST_PANEL),
  },
]);

function close(): void {
  if (typeof router.options.history.state.back === 'string') {
    router.back();
    return;
  }
  const query = faceId.value !== null ? { face: String(faceId.value) } : {};
  void router.push({ name: 'home', query });
}

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' && !commands.open && !e.defaultPrevented) {
    e.preventDefault();
    close();
  }
}

// The studio takes the focus, so Escape closes it at once; Escape from anywhere in the
// window closes it, unless a block editor or the palette used the key first.
const title = ref<HTMLHeadingElement | null>(null);
onMounted(() => {
  title.value?.focus();
  window.addEventListener('keydown', onKey);
  void firstLayouts.loadList();
});
onBeforeUnmount(() => window.removeEventListener('keydown', onKey));
</script>

<template>
  <section class="studio" :aria-label="t('studio.region', { name: fontName })">
    <header class="bar" data-testid="studio-toolbar">
      <UiButton
        class="back"
        :title="t('studio.closeTitle')"
        aria-keyshortcuts="Escape"
        @click="close"
      >
        <UiIcon name="chevronBack" />{{ t('studio.back') }}
      </UiButton>
      <h1 ref="title" class="title" tabindex="-1">{{ t('studio.title') }}</h1>
      <p class="pairing" data-testid="studio-pairing">
        <bdi>{{ pairing }}</bdi>
        <span v-if="firstLayouts.dirty" class="unsaved"> · {{ t('studio.unsaved') }}</span>
      </p>
      <UiSegmentedControl
        class="contexts"
        :model-value="context as StudioContext"
        :options="contextOptions"
        :label="t('studio.contexts')"
        @update:model-value="pickContext"
      />
      <div class="actions">
        <UiButton
          ref="compareButton"
          :aria-pressed="comparing"
          :title="t('studio.compareTitle')"
          @click="studio.toggleCompare()"
        >
          <UiIcon name="compare" />{{ t('studio.compare') }}
        </UiButton>
        <UiButton aria-disabled="true" :title="t('studio.shuffleTitle')">
          <UiIcon name="rescan" />{{ t('studio.shuffle') }}
        </UiButton>
        <UiButton
          :aria-disabled="!firstLayouts.dirty || firstLayouts.busy"
          :title="t('studio.saveTitle')"
          @click="save"
        >
          {{ t('studio.save') }}
        </UiButton>
        <UiButton
          class="export"
          :aria-disabled="!canExport"
          :title="firstLayouts.dirty ? t('studio.exportSaveFirst') : t('studio.exportTitle')"
          @click="exportPng"
        >
          {{ t('studio.export') }}
        </UiButton>
      </div>
    </header>
    <p class="status" :class="{ shown: message }" role="status" data-testid="studio-status">
      {{ message }}
    </p>
    <div class="panels">
      <StudioPanel
        v-for="(panel, i) in studio.panels"
        :key="panel"
        :panel="panel"
        :number="i + 1"
        :face="face"
        :layout-id="panel === FIRST_PANEL ? layoutId : undefined"
        :closable="panel !== FIRST_PANEL"
        @pick="pick"
        @close="closePanel(panel)"
      />
    </div>
  </section>
</template>

<style scoped>
.studio {
  display: flex;
  flex-direction: column;
  min-width: 0;
  height: 100vh;
}
/* One bar; a window too narrow for it wraps the actions under the rest, never wider. */
.bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2) var(--fm-space-4);
  min-height: 56px;
  padding: var(--fm-space-2) var(--fm-space-4);
  border-bottom: 1px solid var(--fm-border);
  background-color: var(--fm-bg);
}
.title {
  flex: none;
  margin: 0;
  font-size: var(--fm-text-md);
  font-weight: 600;
  white-space: nowrap;
}
.pairing {
  flex: 1 1 0;
  min-width: 8em;
  margin: 0;
  overflow: hidden;
  color: var(--fm-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* Board 5: a quiet track, the chosen context on a raised chip. The chip's colour is the
   track's in dark, so a 1 px control-border ring marks the chosen context at 3:1 (WCAG 1.4.11). */
.contexts {
  flex: none;
  border: 0;
  border-radius: var(--fm-radius-lg);
}
.contexts :deep(.seg.text) {
  height: 28px;
  padding: 0 var(--fm-space-3);
}
.contexts :deep(.seg[aria-pressed='true']) {
  background-color: var(--fm-surface-2);
  color: var(--fm-fg);
  box-shadow:
    0 0 0 1px var(--fm-control-border),
    var(--fm-shadow-1);
}
.actions {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--fm-space-2);
  margin-inline-start: auto;
}
.export {
  padding: 0 var(--fm-space-3);
  height: 32px;
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bar-bg);
  color: var(--fm-bar-fg);
  font-weight: 600;
}
.export:not(:disabled):hover {
  color: var(--fm-bar-fg);
}
/* Always in the page, so a screen reader hears the next message; no room while empty. */
.status {
  margin: 0;
  padding: 0 var(--fm-space-4);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.status.shown {
  padding-block: var(--fm-space-1);
}
/* Equal widths; below a panel's least width the row scrolls sideways. */
.panels {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow-x: auto;
}
</style>
