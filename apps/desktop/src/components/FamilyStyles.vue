<script setup lang="ts">
// The Styles section of the family page (E42.3): one row per face plus the instances the user
// pinned, or a ramp along one axis of the hero's face. The accent ring marks the row at the
// hero's weight. The list is a grid with one tab stop: ↑/↓ move between rows, Space toggles a
// face's activation, Enter (or a click) sets the hero to the row.
import { computed, nextTick, ref, watch } from 'vue';
import { useClient } from '../api/client';
import type { AxisValue, FaceDetail, FaceId, FaceSummary, VirtualStyle } from '../api/types';
import { errorMessage, t } from '../i18n';
import { rampAxes, rampStops, ringedStop, type RampStop } from '../preview/ramp';
import { coordinateSettings, faceAxes, variationSettings } from '../preview/variations';
import { NEAR_INSTANCE, useAxesStore } from '../stores/axes';
import { useCompareStore } from '../stores/compare';
import { NAMED_STYLES, useFamilyPageStore } from '../stores/familyPage';
import { useToastStore } from '../stores/toast';
import CopyCss from './CopyCss.vue';
import FamilySample from './FamilySample.vue';
import UiBadge from './ui/UiBadge.vue';
import UiButton from './ui/UiButton.vue';
import UiIconButton from './ui/UiIconButton.vue';
import UiPopover from './ui/UiPopover.vue';
import UiSegmentedControl from './ui/UiSegmentedControl.vue';
import UiSwitch from './ui/UiSwitch.vue';

const props = defineProps<{
  /** The detail of the hero's face, once it has loaded. */
  detail: FaceDetail | null;
  text: string;
}>();
const emit = defineEmits<{ choose: [face: FaceId] }>();

const page = useFamilyPageStore();
const axesSet = useAxesStore();
const compare = useCompareStore();
const toast = useToastStore();

const heroAxes = computed(() => props.detail?.axes ?? []);
const ramps = computed(() => rampAxes(heroAxes.value));
/** The view chosen, unless this family has no such axis. */
const view = computed(() =>
  ramps.value.some((a) => a.tag === page.stylesView) ? page.stylesView : NAMED_STYLES,
);
const rampAxis = computed(() => ramps.value.find((a) => a.tag === view.value) ?? null);

const views = computed(() => {
  if (!ramps.value.length) return [];
  const only = ramps.value.length === 1 ? ramps.value[0] : null;
  return [
    {
      value: NAMED_STYLES,
      label: t('family.namedStyles', { count: page.faces.length + page.pins.length }),
    },
    ...ramps.value.map((a) => ({
      value: a.tag,
      label: only?.tag === 'wght' ? t('family.weightRamp') : a.name,
    })),
  ];
});
const hint = computed(() => {
  const axis = rampAxis.value;
  if (!axis) return t('family.namedHint');
  return axis.tag === 'wght'
    ? t('family.weightRampHint')
    : t('family.axisRampHint', { axis: axis.name });
});

const heroFace = computed(() => page.face);
const heroWeight = computed(() => {
  const face = heroFace.value;
  const wght = heroAxes.value.find((a) => a.tag === 'wght');
  return face && wght ? axesSet.value(face.id, wght) : (face?.weight ?? null);
});
const moved = computed(() => (heroFace.value && axesSet.values(heroFace.value.id)) ?? []);

const leans = (face: FaceSummary | undefined): boolean => !!face && face.slant !== 'upright';
/**
 * A named row is at the hero when its weight is within ±12. Upright and italic can share a weight,
 * and the hero shows only one of them, so the row must also lean the way the hero does.
 */
function atHero(weight: number, face: FaceSummary | undefined): boolean {
  const hero = heroWeight.value;
  return (
    hero !== null &&
    Math.abs(weight - hero) <= NEAR_INSTANCE &&
    leans(face) === leans(heroFace.value ?? undefined)
  );
}

