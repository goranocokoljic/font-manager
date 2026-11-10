<script setup lang="ts">
// The row above a font list (E41.2): the preview field with its hint, the preset chips,
// the Language picker and one button for the Preview options popover, which holds the rest:
// sample, size, auto size, line height, tracking, the metrics lines and outline toggles,
// alignment, colors and the preview surface. Every change goes to the preview store for
// `view`, which every row of that list reads.
import { computed, onBeforeUnmount, ref, useId, watch } from 'vue';
import { t } from '../i18n';
import { FmSlide } from '../motion';
import { ACTIONS, ariaShortcut, chordOf } from '../keyboard/keymap';
import { wallLayout, type WallPreset } from '../list/grid';
import { languageName } from '../search/chips';
import { useActions } from '../stores/commands';
import { useSearchStore } from '../stores/search';
import UiButton from './ui/UiButton.vue';
import UiIcon from './ui/UiIcon.vue';
import UiIconButton from './ui/UiIconButton.vue';
import { LETTERS_HINT, SAMPLE_KINDS, sampleTextFor } from '../preview/sampleText';
import PreviewSizeSlider from './PreviewSizeSlider.vue';
import UiNumberField from './ui/UiNumberField.vue';
import UiSelect from './ui/UiSelect.vue';
import {
  BODY_SIZE,
  LINE_HEIGHT,
  SIZE,
  TRACKING,
  usePreviewStore,
  WALL_PRESET_IDS,
  type Alignment,
  type AutoSize,
  type PreviewSettings,
  type Surface,
} from '../stores/preview';

const props = defineProps<{
  view: string;
  /** The size slider's range, when the view mode has its own (the wall's 24–120). */
  sizeRange?: { readonly min: number; readonly max: number; readonly step: number };
  /** The fonts the field's text renders in, for its hint. */
  fontCount?: number;
  /** Draws the size slider in the row, for a view with no top bar to hold it. */
  inlineSize?: boolean;
}>();
const emit = defineEmits<{
  /** Escape in the field: the focus goes back to the font list. */
  leave: [];
}>();

const store = usePreviewStore();
const search = useSearchStore();
void store.loadPangrams();
// A select is labelled by `for`: wrapped in its label, its option texts would join the name.
const uid = useId();
const s = computed(() => store.settingsFor(props.view));
/** The popover (line height, tracking, colours…) is one click away, closed by default. */
const optionsOpen = ref(false);
const optionsId = `${uid}-options`;
const toggle = ref<HTMLElement | null>(null);
const panel = ref<HTMLElement | null>(null);

function closeOptions(): void {
  optionsOpen.value = false;
  toggle.value?.querySelector('button')?.focus();
}

/** A press outside closes the popover; a Select's menu in it lives in <body>. */
function onPressOutside(e: PointerEvent): void {
  const target = e.target;
  if (!(target instanceof Element)) return;
  if (panel.value?.contains(target) || toggle.value?.contains(target)) return;
  if (target.closest('[data-ui-popover]')) return;
  optionsOpen.value = false;
}
watch(optionsOpen, (open) => {
  if (open) document.addEventListener('pointerdown', onPressOutside, true);
  else document.removeEventListener('pointerdown', onPressOutside, true);
});
onBeforeUnmount(() => document.removeEventListener('pointerdown', onPressOutside, true));

const field = ref<HTMLInputElement | null>(null);
const fieldChord = chordOf(ACTIONS.previewField)!;

useActions([
  {
    id: ACTIONS.previewField,
    // Getters: the palette shows the title in the language chosen now.
    get title() {
      return t('preview.focusField');
    },
    get group() {
      return t('palette.groups.view');
    },
    // A modal dialog keeps the focus; the field behind it is out of reach.
    enabled: () => !document.querySelector('[role="dialog"][aria-modal="true"]'),
    run: () => {
      field.value?.focus();
      field.value?.select();
    },
  },
]);

/** The Letters preset (E49.7): the field takes glyphs, and the cards show only those. */
const letters = computed(() => s.value.sample === 'letters');

/**
 * The field's text: what the rows show, except that it stays empty for the font names. In
 * Letters it is the glyphs as typed; the wall spaces them out.
 */
