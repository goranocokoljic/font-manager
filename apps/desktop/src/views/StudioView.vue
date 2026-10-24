<script setup lang="ts">
// Pairing Studio (E24.1): body-text layouts set in one face, at
// `#/studio?layout=<id>&face=<id>`. The blocks are the Layout mode's live text
// (`FontLayout` / `LayoutBlock`), edited through the text layouts store, so Save writes the
// same template file. Close or Escape goes back to the library: to the entry the studio was
// opened from when there is one, so the selection and view come back as they were.
//
// Each block has a font picker beside its size badge (E24.2): a block can be set in a face
// of its own; the others use the panel's. The choice lives in the studio store for the
// session and never goes into the layout file.
//
// Up to four panels sit side by side, equal widths, scrolling sideways when the window is
// narrow (E24.3). The first is the URL's layout in the library's store; "Add panel" adds a
// copy of the panel that had the focus last, with its own store and block fonts. The studio
// store keeps the panels, so they are there again when the studio opens later.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import StudioPanel from '../components/StudioPanel.vue';
import { t } from '../i18n';
import { useCommandsStore } from '../stores/commands';
import { ALL_FONTS_VIEW } from '../stores/preview';
import { parseFaceId, useSelectionStore } from '../stores/selection';
import { FIRST_PANEL, MAX_PANELS, useStudioStore } from '../stores/studio';
import { useViewsStore } from '../stores/views';

const route = useRoute();
const router = useRouter();
const commands = useCommandsStore();
const selection = useSelectionStore();
const views = useViewsStore();
const studio = useStudioStore();

/** The first panel's layout, from the URL; without one, the library's Layout mode pick. */
const layoutId = computed(() => {
  const raw = Array.isArray(route.query.layout) ? route.query.layout[0] : route.query.layout;
  return typeof raw === 'string' && raw ? raw : views.layoutFor(ALL_FONTS_VIEW);
});
const faceId = computed(() => parseFaceId(route.query.face));

// The panels' face is the selected face, so the library shows the same one after Close.
watch(faceId, (id) => void selection.select(id), { immediate: true });
const face = computed(() => {
  const shown = selection.detail?.face;
  return shown && shown.id === faceId.value ? shown : null;
});
const fontName = computed(() =>
  face.value ? `${face.value.familyName} ${face.value.style}` : t('studio.noFont'),
);

/** The panel "Add panel" copies: the one that had the focus last, else the last one. */
const current = ref<number | null>(null);
const source = computed(() =>
  current.value !== null && studio.panels.includes(current.value)
    ? current.value
    : studio.panels[studio.panels.length - 1]!,
);
const full = computed(() => studio.panels.length >= MAX_PANELS);

const panelRefs = new Map<number, { focus: () => void }>();
function setPanelRef(panel: number, c: unknown): void {
  if (c) panelRefs.set(panel, c as { focus: () => void });
  else panelRefs.delete(panel);
}
const addButton = ref<HTMLButtonElement | null>(null);

/** Adds a copy of the current panel at the end and moves the focus to it. */
async function addPanel(): Promise<void> {
  const panel = studio.addPanel(source.value);
  if (panel === null) return;
  current.value = panel;
  await nextTick();
  panelRefs.get(panel)?.focus();
}

/** Closes one panel; the focus goes to "Add panel". */
function closePanel(panel: number): void {
  studio.closePanel(panel);
  if (current.value === panel) current.value = null;
  addButton.value?.focus();
}

/** Shows another layout in the first panel; the library's Layout mode keeps its own pick. */
function pick(id: string): void {
  void router.replace({ query: { ...route.query, layout: id } });
}

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
});
onBeforeUnmount(() => window.removeEventListener('keydown', onKey));
</script>

<template>
  <section class="studio" :aria-label="t('studio.region', { name: fontName })">
    <header class="bar">
      <h1 ref="title" class="title" tabindex="-1">{{ t('studio.title') }}</h1>
      <p class="font" data-testid="studio-font">
        <span class="label">{{ t('studio.font') }}</span>
        <bdi>{{ fontName }}</bdi>
      </p>
      <button
        ref="addButton"
        type="button"
        class="button"
        :aria-disabled="full"
        :title="
          full ? t('studio.panelsFull', { max: String(MAX_PANELS) }) : t('studio.addPanelTitle')
        "
        @click="addPanel"
      >
        {{ t('studio.addPanel') }}
      </button>
      <button
        type="button"
        class="button"
        aria-keyshortcuts="Escape"
        :title="t('studio.closeTitle')"
        @click="close"
      >
        {{ t('common.close') }}
      </button>
    </header>
    <div class="panels">
      <StudioPanel
        v-for="(panel, i) in studio.panels"
        :key="panel"
        :ref="(c) => setPanelRef(panel, c)"
        :panel="panel"
        :number="i + 1"
        :face="face"
        :layout-id="panel === FIRST_PANEL ? layoutId : undefined"
        :closable="panel !== FIRST_PANEL"
        @focusin="current = panel"
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
  height: 100vh;
}
.bar {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 48px;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.font {
  flex: 1;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.label {
  margin-inline-end: 4px;
  color: var(--fm-muted);
}
.button {
  padding: 4px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.button[aria-disabled='true'] {
  opacity: 0.5;
}
/* Equal widths; below a panel's least width the row scrolls sideways. */
.panels {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow-x: auto;
}
.title:focus-visible,
.button:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