function faceMeta(face: FaceSummary): string {
  return face.slant === 'upright'
    ? String(face.weight)
    : `${face.weight} · ${t(`info.slants.${face.slant}`)}`;
}

interface Row {
  key: string;
  title: string;
  meta: string;
  sampleFace: FaceId;
  variation: string;
  ring: boolean;
  face?: FaceSummary;
  pin?: VirtualStyle;
  stop?: RampStop;
}

const namedRows = computed<Row[]>(() => {
  const faceRows = page.faces.map((face): Row => ({
    key: `face-${face.id}`,
    title: face.style,
    meta: faceMeta(face),
    sampleFace: face.id,
    // A variable row moves with the axes the user moved, and otherwise stays at its own default.
    variation: face.isVariable ? coordinateSettings(moved.value) : 'normal',
    ring: atHero(face.weight, face),
    face,
  }));
  const pinRows = page.pins.map((pin): Row => {
    const face = page.faces.find((f) => f.id === pin.face);
    const weight = pin.coordinates.find((c) => c.tag === 'wght')?.value ?? face?.weight ?? 0;
    return {
      key: `pin-${pin.id}`,
      title: pin.name,
      meta: face ? faceMeta({ ...face, weight }) : String(weight),
      sampleFace: pin.face,
      variation: coordinateSettings(pin.coordinates),
      ring: atHero(weight, face),
      pin,
    };
  });
  return [...faceRows, ...pinRows];
});

const rampRows = computed<Row[]>(() => {
  const axis = rampAxis.value;
  const face = heroFace.value;
  if (!axis || !face) return [];
  const stops = rampStops(axis, props.detail?.instances ?? []);
  const ring = ringedStop(axis, stops, axesSet.value(face.id, axis));
  // The other axes hold the hero's current values while this one is ramped.
  const others = moved.value.filter((v) => v.tag !== axis.tag);
  return stops.map((stop, i) => ({
    key: `ramp-${axis.tag}-${stop.value}`,
    title: String(stop.value),
    meta: stop.name ?? '',
    sampleFace: face.id,
    variation: variationSettings(heroAxes.value, [...others, { tag: axis.tag, value: stop.value }]),
    ring: i === ring,
    stop,
  }));
});

const rows = computed(() => (rampAxis.value ? rampRows.value : namedRows.value));
const rowLabel = (row: Row) =>
  [row.title, row.meta, row.pin ? t('family.yours') : ''].filter(Boolean).join(', ');

/** The row with the list's one tab stop. */
const cursor = ref(0);
const current = computed(() => Math.min(cursor.value, rows.value.length - 1));
watch([view, () => page.id], () => (cursor.value = 0));

const grid = ref<HTMLElement | null>(null);
const rowElement = (i: number) => grid.value?.querySelector<HTMLElement>(`[data-row="${i}"]`);

function focusRow(i: number): void {
  cursor.value = i;
  void nextTick(() => rowElement(i)?.focus());
}

function onFocusIn(e: FocusEvent): void {
  const row = (e.target as Element).closest<HTMLElement>('[data-row]');
  if (row) cursor.value = Number(row.dataset.row);
}

async function setHero(row: Row): Promise<void> {
  const hero = heroFace.value;
  if (row.stop) {
    if (hero && rampAxis.value) axesSet.set(hero.id, rampAxis.value, row.stop.value);
    return;
  }
  emit('choose', row.sampleFace);
  try {
    if (row.pin) {
      const axes = await faceAxes(row.pin.face);
      axesSet.applyInstance(row.pin.face, axes, row.pin);
    } else if (row.face?.isVariable) {
      const wght = (await faceAxes(row.face.id)).find((a) => a.tag === 'wght');
      if (wght) axesSet.set(row.face.id, wght, row.face.weight);
    }
  } catch (e) {
    toast.show(errorMessage(e));
  }
}