const fieldText = computed(() => {
  if (letters.value) return s.value.letters;
  return s.value.sample === 'fontName' ? '' : sampleTextFor(s.value, '', store.pangramText);
});

/**
 * Typing makes the sample custom text and keeps the preset's layout. In Letters it sets the
 * glyphs: the store keeps each once, without spaces, so the field shows what the wall does.
 */
function typeSample(e: Event): void {
  const input = e.target as HTMLInputElement;
  if (!letters.value) {
    set({ sample: 'custom', customText: input.value });
    return;
  }
  set({ letters: input.value });
  // A repeated letter or a space changes nothing in the store, so Vue leaves the field as is.
  if (input.value !== s.value.letters) input.value = s.value.letters;
}

/** A tap-to-add letter of the empty Letters field: it joins the glyphs, the field keeps the focus. */
function addLetter(glyph: string): void {
  set({ letters: s.value.letters + glyph });
  field.value?.focus();
}

/** The Letters hint around its tap-to-add letters: the words before them and after. */
const lettersHint = computed(() => {
  const mark = String.fromCharCode(0);
  const [before = '', after = ''] = t('preview.hints.letters', { letters: mark }).split(mark);
  return { before, after };
});

/** A stock text goes at the first key; the user's own text (or letters) stays to be edited. */
function selectStock(): void {
  if (s.value.sample !== 'custom' && !letters.value) field.value?.select();
}

/** A press on the field's frame (its padding, not the hint's buttons) goes to the input. */
function focusField(e: MouseEvent): void {
  if (e.target === e.currentTarget) field.value?.focus();
}

function leaveField(): void {
  field.value?.blur();
  emit('leave');
}

const chips: readonly WallPreset[] = WALL_PRESET_IDS;

/**
 * The wall's layout now (E43.1): your own long text shows the reading rows, so the
 * Paragraph chip is the one pressed and the size and line height fields are the body's.
 */
const layout = computed(() => wallLayout(s.value.preset, s.value.sample, s.value.customText));
const rows = computed(() => layout.value === 'paragraph');

/**
 * The language the API detects in your own text (E43.3): only while the count line checks
 * this field's text, so the note and the count speak of the same text.
 */
const detectedLanguage = computed(() => {
  const own = s.value.sample === 'custom' ? s.value.customText.trim() : '';
  const best = own && search.coverText === own ? search.detected[0] : undefined;
  return best ? languageName(search.languages, best.code) : '';
});

/** The note at the field's end: what the text on the wall is. */
const hint = computed(() => {
  if (detectedLanguage.value)
    return t('preview.hints.detected', { language: detectedLanguage.value });
  switch (s.value.sample) {
    case 'pangram':
      return t('preview.hints.pangram');
    case 'alphabet':
      return t('preview.hints.alphabet');
    case 'numerals':
      return t('preview.hints.numerals');
    case 'paragraph':
      return t('preview.hints.paragraph', { size: s.value.bodySizePx });
    case 'letters':
      return '';
    default:
      return props.fontCount === undefined
        ? ''
        : t('preview.hints.custom', { count: props.fontCount });
  }
});

function set(patch: Partial<PreviewSettings>): void {
  store.update(props.view, patch);
}

type NumberKey = 'sizePx' | 'lineHeight' | 'trackingEm' | 'bodySizePx' | 'bodyLineHeight';
/** The size field's range: the reading rows' body size, else the view mode's or the default. */
const size = computed(() => (rows.value ? BODY_SIZE : (props.sizeRange ?? SIZE)));
const BOUNDS = computed(() => ({
  sizePx: props.sizeRange ?? SIZE,
  lineHeight: LINE_HEIGHT,
  trackingEm: TRACKING,
  bodySizePx: BODY_SIZE,
  bodyLineHeight: LINE_HEIGHT,
}));
/** The size the field shows: a saved size outside the view mode's range shows at its end. */
const shownSize = computed(() =>
  Math.min(
    size.value.max,
    Math.max(size.value.min, rows.value ? s.value.bodySizePx : s.value.sizePx),
  ),
);
const shownLineHeight = computed(() => (rows.value ? s.value.bodyLineHeight : s.value.lineHeight));
/** The number fields' setting keys, so the template holds no key literals. */
const KEY = computed(
  () =>
    ({
      size: rows.value ? 'bodySizePx' : 'sizePx',
      lineHeight: rows.value ? 'bodyLineHeight' : 'lineHeight',
      tracking: 'trackingEm',
    }) as const,
);

