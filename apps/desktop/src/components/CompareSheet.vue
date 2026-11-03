<script setup lang="ts">
// Compare: the pinned faces (2–4) in the list's current sample text at one size,
// as a sheet over the list. Side by side and Stacked show one block per face;
// Overlay draws two of them on one shared baseline, each in its own color at 50%
// opacity. Every face is live text in the webview, registered through the shared
// face loader while the sheet shows it.
//
// Named comparisons (E13.2): the pins, mode and sample text are saved under a name;
// a saved one is reopened from the list in the sheet (or the command palette) and
// shows the fonts left, naming the ones the library no longer has.
//
// Pairings (E23.4): a panel below the faces suggests fonts that go with the first
// pin, each with an "Add to compare" button.
//
// A modal dialog: Tab cycles inside, plain keys do not reach the list's
// shortcuts, and Escape or Close returns focus to where it was (the list row).
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import type { ComparisonId, FaceId, FaceSummary } from '../api/types';
import { locale, t } from '../i18n';
import { sheetReturnTarget } from '../keyboard/returnFocus';
import { fontNameSample, sampleText } from '../preview/sampleText';
import { useFaceLoader } from '../preview/faceLoader';
import { COMPARE_MODES, MAX_PINS, useCompareStore, type CompareMode } from '../stores/compare';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import PairingsPanel from './PairingsPanel.vue';
import UiButton from './ui/UiButton.vue';
import UiIconButton from './ui/UiIconButton.vue';
import UiSegmentedControl, { type SegmentOption } from './ui/UiSegmentedControl.vue';
import UiSelect from './ui/UiSelect.vue';

const compare = useCompareStore();
const preview = usePreviewStore();
const loader = useFaceLoader();

const dialog = ref<HTMLElement | null>(null);
let returnFocus: HTMLElement | null = null;

const SLOTS = [0, 1] as const;

const settings = computed(() => preview.settingsFor(ALL_FONTS_VIEW));
/** The face's text: the reopened comparison's (empty: the face's name), else the list's. */
const textOf = (face: FaceSummary) =>
  compare.text === null
    ? sampleText(settings.value, face, preview.pangramText)
    : compare.text || fontNameSample(face);

/** The sample text a save keeps: the one shown; empty when each face shows its own name. */
const textToSave = computed(() => {
  if (compare.text !== null) return compare.text;
  const s = settings.value;
  if (s.sample === 'fontName' || (s.sample === 'custom' && !s.customText.trim())) return '';
  const first = compare.pins[0];
  return first ? sampleText(s, first, preview.pangramText) : '';
});

const saveName = ref(compare.current?.name ?? '');
watch(
  () => compare.current,
  (c) => {
    if (c) saveName.value = c.name;
  },
);
const canSave = computed(() => compare.pins.length >= 2 && saveName.value.trim() !== '');

async function onSave(): Promise<void> {
  if (canSave.value) await compare.save(saveName.value, textToSave.value);
}

/** Reopens a saved comparison; the focus goes to its mode, as when the sheet opens. */
async function onReopen(id: ComparisonId): Promise<void> {
  await compare.reopen(id);
  await nextTick();
  dialog.value?.querySelector<HTMLInputElement>('input[name="fm-compare-mode"]:checked')?.focus();
}

// The pinned faces' fonts, registered with the webview while the sheet is open.
const families = reactive(new Map<FaceId, string>());
const held = new Set<FaceId>();
watch(
  () => compare.pins.map((p) => p.id),
  (ids) => {
    for (const id of [...held]) {
      if (ids.includes(id)) continue;
      held.delete(id);
      families.delete(id);
      loader.release(id);
    }
    for (const id of ids) {
      if (held.has(id)) continue;
      held.add(id);
      loader.acquire(id).then(
        (name) => {
          if (held.has(id)) families.set(id, name);
        },
        () => undefined,
      );
    }
  },
  { immediate: true },
);

/** The face's font as a CSS `font-family`, or the UI font until it has loaded. */
const fontOf = (face: FaceSummary) => {
  const name = families.get(face.id);
  return name ? `"${name}", var(--fm-preview-fallback, system-ui), sans-serif` : undefined;
};

onMounted(async () => {
  // Opened from the command palette, the focus is in the palette (or already nowhere,
  // the palette gone): it goes back to where the palette was opened from.
  returnFocus = sheetReturnTarget();
  window.addEventListener('keydown', onOutsideKey, true);
  document.addEventListener('focusin', onFocusIn);
  void compare.loadSaved();
  await nextTick();
  dialog.value?.querySelector<HTMLInputElement>('input[name="fm-compare-mode"]:checked')?.focus();
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onOutsideKey, true);
  document.removeEventListener('focusin', onFocusIn);
  for (const id of held) loader.release(id);
  held.clear();
  if (returnFocus?.isConnected) returnFocus.focus();
});