/** A click anywhere on a row but its own controls. */
function onRowClick(row: Row, e: MouseEvent): void {
  if (!(e.target as Element).closest('button')) void setHero(row);
}

function onKey(e: KeyboardEvent): void {
  if (e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return;
  const i = Number((e.target as Element).closest<HTMLElement>('[data-row]')?.dataset.row ?? -1);
  const row = rows.value[i];
  if (!row) return;
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    const next = i + (e.key === 'ArrowDown' ? 1 : -1);
    if (rows.value[next]) focusRow(next);
    return;
  }
  // On a control inside the row these keys are that control's own.
  if (!(e.target as Element).matches('[data-row]')) return;
  if (e.key === ' ') {
    e.preventDefault();
    if (row.face && !e.repeat) void page.toggleFace(row.face);
  } else if (e.key === 'Enter') {
    e.preventDefault();
    if (!e.repeat) void setHero(row);
  }
}

/** What a row's Copy CSS writes: the coordinates its sample shows, as fm-core builds them. */
async function css(face: FaceId, variations: readonly AxisValue[]): Promise<string> {
  const got = await useClient().cssValues({ face, variations: [...variations], features: [] });
  return got.variationSettings;
}
const rowCss = (row: Row) =>
  css(row.sampleFace, row.pin ? row.pin.coordinates : row.face?.isVariable ? moved.value : []);

// The row menu of a pin. As in the sidebar's menu the focus goes into the layer, which lives in
// <body>, so its keys never reach the row and must not reach the page either.
const menuPin = ref<VirtualStyle | null>(null);
const menuAnchor = ref<HTMLElement | null>(null);
const menuEl = ref<HTMLElement | null>(null);

function openMenu(pin: VirtualStyle, e: MouseEvent): void {
  if (menuPin.value?.id === pin.id) {
    closeMenu();
    return;
  }
  menuAnchor.value = e.currentTarget as HTMLElement;
  menuPin.value = pin;
}

function focusMenu(): void {
  menuEl.value?.querySelector('button')?.focus();
}

function closeMenu(): void {
  menuPin.value = null;
}

function onMenuKey(e: KeyboardEvent): void {
  e.stopPropagation();
  if (e.key === 'Tab') {
    closeMenu();
  } else if (e.key === 'Escape') {
    e.preventDefault();
    closeMenu();
    menuAnchor.value?.focus();
  }
}

async function deleteMenuPin(): Promise<void> {
  const pin = menuPin.value;
  if (!pin) return;
  closeMenu();
  await page.deletePin(pin);
  if (rows.value.length) focusRow(current.value);
}
</script>