/**
 * While typing, only a value inside the bounds applies (so "1" on the way to "12" does
 * not jump to the minimum). On commit, the store clamps and the field shows the result.
 */
function slid(key: NumberKey, e: Event): void {
  typed(key, (e.target as HTMLInputElement).valueAsNumber, false);
}
function typed(key: NumberKey, value: number | '', commit: boolean): void {
  if (value === '') return;
  const b = BOUNDS.value[key];
  if (commit) set({ [key]: Math.min(b.max, Math.max(b.min, value)) });
  else if (value >= b.min && value <= b.max) set({ [key]: value });
}

// The labels are read where they are shown (`preview.aligns.*`) or when the options are
// built (`preview.autoSizes.*`, `preview.surfaces.*`), so they follow the language.
const alignments: Alignment[] = ['start', 'center', 'end'];
const autoSizes: AutoSize[] = ['off', 'xHeight', 'capHeight'];
const surfaces: Surface[] = ['auto', 'light', 'dark'];
const languageOptions = computed(() =>
  store.languagePicks.map((lang) => ({
    value: lang.code,
    label: lang.name,
    disabled: !lang.pangram,
    title: lang.pangram ? undefined : t('preview.noPangram'),
  })),
);
const autoSizeOptions = computed(() =>
  autoSizes.map((value) => ({ value, label: t(`preview.autoSizes.${value}`) })),
);
const surfaceOptions = computed(() =>
  surfaces.map((value) => ({ value, label: t(`preview.surfaces.${value}`) })),
);

/** What a color well shows while its setting is `null` (the surface's own color). */
function isDarkSurface(): boolean {
  if (s.value.surface !== 'auto') return s.value.surface === 'dark';
  const theme = document.documentElement.dataset.theme;
  if (theme === 'light' || theme === 'dark') return theme === 'dark';
  return globalThis.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
}
const textWell = computed(() => s.value.textColor ?? (isDarkSurface() ? '#ececec' : '#1a1a1a'));
const backgroundWell = computed(
  () => s.value.backgroundColor ?? (isDarkSurface() ? '#1c1c1e' : '#ffffff'),
);

function colorFrom(e: Event): string {
  return (e.target as HTMLInputElement).value;
}
</script>

