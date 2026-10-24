<script setup lang="ts">
// One Pairing Studio panel (E24.3): a layout picker, the layout file's actions and the
// layout's live blocks with their font pickers, all on the panel's own text layouts store.
// The first panel is the library's layout: its layout comes from the URL (`layoutId`) and a
// pick goes back up as `pick`. The others open their pick in their own store. Each panel is
// a region named by its place and layout ("Panel 2: Book").
import { computed, onMounted, ref, useId, watch } from 'vue';
import type { FaceSummary } from '../api/types';
import { t } from '../i18n';
import { blockScope, layoutsOf, useStudioStore } from '../stores/studio';
import { LAYOUT_PRESETS, PRESET_LABELS } from '../stores/views';
import BlockFontPicker from './BlockFontPicker.vue';
import FontLayout from './FontLayout.vue';
import LayoutActions from './LayoutActions.vue';

const props = defineProps<{
  /** The panel's key in the studio store. */
  panel: number;
  /** The panel's place, from 1, left to right. */
  number: number;
  /** The face the panel's blocks are set in, unless a block has its own. */
  face: FaceSummary | null;
  /** The first panel's layout, from the URL; not given for the other panels. */
  layoutId?: string;
  /** Whether the panel has a "Close panel" button. */
  closable: boolean;
}>();

const emit = defineEmits<{
  /** The first panel picked another layout; the parent puts it in the URL. */
  pick: [id: string];
  /** "Close panel" was pressed. */
  close: [];
}>();

const studio = useStudioStore();
const layouts = layoutsOf(props.panel);
const headingId = `fm-studio-panel-${useId()}`;

const selected = computed(() => props.layoutId ?? layouts.saved?.id ?? '');
const label = computed(() =>
  t('studio.panel', {
    number: String(props.number),
    name: layouts.draft?.name ?? t('studio.noLayout'),
  }),
);

// Before the list arrives the picker offers the presets, so the pick shows at once.
const layoutOptions = computed(() =>
  layouts.entries.length
    ? layouts.entries
    : LAYOUT_PRESETS.map((p) => ({ id: p, name: PRESET_LABELS[p], preset: p, broken: null })),
);

watch(
  () => props.layoutId,
  (id) => {
    if (id !== undefined && layouts.saved?.id !== id) void layouts.open(id);
  },
  { immediate: true },
);

/** Shows another layout in the panel; the library's Layout mode keeps its own pick. */
function pick(id: string): void {
  if (props.layoutId !== undefined) emit('pick', id);
  else if (layouts.saved?.id !== id) void layouts.open(id);
}

/** The block-font scope of the open layout; block fonts are kept per panel and layout. */
const scope = computed(() => (layouts.saved ? blockScope(props.panel, layouts.saved.id) : null));
const blockFaces = computed(() =>
  (layouts.draft?.blocks ?? []).map((_, i) => studio.faceOf(scope.value, i)),
);

const heading = ref<HTMLHeadingElement | null>(null);
defineExpose({ focus: () => heading.value?.focus() });

onMounted(() => {
  void layouts.loadList();
  void layouts.loadBounds();
});
</script>

<template>
  <section class="panel" :aria-labelledby="headingId" data-testid="studio-panel">
    <header class="bar">
      <h2 :id="headingId" ref="heading" class="name" tabindex="-1">{{ label }}</h2>
      <label class="picker">
        {{ t('studio.layout') }}
        <select :value="selected" @change="pick(($event.target as HTMLSelectElement).value)">
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
        v-if="closable"
        type="button"
        class="button"
        :aria-label="t('studio.closePanelFor', { number: String(number) })"
        @click="emit('close')"
      >
        {{ t('studio.closePanel') }}
      </button>
    </header>
    <LayoutActions :store="layouts" @opened="pick" />
    <FontLayout
      :layout="layouts.draft"
      :face="face"
      editable
      :bounds="layouts.bounds"
      :set-props="layouts.setBlockProps"
      :width="layouts.width"
      :set-width-lock="layouts.setWidthLock"
      :block-faces="blockFaces"
      @edit-text="layouts.setBlockText"
    >
      <template v-if="scope" #block-tools="{ index, label: blockLabel }">
        <BlockFontPicker
          :layout-id="scope"
          :index="index"
          :label="blockLabel"
          :face="blockFaces[index] ?? null"
        />
      </template>
    </FontLayout>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex: 1 1 0;
  flex-direction: column;
  min-width: 26rem;
  min-height: 0;
}
.panel + .panel {
  border-inline-start: 1px solid var(--fm-border);
}
.bar {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 40px;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
}
.name {
  flex: 1;
  margin: 0;
  overflow: hidden;
  font-size: 12px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
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
.name:focus-visible,
.picker select:focus-visible,
.button:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
