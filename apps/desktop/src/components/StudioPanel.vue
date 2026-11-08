<script setup lang="ts">
// One Pairing Studio panel (E24.3): a layout picker, the layout file's actions and the
// layout's live blocks with their font pickers, all on the panel's own text layouts store.
// The first panel is the library's layout: its layout comes from the URL (`layoutId`) and a
// pick goes back up as `pick`. The others open their pick in their own store. Each panel is
// a region named by its place and layout ("Panel 2: Book").
//
// "Print..." (E24.4) prints this panel alone through the OS print dialog: the studio store
// marks it the print target and the print stylesheet leaves out everything else, the
// controls marked `fm-no-print` included. PDF is the dialog's save-as-PDF.
//
// The first panel has no bar of its own (E45.1): the Studio's one toolbar picks its context,
// saves and exports it, and the command palette prints it. Its heading stays for screen
// readers. The compared panel keeps its bar: layout, file actions, Print and Close.
//
// With `rail` and a layout that has roles (a context, E45.2), the roles rail sits left of the
// page: a card per role with its font picker and lock, in place of a picker beside each block.
// The page's blocks show their role only on hover; a click (or the focus) selects a block, its
// card in the rail gets the ring and scrolls into view. A layout without roles keeps the
// per-block pickers (E24.2).
//
// Shuffle, a Try pick and Undo (E45.3) crossfade the blocks they change (`fades`).
import { computed, onMounted, ref, useId, watch } from 'vue';
import type { FaceSummary } from '../api/types';
import { t } from '../i18n';
import { blockScope, layoutsOf, roleOf, useStudioStore } from '../stores/studio';
import { LAYOUT_PRESETS, PRESET_LABELS } from '../stores/views';
import BlockFontPicker from './BlockFontPicker.vue';
import FontLayout from './FontLayout.vue';
import LayoutActions from './LayoutActions.vue';
import StudioRolesRail from './StudioRolesRail.vue';
import UiIconButton from './ui/UiIconButton.vue';
import UiSelect from './ui/UiSelect.vue';

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
  /** Whether the panel shows the roles rail for a layout with roles (E45.2). */
  rail?: boolean;
}>();

const emit = defineEmits<{
  /** The first panel picked another layout; the parent puts it in the URL. */
  pick: [id: string];
  /** "Close panel" was pressed. */
  close: [];
  /** The rail's "Browse catalogs": the library has nothing to suggest (E45.3). */
  browse: [];
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
/** The picker's options: an unreadable layout is listed, named so, and cannot be chosen. */
const pickerOptions = computed(() =>
  layoutOptions.value.map((e) => ({
    value: e.id,
    label: e.broken === null ? e.name : t('home.unreadable', { name: e.name }),
    disabled: e.broken !== null,
    title: e.broken ?? undefined,
  })),
);

watch(
  () => props.layoutId,
  (id) => {
    // An empty id: the context list has not arrived yet.
    if (id && layouts.saved?.id !== id) void layouts.open(id);
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
/** Each block's crossfade count: Shuffle, a Try pick and Undo crossfade a block (E45.3). */
const blockFades = computed(() =>
  (layouts.draft?.blocks ?? []).map((_, i) => studio.fadeOf(scope.value, i)),
);

/** The layout's roles, when the panel shows them in the rail. */
const roles = computed(() => (props.rail ? (layouts.draft?.roles ?? []) : []));
const hasRail = computed(() => roles.value.length > 0 && scope.value !== null);
/** Each block's role name, the mark it shows in the margin on hover and while selected. */
const roleMarks = computed(() =>
  hasRail.value
    ? (layouts.draft?.blocks ?? []).map((_, i) => {
        const r = roleOf(roles.value, i);
        return r ? t(`studio.role.${r.role}`) : null;
      })
    : undefined,
);

/** The selected block; another layout starts with none. */
const selectedBlock = ref<number | null>(null);
watch(
  () => layouts.saved?.id,
  () => (selectedBlock.value = null),
);
const selectedRole = computed(() =>
  selectedBlock.value === null ? null : (roleOf(roles.value, selectedBlock.value)?.role ?? null),
);
const railRef = ref<InstanceType<typeof StudioRolesRail> | null>(null);

/** A block was clicked or focused: it is the selected one, and its card comes into view. */
function select(index: number): void {
  selectedBlock.value = index;
  const role = roleOf(roles.value, index);
  if (role) railRef.value?.reveal(role.role);
}

const printing = computed(() => studio.printPanel === props.panel);
const printError = computed(() =>
  studio.printError?.panel === props.panel ? studio.printError.error : null,
);

const heading = ref<HTMLHeadingElement | null>(null);
defineExpose({ focus: () => heading.value?.focus() });

onMounted(() => {
  void layouts.loadList();
  void layouts.loadBounds();
});
</script>

<template>
  <section
    class="panel"
    :aria-labelledby="headingId"
    data-testid="studio-panel"
    :data-print-target="printing ? '' : undefined"
  >
    <h2 v-if="!closable" :id="headingId" ref="heading" class="sr-only" tabindex="-1">
      {{ label }}
    </h2>
    <header v-else class="bar fm-no-print">
      <h2 :id="headingId" ref="heading" class="name" tabindex="-1">{{ label }}</h2>
      <LayoutActions :store="layouts" inline @opened="pick" />
      <label class="picker">
        {{ t('studio.layout') }}
        <UiSelect :model-value="selected" :options="pickerOptions" @update:model-value="pick" />
      </label>
      <UiIconButton
        icon="print"
        :label="t('studio.printPanelFor', { number: String(number) })"
        :title="t('studio.printTitle')"
        @click="studio.print(panel)"
      />
      <UiIconButton
        icon="close"
        :label="t('studio.closePanelFor', { number: String(number) })"
        @click="emit('close')"
      />
    </header>
    <p v-if="printError" class="error fm-no-print" role="alert">{{ printError.message }}</p>
    <div class="body">
      <StudioRolesRail
        v-if="hasRail && layouts.draft && scope"
        ref="railRef"
        :scope="scope"
        :layout="layouts.draft"
        :face="face"
        :selected="selectedRole"
        @browse="emit('browse')"
      />
      <FontLayout
        :layout="layouts.draft"
        :face="face"
        editable
        :bounds="layouts.bounds"
        :set-props="layouts.setBlockProps"
        :width="layouts.width"
        :set-width-lock="layouts.setWidthLock"
        bare
        :block-faces="blockFaces"
        :fades="blockFades"
        :role-marks="roleMarks"
        :selected="hasRail ? selectedBlock : undefined"
        @edit-text="layouts.setBlockText"
        @select="select"
      >
        <template v-if="scope && !hasRail" #block-tools="{ index, label: blockLabel }">
          <BlockFontPicker
            :layout-id="scope"
            :index="index"
            :label="blockLabel"
            :face="blockFaces[index] ?? null"
          />
        </template>
      </FontLayout>
    </div>
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
/* The rail beside the page; the page takes the rest. */
.body {
  display: flex;
  flex: 1;
  min-height: 0;
}
.panel + .panel {
  border-inline-start: 1px solid var(--fm-border);
}
.bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--fm-space-2);
  min-height: 40px;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
}
.name {
  flex: 0 1 auto;
  margin: 0;
  overflow: hidden;
  font-size: var(--fm-text-sm);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.picker {
  display: flex;
  margin-inline-start: auto;
  align-items: center;
  gap: 4px;
}
.error {
  margin: 0;
  padding: 4px 16px;
  color: var(--fm-danger);
}
.name:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
