<script setup lang="ts">
// Compare: the pinned faces (2–4) in the list's current sample text at one size,
// as a sheet over the list. Stacked (the default) shows one row per face and Grid two
// faces per row (E48.2; stored as the API's `sideBySide`); Overlay draws two of them on
// one shared baseline, each in its own color at 50% opacity. Every face is live text in
// the webview, registered through the shared face loader while the sheet shows it.
//
// Named comparisons (E13.2): the pins, mode and sample text are saved under a name;
// a saved one is reopened from the list in the sheet (or the command palette) and
// shows the fonts left, naming the ones the library no longer has.
//
// Pairings (E23.4): a panel below the faces suggests fonts that go with the first
// pin, each with an "Add to compare" button.
//
// A bottom sheet (E48.1): it slides up over the wall, which stays visible above it under
// a 28 % scrim. A handle on its top edge resizes it from 40 % to the whole window; a
// double-click on the handle or the maximize button toggles full height, where the
// radius and the scrim go away. The height is a view preference (`compareSheet` store);
// Auto's specimen size follows it.
//
// A modal dialog: Tab cycles inside, plain keys do not reach the list's
// shortcuts, and Escape or Close returns focus to where it was (the list row).
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import type { ComparisonId, FaceId, FaceSummary } from '../api/types';
import { locale, t } from '../i18n';
import { sheetReturnTarget } from '../keyboard/returnFocus';
import { fontNameSample, sampleText } from '../preview/sampleText';
import { useFaceLoader } from '../preview/faceLoader';
import { FmFade, FmSlide } from '../motion';
import { SHEET_HEIGHT, heightAtPointer, useCompareSheetStore } from '../stores/compareSheet';
import {
  COMPARE_MODES,
  COMPARE_UNDO,
  MAX_PINS,
  useCompareStore,
  type CompareMode,
} from '../stores/compare';
import { matches } from '../keyboard/keymap';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import CompareToggle from './CompareToggle.vue';
import PairingsPanel from './PairingsPanel.vue';
import UiBadge from './ui/UiBadge.vue';
import UiButton from './ui/UiButton.vue';
import UiIcon from './ui/UiIcon.vue';
import UiIconButton from './ui/UiIconButton.vue';
import UiSegmentedControl, { type SegmentOption } from './ui/UiSegmentedControl.vue';
import UiSelect from './ui/UiSelect.vue';

const compare = useCompareStore();
const sheet = useCompareSheetStore();
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

// The saved comparisons: a list under the bookmark button, inside the sheet.
const savedOpen = ref(false);
const savedButton = ref<InstanceType<typeof UiIconButton> | null>(null);
const savedPanel = ref<HTMLElement | null>(null);
const savedButtonEl = () => (savedButton.value?.$el as HTMLElement | undefined) ?? null;

function toggleSaved(): void {
  savedOpen.value = !savedOpen.value;
}
/** Closes the list; `refocus` gives the focus back to its button (Escape). */
function closeSaved(refocus: boolean): void {
  savedOpen.value = false;
  if (refocus) savedButtonEl()?.focus();
}
function onSavedPointer(e: PointerEvent): void {
  const target = e.target as Node | null;
  if (!target || savedPanel.value?.contains(target) || savedButtonEl()?.contains(target)) return;
  closeSaved(false);
}
watch(savedOpen, (open) => {
  if (open) document.addEventListener('pointerdown', onSavedPointer, true);
  else document.removeEventListener('pointerdown', onSavedPointer, true);
});

/** The base: the first pin, the face the others are compared to. */
const countLabel = computed(() => {
  const base = compare.base;
  const count = compare.pins.length;
  return base
    ? t('compare.setCountBase', { count, max: MAX_PINS, name: fontNameSample(base) })
    : t('compare.setCount', { count, max: MAX_PINS });
});

// The height: a drag on the handle sets it live and saves it when the drag ends.
const dragging = ref(false);
const heightPercent = computed(() => Math.round(sheet.height * 100));