<template>
  <div class="family-styles">
    <div v-if="views.length" class="head">
      <UiSegmentedControl
        :model-value="view"
        :options="views"
        :label="t('family.stylesView')"
        wrap
        @update:model-value="page.stylesView = $event"
      />
      <p class="hint">{{ hint }}</p>
    </div>
    <div
      ref="grid"
      class="rows"
      role="grid"
      data-testid="family-styles"
      :aria-label="t('family.stylesList')"
      @keydown="onKey"
      @focusin="onFocusIn"
    >
      <div
        v-for="(row, i) in rows"
        :key="row.key"
        class="row"
        :class="{ ring: row.ring }"
        role="row"
        :aria-label="rowLabel(row)"
        :aria-current="row.ring ? 'true' : undefined"
        :tabindex="i === current ? 0 : -1"
        :data-row="i"
        :data-testid="row.pin ? 'family-pin' : row.stop ? 'family-ramp-row' : 'family-style-row'"
        @click="onRowClick(row, $event)"
      >
        <div class="label" role="gridcell">
          <span class="title">
            {{ row.title }}
            <UiBadge v-if="row.pin">{{ t('family.yours') }}</UiBadge>
          </span>
          <span class="meta">{{ row.meta }}</span>
        </div>
        <div class="sample" role="gridcell">
          <FamilySample
            class="row-sample"
            :data-testid="row.face ? 'family-style-sample' : undefined"
            aria-hidden="true"
            :face="row.sampleFace"
            :text="text"
            :variation="row.variation"
          />
          <span v-if="!row.stop" class="hover-actions">
            <CopyCss
              :label="t('family.styleCss', { name: row.title })"
              :css="() => rowCss(row)"
              :tabindex="i === current ? 0 : -1"
            />
            <UiButton
              v-if="row.face"
              :aria-label="t('family.compareStyle', { name: row.title })"
              :tabindex="i === current ? 0 : -1"
              @click="compare.pin(row.face)"
            >
              {{ t('compare.title') }}
            </UiButton>
          </span>
        </div>
        <div v-if="!row.stop" class="end" role="gridcell">
          <UiSwitch
            v-if="row.face"
            :checked="row.face.active !== null"
            :aria-label="t('family.styleActive', { name: row.title })"
            :tabindex="i === current ? 0 : -1"
            @click="page.toggleFace(row.face)"
          />
          <UiIconButton
            v-else-if="row.pin"
            icon="more"
            :label="t('family.pinMenu', { name: row.title })"
            aria-haspopup="menu"
            :aria-expanded="menuPin?.id === row.pin.id"
            :tabindex="i === current ? 0 : -1"
            @click="openMenu(row.pin, $event)"
          />
        </div>
      </div>
    </div>
    <UiPopover :open="menuPin !== null" :anchor="menuAnchor" @close="closeMenu" @placed="focusMenu">
      <div
        v-if="menuPin"
        ref="menuEl"
        class="menu"
        role="menu"
        :aria-label="t('family.pinMenu', { name: menuPin.name })"
        @keydown="onMenuKey"
      >
        <button
          type="button"
          role="menuitem"
          :aria-label="t('family.deletePin', { name: menuPin.name })"
          @click="deleteMenuPin"
        >
          {{ t('common.delete') }}
        </button>
      </div>
    </UiPopover>
  </div>
</template>

<style scoped>
.head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-2) var(--fm-space-3);
  margin-block-end: var(--fm-space-2);
}
.hint {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.row {
  position: relative;
  display: grid;
  grid-template-columns: 150px minmax(0, 1fr) auto;
  align-items: center;
  column-gap: var(--fm-space-3);
  min-height: 72px;
  padding: var(--fm-space-2) var(--fm-space-3);
  border-bottom: 1px solid var(--fm-border);
  cursor: pointer;
}
/* A drawn layer and not the row's border: the row keeps its size, and the focus ring its outline. */
.row.ring::after {
  content: '';
  position: absolute;
  inset: 0;
  border: 2px solid var(--fm-accent);
  border-radius: var(--fm-radius-md);
  pointer-events: none;
}
.label {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.title {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-1);
  overflow-wrap: anywhere;
}
.meta {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  font-variant-numeric: tabular-nums;
}
/* The actions lie over the end of the sample, so the sample keeps the room while they are hidden. */
.sample {
  display: grid;
  align-items: center;
  min-width: 0;
}
.sample > * {
  grid-area: 1 / 1;
}
.row-sample {
  font-size: 38px;
  line-height: 1.3;
}
.hover-actions {
  display: flex;
  align-items: center;
  justify-self: end;
  gap: var(--fm-space-1);
  padding-inline-start: var(--fm-space-2);
  background-color: var(--fm-bg);
  opacity: 0;
}
.row:hover .hover-actions,
.row:focus-within .hover-actions {
  opacity: 1;
}
@media (hover: none) {
  .hover-actions {
    opacity: 1;
  }
}
.end {
  display: flex;
  align-items: center;
}
.menu button {
  display: block;
  width: 100%;
  height: 28px;
  padding: 0 var(--fm-space-2);
  border: 0;
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  text-align: start;
  white-space: nowrap;
}
</style>