/** Removes a pin and keeps the focus in the sheet: on the next Remove button, else the mode. */
async function remove(face: FaceId): Promise<void> {
  const buttons = [...(dialog.value?.querySelectorAll<HTMLElement>('[data-remove]') ?? [])];
  const at = buttons.findIndex((b) => b.dataset.remove === String(face));
  compare.unpin(face);
  await nextTick();
  if (!dialog.value) return;
  const left = [...dialog.value.querySelectorAll<HTMLElement>('[data-remove]')];
  const next =
    left[Math.min(at, left.length - 1)] ??
    dialog.value.querySelector<HTMLElement>('input[name="fm-compare-mode"]:checked');
  next?.focus();
}

// The sheet is a specimen: it draws the list's size at twice the scale, within 40–96 px.
// A chosen step overrides it for this sheet; "Auto" follows the list.
const sizeChoice = ref<'auto' | '48' | '72' | '96'>('auto');
const sizeOptions = computed<SegmentOption<'auto' | '48' | '72' | '96'>[]>(() => [
  { value: 'auto', label: t('compare.sizeAuto') },
  { value: '48', label: '48' },
  { value: '72', label: '72' },
  { value: '96', label: '96' },
]);
const size = computed(() =>
  sizeChoice.value === 'auto'
    ? Math.min(96, Math.max(40, settings.value.sizePx * 2))
    : Number(sizeChoice.value),
);
// Overlay: one SVG line box; both texts sit on the same baseline (one `y`).
const overlayHeight = computed(() => Math.ceil(size.value * 1.5));
const overlayBaseline = computed(() => Math.round(size.value * 1.1));
const overlayDrawn = computed(() =>
  compare.overlayPair.filter((f): f is FaceSummary => f !== null),
);
const overlayLabel = computed(() =>
  t('compare.overlayLabel', {
    names: new Intl.ListFormat(locale(), { type: 'conjunction' }).format(
      overlayDrawn.value.map(fontNameSample),
    ),
    size: size.value,
  }),
);

function onMode(e: Event): void {
  compare.setMode((e.target as HTMLInputElement).value as CompareMode);
}
const pinOptions = computed(() =>
  compare.pins.map((p) => ({ value: p.id, label: fontNameSample(p) })),
);

/** Tab cycles inside the sheet; Escape closes it; plain keys stay out of the list's shortcuts. */
function onKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    compare.close();
    return;
  }
  if (!e.ctrlKey && !e.metaKey) e.stopPropagation();
  if (e.key !== 'Tab' || !dialog.value) return;
  // One stop per radio group: the checked radio.
  const stops = [
    ...dialog.value.querySelectorAll<HTMLElement>('input, select, button, [tabindex="0"]'),
  ].filter(
    (el) =>
      !el.hasAttribute('disabled') &&
      !(el instanceof HTMLInputElement && el.type === 'radio' && !el.checked),
  );
  const first = stops[0];
  const last = stops[stops.length - 1];
  if (!first || !last) return;
  // On the sheet itself (after a click on its text): Tab goes to the first control.
  if (document.activeElement === dialog.value) {
    e.preventDefault();
    (e.shiftKey ? last : first).focus();
  } else if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

/**
 * A key whose target is outside the sheet (the focus fell to the page, e.g. after a
 * click in some browsers) is handled as if it were in the sheet: the list's shortcuts
 * behind the modal never see it.
 */
function onOutsideKey(e: KeyboardEvent): void {
  if (!dialog.value || (e.target instanceof Node && dialog.value.contains(e.target))) return;
  // The command palette (Ctrl/Cmd+K) may open over the sheet; it owns its keys.
  if (e.target instanceof Element && e.target.closest('[role="dialog"]')) return;
  if (e.key === 'Tab') {
    e.preventDefault();
    dialog.value.focus();
  }
  onKeyDown(e);
}

/** Focus that moves to the page behind the sheet is taken back to the sheet. */
function onFocusIn(e: FocusEvent): void {
  const target = e.target;
  if (!dialog.value || !(target instanceof Element) || dialog.value.contains(target)) return;
  if (target.closest('[role="dialog"]')) return;
  dialog.value.focus();
}
</script>