function onHandleDown(e: PointerEvent): void {
  if (e.button !== 0) return;
  e.preventDefault();
  dragging.value = true;
  (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
}
function onHandleMove(e: PointerEvent): void {
  if (dragging.value) sheet.setHeight(heightAtPointer(e.clientY, window.innerHeight));
}
function onHandleUp(e: PointerEvent): void {
  if (!dragging.value) return;
  dragging.value = false;
  (e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId);
  sheet.save();
}
/** Arrow keys move the handle by 5 %; Home is the lowest height, End the full one. */
function onHandleKey(e: KeyboardEvent): void {
  const step = 0.05;
  const next =
    e.key === 'ArrowUp'
      ? sheet.height + step
      : e.key === 'ArrowDown'
        ? sheet.height - step
        : e.key === 'Home'
          ? SHEET_HEIGHT.min
          : e.key === 'End'
            ? SHEET_HEIGHT.max
            : null;
  if (next === null) return;
  e.preventDefault();
  sheet.setHeight(next);
  sheet.save();
}

/** Reopens a saved comparison; the focus goes to its mode, as when the sheet opens. */
async function onReopen(id: ComparisonId): Promise<void> {
  savedOpen.value = false;
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
  document.removeEventListener('pointerdown', onSavedPointer, true);
  window.removeEventListener('keydown', onOutsideKey, true);
  document.removeEventListener('focusin', onFocusIn);
  for (const id of held) loader.release(id);
  held.clear();
  if (returnFocus?.isConnected) returnFocus.focus();
});

/** Removes a pin and keeps the focus in the sheet: on the next face's icon, else the mode. */
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