<template>
  <div class="controls" role="group" :aria-label="t('preview.settings')">
    <div class="main">
      <div class="hero" @click="focusField">
        <label :for="`${uid}-field`" class="sr-only">{{ t('preview.customText') }}</label>
        <input
          :id="`${uid}-field`"
          ref="field"
          type="text"
          dir="auto"
          autocomplete="off"
          spellcheck="false"
          :class="{ letters }"
          :aria-keyshortcuts="ariaShortcut(fieldChord)"
          :value="fieldText"
          :placeholder="letters ? '' : t('preview.heroPlaceholder')"
          @focus="selectStock"
          @input="typeSample"
          @keydown.esc.prevent="leaveField"
        />
        <span
          v-if="letters && !s.letters"
          class="hint letters-hint"
          data-testid="preview-hint"
          @click="focusField"
        >
          {{ lettersHint.before
          }}<button
            v-for="g in LETTERS_HINT"
            :key="g"
            type="button"
            class="add-letter"
            :aria-label="t('preview.addLetter', { letter: g })"
            @click="addLetter(g)"
          >
            {{ g }}</button
          >{{ lettersHint.after }}
        </span>
        <span v-else-if="hint" class="hint" data-testid="preview-hint">{{ hint }}</span>
      </div>
      <div class="chips" role="group" :aria-label="t('preview.preset')">
        <button
          v-for="p in chips"
          :key="p"
          type="button"
          class="chip"
          :aria-pressed="layout === p"
          @click="store.applyPreset(view, p)"
        >
          {{ t(`preview.presets.${p}`) }}
        </button>
      </div>
      <span class="divider" aria-hidden="true" />
      <div class="language">
        <UiIcon name="globe" />
        <label :for="`${uid}-language`" class="sr-only">{{ t('preview.language') }}</label>
        <UiSelect
          :id="`${uid}-language`"
          :model-value="s.language"
          :options="languageOptions"
          :fit="false"
          @update:model-value="store.chooseLanguage(view, $event)"
          @repick="store.chooseLanguage(view, $event)"
        />
      </div>
      <PreviewSizeSlider v-if="inlineSize" :view="view" :size-range="sizeRange" />
      <span ref="toggle" class="options-toggle">
        <UiIconButton
          icon="sliders"
          :label="t('preview.options')"
          :title="t('preview.options')"
          :aria-expanded="optionsOpen"
          :aria-controls="optionsId"
          @click="optionsOpen = !optionsOpen"
          @keydown.esc="optionsOpen && closeOptions()"
        />
      </span>
    </div>
    <FmSlide from="bottom" distance="4px" speed="fast" reduced="fade">
      <div
        v-show="optionsOpen"
        :id="optionsId"
        ref="panel"
        class="more"
        role="group"
        :aria-label="t('preview.options')"
        @keydown.esc="closeOptions"
      >
        <div class="field">
          <label :for="`${uid}-sample`">{{ t('preview.sample') }}</label>
          <UiSelect
            :id="`${uid}-sample`"
            :model-value="s.sample"
            :options="SAMPLE_KINDS"
            @update:model-value="set({ sample: $event })"
          />
        </div>
        <div class="field">
          <span>{{ t('preview.size') }}</span>
          <UiNumberField
            class="number"
            :aria-label="t('preview.sizeInPixels')"
            :min="size.min"
            :max="size.max"
            :step="size.step"
            :model-value="shownSize"
            @update:model-value="typed(KEY.size, $event, false)"
            @commit="typed(KEY.size, $event, true)"
          />
        </div>
        <div class="field">
          <label :for="`${uid}-auto-size`" :title="t('preview.autoSizeHint')">{{
            t('preview.autoSize')
          }}</label>
          <UiSelect
            :id="`${uid}-auto-size`"
            :model-value="s.autoSize"
            :options="autoSizeOptions"
            @update:model-value="set({ autoSize: $event })"
          />
        </div>
        <div class="field">
          <span>{{ t('preview.lineHeight') }}</span>
          <UiNumberField
            class="number"
            :aria-label="t('preview.lineHeight')"
            :min="LINE_HEIGHT.min"
            :max="LINE_HEIGHT.max"
            :step="LINE_HEIGHT.step"
            :model-value="shownLineHeight"
            @update:model-value="typed(KEY.lineHeight, $event, false)"
            @commit="typed(KEY.lineHeight, $event, true)"
          />
        </div>
        <div class="field" role="group" :aria-labelledby="`${uid}-tracking`">
          <span :id="`${uid}-tracking`">{{ t('preview.tracking') }}</span>
          <input
            type="range"
            :aria-label="t('preview.trackingSlider')"
            :min="TRACKING.min"
            :max="TRACKING.max"
            :step="TRACKING.step"
            :value="s.trackingEm"
            @input="slid(KEY.tracking, $event)"
          />
          <UiNumberField
            class="number"
            :aria-label="t('preview.trackingInEm')"
            :min="TRACKING.min"
            :max="TRACKING.max"
            :step="TRACKING.step"
            :model-value="s.trackingEm"
            @update:model-value="typed(KEY.tracking, $event, false)"
            @commit="typed(KEY.tracking, $event, true)"
          />
        </div>
        <label class="field" :title="t('preview.metricsHint')">
          <input
            type="checkbox"
            :checked="s.metrics"
            @change="set({ metrics: ($event.target as HTMLInputElement).checked })"
          />
          <span>{{ t('preview.metrics') }}</span>
        </label>
        <label class="field">
          <input
            type="checkbox"
            :checked="s.outline"
            @change="set({ outline: ($event.target as HTMLInputElement).checked })"
          />
          <span>{{ t('preview.outline') }}</span>
        </label>

        <div class="field" role="group" :aria-label="t('preview.alignment')">
          <UiButton
            v-for="a in alignments"
            :key="a"
            :aria-pressed="s.align === a"
            @click="set({ align: a })"
          >
            {{ t(`preview.aligns.${a}`) }}
          </UiButton>
        </div>

        <label class="field">
          <span>{{ t('preview.text') }}</span>
          <input
            type="color"
            :aria-label="t('preview.textColor')"
            :value="textWell"
            @input="set({ textColor: colorFrom($event) })"
          />
        </label>
        <label class="field">
          <span>{{ t('preview.background') }}</span>
          <input
            type="color"
            :aria-label="t('preview.backgroundColor')"
            :value="backgroundWell"
            @input="set({ backgroundColor: colorFrom($event) })"
          />
        </label>
        <div class="field">
          <label :for="`${uid}-surface`">{{ t('preview.surface') }}</label>
          <UiSelect
            :id="`${uid}-surface`"
            :model-value="s.surface"
            :options="surfaceOptions"
            @update:model-value="set({ surface: $event })"
          />
        </div>
        <UiButton @click="store.reset(view)">
          {{ t('common.reset') }}
        </UiButton>
      </div>
    </FmSlide>
  </div>
