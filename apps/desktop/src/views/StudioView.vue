<script setup lang="ts">
// Pairing Studio (E24.1): one body-text layout set in one face, in its own panel at
// `#/studio?layout=<id>&face=<id>`. The blocks are the Layout mode's live text
// (`FontLayout` / `LayoutBlock`), edited through the same text layouts store, so Save
// writes the same template file. Close or Escape goes back to the library: to the entry
// the studio was opened from when there is one, so the selection and view come back as
// they were.
//
// Each block has a font picker beside its size badge (E24.2): a block can be set in a face
// of its own; the others use the panel's. The choice lives in the studio store for the
// session and never goes into the layout file.
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import BlockFontPicker from '../components/BlockFontPicker.vue';
import FontLayout from '../components/FontLayout.vue';
import LayoutActions from '../components/LayoutActions.vue';
import { t } from '../i18n';
import { useCommandsStore } from '../stores/commands';
import { ALL_FONTS_VIEW } from '../stores/preview';
import { parseFaceId, useSelectionStore } from '../stores/selection';
import { useStudioStore } from '../stores/studio';
import { useTextLayoutsStore } from '../stores/textLayouts';
import { LAYOUT_PRESETS, PRESET_LABELS, useViewsStore } from '../stores/views';

const route = useRoute();
const router = useRouter();
const commands = useCommandsStore();
const selection = useSelectionStore();
const textLayouts = useTextLayoutsStore();
const views = useViewsStore();
const studio = useStudioStore();

/** The layout in the URL; without one, the library's Layout mode pick. */
const layoutId = computed(() => {
  const raw = Array.isArray(route.query.layout) ? route.query.layout[0] : route.query.layout;
  return typeof raw === 'string' && raw ? raw : views.layoutFor(ALL_FONTS_VIEW);
});
const faceId = computed(() => parseFaceId(route.query.face));

// The panel's face is the selected face, so the library shows the same one after Close.
watch(faceId, (id) => void selection.select(id), { immediate: true });
const face = computed(() => {
  const shown = selection.detail?.face;
  return shown && shown.id === faceId.value ? shown : null;
});
const fontName = computed(() =>
  face.value ? `${face.value.familyName} ${face.value.style}` : t('studio.noFont'),
);

// Before the list arrives the picker offers the presets, so the pick shows at once.
const layoutOptions = computed(() =>
  textLayouts.entries.length
    ? textLayouts.entries
    : LAYOUT_PRESETS.map((p) => ({ id: p, name: PRESET_LABELS[p], preset: p, broken: null })),
);

watch(
  layoutId,
  (id) => {
    if (textLayouts.saved?.id !== id) void textLayouts.open(id);
  },
  { immediate: true },
);

/** The layout the blocks belong to, once it is open; block fonts are kept per layout. */
const openId = computed(() => textLayouts.saved?.id ?? null);
const blockFaces = computed(() =>
  (textLayouts.draft?.blocks ?? []).map((_, i) => studio.faceOf(openId.value, i)),
);

/** Shows another layout in the panel; the library's Layout mode keeps its own pick. */
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

// The panel takes the focus, so Escape closes it at once; Escape from anywhere in the
// window closes it, unless a block editor or the palette used the key first.
const title = ref<HTMLHeadingElement | null>(null);
onMounted(() => {
  title.value?.focus();
  window.addEventListener('keydown', onKey);
  void textLayouts.loadList();
  void textLayouts.loadBounds();
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
      <label class="picker">
        {{ t('studio.layout') }}
        <select :value="layoutId" @change="pick(($event.target as HTMLSelectElement).value)">
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
    <LayoutActions @opened="pick" />
    <FontLayout
      :layout="textLayouts.draft"
      :face="face"
      editable
      :bounds="textLayouts.bounds"
      :set-props="textLayouts.setBlockProps"
      :width="textLayouts.width"
      :set-width-lock="textLayouts.setWidthLock"
      :block-faces="blockFaces"
      @edit-text="textLayouts.setBlockText"
    >
      <template v-if="openId" #block-tools="{ index, label }">
        <BlockFontPicker
          :layout-id="openId"
          :index="index"
          :label="label"
          :face="blockFaces[index] ?? null"
        />
      </template>
    </FontLayout>
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
.picker {
  display: flex;
  align-items: center;
  gap: 4px;
}
.picker select {
  padding: 2px 4px;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  background: var(--fm-bg);
  color: inherit;
  font: inherit;
}
.button {
  padding: 4px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.title:focus-visible,
.picker select:focus-visible,
.button:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