<template>
  <div class="backdrop" @mousedown.self="compare.close">
    <div
      ref="dialog"
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="fm-compare-sheet-title"
      tabindex="-1"
      @keydown="onKeyDown"
    >
      <header class="head">
        <h2 id="fm-compare-sheet-title">{{ t('compare.title') }}</h2>
        <span v-if="compare.current" class="current" data-testid="compare-current">
          {{ compare.current.name }}
        </span>
        <span class="muted">{{
          t('compare.pinnedCount', { count: compare.pins.length, max: MAX_PINS })
        }}</span>
        <div class="modes" role="radiogroup" aria-labelledby="fm-compare-mode-label">
          <span id="fm-compare-mode-label" class="sr-only">{{ t('compare.mode') }}</span>
          <label v-for="m in COMPARE_MODES" :key="m.value" class="mode">
            <input
              type="radio"
              name="fm-compare-mode"
              :value="m.value"
              :checked="compare.mode === m.value"
              @change="onMode"
            />
            {{ m.label }}
          </label>
        </div>
        <UiSegmentedControl
          v-model="sizeChoice"
          :options="sizeOptions"
          :label="t('compare.sizeLabel')"
        />
        <UiIconButton
          class="close"
          icon="close"
          :label="t('compare.closeLabel')"
          :title="t('compare.closeTitle')"
          @click="compare.close"
        />
      </header>

      <div class="named">
        <form class="save" @submit.prevent="onSave">
          <input
            v-model="saveName"
            class="field"
            type="text"
            :aria-label="t('compare.nameLabel')"
            :placeholder="t('compare.namePlaceholder')"
            maxlength="200"
          />
          <UiButton type="submit" :disabled="!canSave">{{ t('compare.save') }}</UiButton>
        </form>
        <div v-if="compare.saved.length" class="saved">
          <span id="fm-compare-saved-label" class="muted">{{ t('compare.saved') }}</span>
          <ul aria-labelledby="fm-compare-saved-label">
            <li v-for="c in compare.saved" :key="c.id">
              <UiButton
                :aria-current="compare.current?.id === c.id ? 'true' : undefined"
                :aria-label="t('compare.openLabel', { name: c.name })"
                @click="onReopen(c.id)"
              >
                {{ c.name }} <span class="muted">({{ c.faceCount }})</span>
              </UiButton>
            </li>
          </ul>
        </div>
      </div>

      <p v-if="compare.missing.length" class="missing" role="status" data-testid="compare-missing">
        {{ t('compare.missing', { names: compare.missing.map((m) => m.name).join(', ') }) }}
      </p>

      <p v-if="compare.pins.length < 2" class="muted" role="status">
        {{ t('compare.pinTwo') }}
      </p>

      <div
        v-if="compare.mode === 'overlay'"
        class="overlay"
        role="group"
        :aria-label="t('compare.overlay')"
        data-testid="compare-overlay"
      >
        <div v-if="compare.pins.length > 2" class="picks">
          <label v-for="slot in SLOTS" :key="slot" class="pick">
            <span class="swatch" :class="`color-${slot}`" aria-hidden="true"></span>
            {{ slot === 0 ? t('compare.firstFont') : t('compare.secondFont') }}
            <UiSelect
              :aria-label="slot === 0 ? t('compare.firstFont') : t('compare.secondFont')"
              :model-value="compare.overlayPair[slot]?.id"
              :options="pinOptions"
              @update:model-value="compare.setOverlay(slot, $event)"
            />
          </label>
        </div>
        <div class="stage">
          <svg
            class="drawing"
            role="img"
            :aria-label="overlayLabel"
            width="100%"
            :height="overlayHeight"
          >
            <line class="baseline" x1="0" x2="100%" :y1="overlayBaseline" :y2="overlayBaseline" />
            <text
              v-for="(f, slot) in overlayDrawn"
              :key="f.id"
              :class="['overlay-text', `color-${slot}`]"
              :data-face="f.id"
              x="8"
              :y="overlayBaseline"
              :font-size="size"
              :style="{
                fontFamily: fontOf(f),
                fill: `var(--fm-compare-${slot})`,
                fillOpacity: 0.5,
              }"
            >
              {{ textOf(f) }}
            </text>
          </svg>
        </div>
        <ul class="legend" :aria-label="t('compare.legend')">
          <li v-for="(f, slot) in overlayDrawn" :key="f.id">
            <span class="swatch" :class="`color-${slot}`" aria-hidden="true"></span>
            {{ fontNameSample(f) }}
          </li>
        </ul>
      </div>

      <ul
        v-else
        class="faces"
        :class="compare.mode"
        :aria-label="
          compare.mode === 'stacked' ? t('compare.modes.stacked') : t('compare.modes.sideBySide')
        "
      >
        <li v-for="f in compare.pins" :key="f.id" class="face" :data-face="f.id">
          <div class="label">
            <span class="name">{{ fontNameSample(f) }}</span>
            <UiButton
              :data-remove="f.id"
              :aria-label="t('compare.removeLabel', { name: fontNameSample(f) })"
              @click="remove(f.id)"
            >
              {{ t('common.remove') }}
            </UiButton>
          </div>
          <p
            class="sample"
            dir="auto"
            :style="{
              fontFamily: fontOf(f),
              fontSize: `${size}px`,
              lineHeight: settings.lineHeight,
              letterSpacing: `${settings.trackingEm}em`,
            }"
          >
            {{ textOf(f) }}
          </p>
        </li>
      </ul>

      <PairingsPanel />
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: var(--fm-z-popover);
  display: flex;
  justify-content: center;
  align-items: center;
  background: var(--fm-scrim);
}
.sheet {
  /* Two colors that stay apart in both themes and for common color blindness. */
  --fm-compare-0: #1d6fe0;
  --fm-compare-1: #e0451d;
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
  width: min(1200px, 96vw);
  min-height: min(560px, 90vh);
  max-height: 90vh;
  padding: var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  box-shadow: var(--fm-shadow-2);
  overflow: auto;
}
.sheet:focus {
  outline: none;
}
.head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-3);
}
.current {
  font-weight: 600;
}
.named {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-3);
}
.save {
  display: flex;
  gap: var(--fm-space-2);
}
.saved {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2);
}
.saved ul {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
.saved [aria-current='true'] {
  color: var(--fm-accent);
}
.missing {
  margin: 0;
}
h2 {
  margin: 0;
  font-size: var(--fm-text-lg);
  font-weight: 600;
}
/* A text segmented control: the radios stay real inputs, drawn as the segments. */
.modes {
  display: flex;
  flex: none;
  align-items: center;
  gap: 2px;
  margin-inline: auto;
  padding: 2px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
}
/* Same segment metrics as `UiSegmentedControl` text segments, which sit beside it (size). */
.mode {
  position: relative;
  display: grid;
  place-items: center;
  height: 24px;
  padding: 0 var(--fm-space-2);
  border-radius: var(--fm-radius-sm);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  transition:
    background-color var(--fm-motion-fast) var(--fm-ease),
    color var(--fm-motion-fast) var(--fm-ease);
}
.mode:hover {
  color: var(--fm-fg);
}
.mode:has(input:checked) {
  background-color: var(--fm-bg);
  color: var(--fm-accent);
  box-shadow: 0 0 0 1px var(--fm-border);
}
.mode input {
  position: absolute;
  inset: 0;
  margin: 0;
  opacity: 0;
}
.mode:has(input:focus-visible) {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.muted {
  margin: 0;
  color: var(--fm-muted);
}
.field {
  padding: 2px 0;
  border: 0;
  border-bottom: 1px solid var(--fm-border);
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
}
.field:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.faces {
  display: grid;
  flex: 1;
  align-content: start;
  gap: 0;
  margin: 0;
  padding: 0;
  list-style: none;
}
.faces.sideBySide {
  grid-auto-flow: column;
  grid-auto-columns: minmax(0, 1fr);
}
.face {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  min-width: 0;
  padding: var(--fm-space-4) var(--fm-space-3);
  border-block-start: 1px solid var(--fm-border);
}
.sideBySide .face {
  border-block-start: 0;
  border-inline-start: 1px solid var(--fm-border);
}
.sideBySide .face:first-child {
  border-inline-start: 0;
}
.label {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--fm-space-2);
  color: var(--fm-muted);
}
.name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sample {
  margin: 0;
  overflow-wrap: anywhere;
}
.overlay {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--fm-space-2);
}
.picks {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-3);
}
.pick {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
}
.stage {
  display: flex;
  flex: 1;
  flex-direction: column;
  justify-content: center;
  overflow: hidden;
  padding-block: var(--fm-space-6);
  border-block: 1px solid var(--fm-border);
}
.drawing {
  display: block;
}
.baseline {
  stroke: var(--fm-fg);
  stroke-width: 1;
}
.overlay-text {
  white-space: pre;
}
.legend {
  display: flex;
  gap: var(--fm-space-4);
  margin: 0;
  padding: 0;
  list-style: none;
}
.legend li {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
}
.swatch {
  flex: none;
  width: 14px;
  height: 14px;
  border-radius: var(--fm-radius-sm);
  opacity: 0.5;
}
.swatch.color-0 {
  background: var(--fm-compare-0);
}
.swatch.color-1 {
  background: var(--fm-compare-1);
}
</style>