</template>

<style scoped>
.controls {
  position: relative;
  display: flex;
  flex-direction: column;
  font-size: var(--fm-text-sm);
}
.main {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2) 12px;
  padding: 14px 26px 6px;
  container-type: inline-size;
}
/* The field's hairline is its only edge: --fm-control-border keeps it at 3:1. */
.hero {
  display: flex;
  flex: 1 1 280px;
  align-items: center;
  gap: 12px;
  box-sizing: border-box;
  min-width: 0;
  height: 44px;
  padding: 0 16px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bg);
}
/* Too narrow for one row: the field takes its own, so the divider never ends a row alone. */
@container (max-width: 900px) {
  .hero {
    flex-basis: 100%;
  }
}
.hero:focus-within {
  outline: 2px solid var(--fm-accent);
  outline-offset: -1px;
}
.hero input {
  flex: 1 1 0;
  min-width: 0;
  padding: 0;
  border: 0;
  background-color: transparent;
  color: inherit;
  font: inherit;
  font-size: var(--fm-text-lg);
}
.hero input:focus {
  outline: none;
}
/* Letters (E49.7): the glyphs stand apart, as on the cards (board 8-letters-wall.png). */
.hero input.letters {
  letter-spacing: 0.5em;
}
.letters-hint {
  display: inline-flex;
  align-items: center;
  white-space: pre;
}
.add-letter {
  min-width: 24px;
  height: 24px;
  margin: 0 1px;
  padding: 0 4px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm, 6px);
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  cursor: pointer;
}
.add-letter:hover {
  background-color: var(--fm-surface);
}
.hint {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  color: var(--fm-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}
.chips {
  display: flex;
  flex: none;
  gap: 6px;
}
.chip {
  height: 28px;
  padding: 0 12px;
  border: 1px solid var(--fm-border-strong);
  border-radius: 999px;
  background-color: transparent;
  color: inherit;
  font: inherit;
  cursor: pointer;
}
.chip:hover {
  background-color: var(--fm-surface);
}
.chip[aria-pressed='true'] {
  border-color: var(--fm-fg);
  background-color: var(--fm-fg);
  color: var(--fm-bg);
}
.divider {
  flex: none;
  width: 1px;
  height: 20px;
  background-color: var(--fm-border);
}
.language {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--fm-space-1);
  color: var(--fm-muted);
}
.options-toggle {
  display: inline-flex;
  flex: none;
}
/* In the group's DOM, not a <body> layer: unlike UiPopover it takes the focus and tab order. */
.more {
  position: absolute;
  top: 100%;
  z-index: var(--fm-z-popover);
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-gap) 16px;
  box-sizing: border-box;
  width: min(400px, calc(100% - 52px));
  max-height: 60vh;
  padding: 12px;
  overflow-y: auto;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface-2);
  box-shadow: var(--fm-shadow-2);
  inset-inline-end: 26px;
}
.field {
  display: flex;
  align-items: center;
  gap: 4px;
}
.field > span,
.field > label {
  color: var(--fm-muted);
}
.field .number {
  width: 7em;
}
input[type='range'] {
  width: 96px;
}
input[type='checkbox'] {
  margin: 0;
  accent-color: var(--fm-accent);
}
input[type='color'] {
  width: 28px;
  height: 22px;
  padding: 0;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
}
.chip:focus-visible,
.more :focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