// The sheet is a specimen: Auto grows with the sheet, 56 px at 40 % to 80 px at full
// height. A chosen step overrides it for this sheet.
const sizeChoice = ref<'auto' | '48' | '72' | '96'>('auto');
const sizeOptions = computed<SegmentOption<'auto' | '48' | '72' | '96'>[]>(() => [
  { value: 'auto', label: t('compare.sizeAuto') },
  { value: '48', label: '48' },
  { value: '72', label: '72' },
  { value: '96', label: '96' },
]);
const size = computed(() =>
  sizeChoice.value === 'auto' ? sheet.autoSize : Number(sizeChoice.value),
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

/** A field that types text (the comparison's name) keeps Ctrl/Cmd+Z for its own undo. */
function typesText(target: EventTarget | null): boolean {
  return target instanceof HTMLInputElement && target.type === 'text';
}

/** Tab cycles inside the sheet; Escape closes it; plain keys stay out of the list's shortcuts. */
function onKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    if (savedOpen.value) closeSaved(true);
    else compare.close();
    return;
  }
  // Ctrl/Cmd+Z: the Undo of 'Only this', whose toast the focus trap keeps out of reach. A
  // field that types text keeps the key for its own undo.
  if (matches(e, COMPARE_UNDO) && !typesText(e.target) && compare.undoOnlyThis()) {
    e.preventDefault();
    e.stopPropagation();
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
  <div class="compare-layer">
    <FmFade appear reduced="fade">
      <div
        class="scrim"
        :class="{ clear: sheet.full }"
        data-testid="compare-scrim"
        aria-hidden="true"
        @mousedown="compare.close"
      />
    </FmFade>
    <FmSlide from="bottom" distance="100%" reduced="fade" appear>
      <div
        ref="dialog"
        class="sheet"
        :class="{ full: sheet.full, dragging }"
        :style="{ height: `${sheet.height * 100}%` }"
        role="dialog"
        aria-modal="true"
        aria-labelledby="fm-compare-sheet-title"
        tabindex="-1"
        data-testid="compare-sheet"
        @keydown="onKeyDown"
      >
        <header class="head">
          <h2 id="fm-compare-sheet-title">{{ t('compare.title') }}</h2>
          <span class="muted count" data-testid="compare-count">{{ countLabel }}</span>
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
              <UiIcon :name="m.icon" />
              {{ m.label }}
            </label>
          </div>
          <UiSegmentedControl
            v-model="sizeChoice"
            :options="sizeOptions"
            :label="t('compare.sizeLabel')"
          />
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
          <span class="spacer" />
          <UiIconButton
            ref="savedButton"
            icon="bookmark"
            :label="t('compare.savedButton')"
            :aria-expanded="savedOpen ? 'true' : 'false'"
            aria-controls="fm-compare-saved"
            @click="toggleSaved"
          />
          <UiIconButton
            :icon="sheet.full ? 'minimize' : 'maximize'"
            :label="sheet.full ? t('compare.restore') : t('compare.maximize')"
            data-testid="compare-maximize"
            @click="sheet.toggleFull"
          />
          <UiIconButton
            class="close"
            icon="close"
            :label="t('compare.closeLabel')"
            :title="t('compare.closeTitle')"
            @click="compare.close"
          />
          <div
            v-if="savedOpen"
            id="fm-compare-saved"
            ref="savedPanel"
            class="saved"
            data-testid="compare-saved"
          >
            <span id="fm-compare-saved-label" class="muted">{{ t('compare.saved') }}</span>
            <ul v-if="compare.saved.length" aria-labelledby="fm-compare-saved-label">
              <li v-for="c in compare.saved" :key="c.id">
                <button
                  type="button"
                  class="saved-item"
                  :aria-current="compare.current?.id === c.id ? 'true' : undefined"
                  :aria-label="t('compare.openLabel', { name: c.name })"
                  @click="onReopen(c.id)"
                >
                  {{ c.name }} <span class="muted">({{ c.faceCount }})</span>
                </button>
              </li>
            </ul>
            <p v-else class="muted">{{ t('compare.savedEmpty') }}</p>
          </div>
        </header>

        <div class="body">
          <p
            v-if="compare.missing.length"
            class="missing"
            role="status"
            data-testid="compare-missing"
          >
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
                <line
                  class="baseline"
                  x1="0"
                  x2="100%"
                  :y1="overlayBaseline"
                  :y2="overlayBaseline"
                />
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
              <li v-for="(f, slot) in overlayDrawn" :key="f.id" :data-face="f.id">
                <span class="swatch" :class="`color-${slot}`" aria-hidden="true"></span>
                <!-- A real space between the two, for the accessible name. -->
                <span class="legend-family">{{ f.familyName }}</span
                >{{ ' ' }}<span class="legend-style muted">{{ f.style }}</span>
              </li>
            </ul>
          </div>

          <ul
            v-else
            class="faces"
            :class="compare.mode"
            :aria-label="t(`compare.modes.${compare.mode}`)"
          >
            <li v-for="f in compare.pins" :key="f.id" class="face" :data-face="f.id">
              <div class="label">
                <span class="who">
                  <span class="name">{{ fontNameSample(f) }}</span>
                  <!-- The base (E48.5): '· base' in the accent, its word on-accent (DESIGN.md). -->
                  <template v-if="f.id === compare.base?.id">
                    <span aria-hidden="true">·</span>
                    <UiBadge tone="accent" data-testid="face-base">{{
                      t('compare.baseTag')
                    }}</UiBadge>
                  </template>
                </span>
                <!-- On hover or focus: 'Only this' keeps the face alone (E48.5); the filled
                     compare icon removes it (E48.4). -->
                <span class="face-actions">
                  <UiButton
                    v-if="compare.pins.length > 1"
                    data-testid="face-only"
                    :aria-label="t('compare.onlyThisLabel', { name: fontNameSample(f) })"
                    @click="compare.onlyThis(f)"
                    >{{ t('compare.onlyThis') }}</UiButton
                  >
                  <CompareToggle
                    manual
                    :data-remove="f.id"
                    data-testid="face-remove"
                    :face="f"
                    @toggle="remove(f.id)"
                  />
                </span>
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

        <!-- Last in the Tab order: the controls come first, as before the sheet resized. -->
        <div
          class="handle"
          role="separator"
          aria-orientation="horizontal"
          :aria-label="t('compare.resizeLabel')"
          :title="t('compare.resizeTitle')"
          aria-valuemin="40"
          aria-valuemax="100"
          :aria-valuenow="heightPercent"
          :aria-valuetext="`${heightPercent} %`"
          tabindex="0"
          data-testid="compare-handle"
          @pointerdown="onHandleDown"
          @pointermove="onHandleMove"
          @pointerup="onHandleUp"
          @pointercancel="onHandleUp"
          @dblclick="sheet.toggleFull"
          @keydown="onHandleKey"
        >
          <span class="grip" aria-hidden="true"></span>
        </div>
      </div>
    </FmSlide>
  </div>
</template>

<style scoped>
/* The layer over the window: the scrim on the wall above, the sheet at the bottom. */
.compare-layer {
  position: fixed;
  inset: 0;
  z-index: var(--fm-z-popover);
}
.scrim {
  position: absolute;
  inset: 0;
  background-color: var(--fm-scrim-sheet);
}
/* At full height the sheet is the whole window: no scrim left to see. */
.scrim.clear {
  background-color: transparent;
}
.sheet {
  /* Two colors that stay apart in both themes and for common color blindness. */
  --fm-compare-0: #1d6fe0;
  --fm-compare-1: #e0451d;
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  border-top: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg) var(--fm-radius-lg) 0 0;
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  box-shadow: var(--fm-shadow-2);
}
.sheet.full {
  border-top: 0;
  border-radius: 0;
  box-shadow: none;
}
.sheet:focus {
  outline: none;
}
.sheet.dragging {
  user-select: none;
}
/* The handle: a grip centered on the top edge, a taller strip to grab. */
.handle {
  position: absolute;
  inset-block-start: 0;
  inset-inline: 0;
  display: flex;
  justify-content: center;
  height: 14px;
  padding-top: 5px;
  cursor: ns-resize;
  touch-action: none;
}
.grip {
  width: 40px;
  height: 4px;
  border-radius: 2px;
  background-color: var(--fm-control-border);
}
.handle:focus-visible {
  outline: none;
}
.handle:focus-visible .grip {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
/* One row: nothing wraps; the name field gives way first. */
.head {
  position: relative;
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  gap: var(--fm-space-3);
  padding: var(--fm-space-4) var(--fm-space-4) var(--fm-space-3);
  white-space: nowrap;
}
.count {
  overflow: hidden;
  min-width: 0;
  text-overflow: ellipsis;
}
.spacer {
  flex: 1;
}
.body {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--fm-space-3);
  min-height: 0;
  padding: 0 var(--fm-space-4) var(--fm-space-4);
  overflow: auto;
}
.save {
  display: flex;
  flex: 0 1 auto;
  align-items: center;
  gap: var(--fm-space-2);
  min-width: 0;
}
.saved {
  position: absolute;
  inset-block-start: 100%;
  inset-inline-end: var(--fm-space-4);
  z-index: 1;
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  min-width: 220px;
  max-height: 320px;
  padding: var(--fm-space-3);
  overflow: auto;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface-2);
  box-shadow: var(--fm-shadow-1);
  white-space: normal;
}
.saved ul {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.saved-item {
  width: 100%;
  padding: var(--fm-space-1) var(--fm-space-2);
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-fg);
  text-align: start;
}
.saved [aria-current='true'] {
  font-weight: 600;
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
  padding: 2px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
}
/* Same segment metrics as `UiSegmentedControl` text segments, which sit beside it (size). */
.mode {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
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
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  box-shadow: 0 0 0 1px var(--fm-accent);
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
  width: 200px;
  min-width: 80px;
  flex: 0 1 200px;
  height: 28px;
  padding: 0 var(--fm-space-2);
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
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
/* Grid (stored as `sideBySide`): two faces per row; 2 faces fill one row, 3–4 a 2×2. */
.faces.sideBySide {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
/* A face's actions come up on hover or focus (E48.4). */
.face-actions {
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
  opacity: 0;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
.face:hover .face-actions,
.face:focus-within .face-actions {
  opacity: 1;
}
.face {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  min-width: 0;
  padding: var(--fm-space-4) var(--fm-space-3);
  border-block-start: 1px solid var(--fm-border);
}
.sideBySide .face:nth-child(even) {
  border-inline-start: 1px solid var(--fm-border);
}
.label {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--fm-space-2);
  color: var(--fm-muted);
}
.who {
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
  min-width: 0;
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
