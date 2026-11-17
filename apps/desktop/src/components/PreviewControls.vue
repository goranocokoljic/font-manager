<script setup lang="ts">
// The row above a font list (E41.2): the preview field with its hint, the preset chips,
// the Language picker and one button for the Preview options popover (E54.8, board 12),
// which holds the rest in three groups: Text (sample, align), Size and spacing (size, auto
// size, line height, tracking) and Guides (metric lines, outlines). There are no colour
// controls: every preview draws in the theme's text colour on the theme's surface. Every
// change goes to the preview store for `view`, which every row of that list reads.
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from 'vue';
import { locale, t } from '../i18n';
import { FmSlide } from '../motion';
import { ACTIONS, ariaShortcut, chordOf } from '../keyboard/keymap';
import { cardText, wallLayout, type WallPreset } from '../list/grid';
import { languageName } from '../search/chips';
import { chosenValue, traitForGlyph } from '../search/traits';
import { useActions } from '../stores/commands';
import { useSearchStore } from '../stores/search';
import LetterPicker from './LetterPicker.vue';
import UiButton from './ui/UiButton.vue';
import UiIcon from './ui/UiIcon.vue';
import UiIconButton from './ui/UiIconButton.vue';
import { LETTERS_HINT, SAMPLE_KINDS, sampleTextFor } from '../preview/sampleText';
import {
  applicability,
  previewPreset,
  type ControlState,
  type PreviewSurface,
} from '../preview/applicability';
import PreviewSizeSlider from './PreviewSizeSlider.vue';
import UiNumberField from './ui/UiNumberField.vue';
import UiSegmentedControl, { type SegmentOption } from './ui/UiSegmentedControl.vue';
import UiSelect from './ui/UiSelect.vue';
import UiSwitch from './ui/UiSwitch.vue';
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
} from '../stores/preview';

const props = defineProps<{
  view: string;
  /** The size slider's range, when the view mode has its own (the wall's 24–120). */
  sizeRange?: { readonly min: number; readonly max: number; readonly step: number };
  /** The fonts the field's text renders in, for its hint. */
  fontCount?: number;
  /** Draws the size slider in the row, for a view with no top bar to hold it. */
  inlineSize?: boolean;
  /** The view mode the rows are in: it decides where the Language picker applies (E51.4). */
  mode?: PreviewSurface;
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
/** The popover (sample, size, spacing, guides) is one click away, closed by default. */
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
    id: ACTIONS.matchLetters,
    get title() {
      return t('letters.matchByLetters');
    },
    get group() {
      return t('palette.groups.view');
    },
    // Palette only (no key): the palette is the modal open when it runs.
    run: matchByLetters,
  },
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

/** "Match by letters…" (E49.6): the Letters preset, and the field to type the letters in. */
async function matchByLetters(): Promise<void> {
  if (!letters.value) store.applyPreset(props.view, 'letters');
  await nextTick();
  field.value?.focus();
}

// The letters are the controls (E49.6, board 8-letters-picker.png): while the field is not
// being edited, each typed glyph is a button; one whose glyph has a trait the picker offers
// carries a hairline and opens the picker of that trait's values.
watch(
  letters,
  (on) => {
    if (on) void search.loadTraitCatalog();
  },
  { immediate: true },
);
/** The field has the focus: its plain text is shown to edit, not the letter buttons. */
const editing = ref(false);
const glyphs = computed(() => [...s.value.letters]);
const showLetterButtons = computed(
  () => letters.value && glyphs.value.length > 0 && !editing.value,
);
const traitOf = (glyph: string) => traitForGlyph(search.traitCatalog, glyph);
const chosenOf = (glyph: string) => {
  const trait = traitOf(glyph);
  return trait ? chosenValue(search.traits, trait.id) : null;
};
/** The chosen value's label, lower case, for a letter's accessible name. */
const chosenLabel = (glyph: string) => {
  const value = chosenOf(glyph);
  return traitOf(glyph)
    ?.values.find((v) => v.id === value)
    ?.label.toLowerCase();
};
function letterName(glyph: string): string {
  if (!traitOf(glyph)) return glyph;
  const value = chosenLabel(glyph);
  return value
    ? t('letters.letterChosen', { letter: glyph, value })
    : t('letters.letterShape', { letter: glyph });
}

const pickerGlyph = ref<string | null>(null);
const pickerAnchor = ref<HTMLElement | null>(null);
const pickerTrait = computed(() => (pickerGlyph.value ? traitOf(pickerGlyph.value) : undefined));

/** A letter with a trait opens its picker; one without lets you edit the letters. */
function pressLetter(glyph: string, e: MouseEvent): void {
  if (!traitOf(glyph)) {
    field.value?.focus();
    return;
  }
  if (pickerGlyph.value === glyph) {
    closePicker();
    return;
  }
  pickerAnchor.value = e.currentTarget as HTMLElement;
  pickerGlyph.value = glyph;
}

/** Closes the picker; the focus goes back to its letter. */
function closePicker(): void {
  const letter = pickerAnchor.value;
  pickerGlyph.value = null;
  void nextTick(() => letter?.focus());
}

function chooseShape(value: string | null): void {
  const trait = pickerTrait.value;
  if (trait) search.chooseTraitValue(trait.id, value);
}

/** The glyphs whose shape is chosen, as a list ("R and O"). */
const narrowed = computed(() => glyphs.value.filter((g) => chosenOf(g) !== null));
const letterHint = computed(() => {
  if (!glyphs.value.some((g) => traitOf(g))) return '';
  if (!narrowed.value.length) return t('letters.hintPick');
  const list = new Intl.ListFormat(locale(), { type: 'conjunction' }).format(narrowed.value);
  return t('letters.hintNarrowed', { letters: list });
});

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
 * The Language picker applies to the Pangram preset only (E51.4): elsewhere it stays in its
 * place, off, and its tooltip says to switch to Pangram. The stored language is kept.
 */
const languageState = computed(
  () => applicability(props.mode ?? 'list', previewPreset(s.value)).language,
);
const languageWhy = computed(() =>
  languageState.value.applies ? undefined : t(languageState.value.reason),
);

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
      return letterHint.value;
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

// The labels are read when the options are built, so they follow the language.
const ALIGN_ICONS = { start: 'alignStart', center: 'alignCenter', end: 'alignEnd' } as const;
const alignOptions = computed<SegmentOption<Alignment>[]>(() =>
  (['start', 'center', 'end'] as const).map((value) => ({
    value,
    label: t(`preview.aligns.${value}`),
    icon: ALIGN_ICONS[value],
  })),
);
const autoSizeOptions = computed<SegmentOption<AutoSize>[]>(() =>
  (['off', 'xHeight', 'capHeight'] as const).map((value) => ({
    value,
    label: t(`preview.autoSizes.${value}`),
  })),
);
const languageOptions = computed(() =>
  store.languagePicks.map((lang) => ({
    value: lang.code,
    label: lang.name,
    disabled: !lang.pangram,
    title: lang.pangram ? undefined : t('preview.noPangram'),
  })),
);

/**
 * Where the popover's Size and Line height have an effect (E54.8). Size: in the popover it
 * sets the body size of the reading rows too, so only the view mode can turn it off (the
 * table's strip, the fixed waterfall, the layout pane); the preset is read as Word for that.
 * Line height: off on the wall while every card is a single line.
 */
const sizeState = computed(() => applicability(props.mode ?? 'list', 'word').size);
const lineHeightState = computed(
  () => applicability(props.mode ?? 'list', previewPreset(s.value)).lineHeight,
);

/** One slider row of Size and spacing: a range input and a number field with the same value. */
interface SliderRow {
  id: 'size' | 'lineHeight' | 'tracking';
  label: string;
  slider: string;
  field: string;
  key: NumberKey;
  bounds: { readonly min: number; readonly max: number; readonly step: number };
  value: number;
  decimals?: number;
  unit?: string;
  state: ControlState;
}
const sliderRows = computed<SliderRow[]>(() => [
  {
    id: 'size',
    label: t('preview.size'),
    // Not "Size slider": that names the toolbar's, which sizes the same setting.
    slider: t('preview.textSizeSlider'),
    field: t('preview.sizeInPixels'),
    key: KEY.value.size,
    bounds: size.value,
    value: shownSize.value,
    unit: t('preview.units.px'),
    state: sizeState.value,
  },
  {
    id: 'lineHeight',
    label: t('preview.lineHeight'),
    slider: t('preview.lineHeightSlider'),
    field: t('preview.lineHeight'),
    key: KEY.value.lineHeight,
    bounds: LINE_HEIGHT,
    value: shownLineHeight.value,
    decimals: 2,
    state: lineHeightState.value,
  },
  {
    id: 'tracking',
    label: t('preview.tracking'),
    slider: t('preview.trackingSlider'),
    field: t('preview.trackingInEm'),
    key: KEY.value.tracking,
    bounds: TRACKING,
    value: s.value.trackingEm,
    decimals: 2,
    unit: t('preview.units.em'),
    state: { applies: true },
  },
]);
/** Size and spacing in the board's order: Size, Auto size, Line height, Tracking. */
const rowsInOrder = computed<Array<SliderRow | 'autoSize'>>(() => {
  const [size, ...spacing] = sliderRows.value;
  return [size!, 'autoSize', ...spacing];
});

/** The Guides group: each a switch with a one-line description under its label. */
const guides = computed(() => [
  { key: 'metrics' as const, label: t('preview.metrics'), hint: t('preview.metricsHint') },
  { key: 'outline' as const, label: t('preview.outline'), hint: t('preview.outlineHint') },
]);
const why = (row: SliderRow) => (row.state.applies ? undefined : t(row.state.reason));

/** A slider or field whose setting has no effect takes no change; the stored value stays. */
function slideRow(row: SliderRow, e: Event): void {
  const input = e.target as HTMLInputElement;
  if (!row.state.applies) {
    input.valueAsNumber = row.value;
    return;
  }
  slid(row.key, e);
}
function typeRow(row: SliderRow, value: number | '', commit: boolean): void {
  if (row.state.applies) typed(row.key, value, commit);
}
/** Tab and Shift+Tab pass; the keys that would move a slider without effect do nothing. */
function rowKey(row: SliderRow, e: KeyboardEvent): void {
  if (!row.state.applies && e.key !== 'Tab') e.preventDefault();
}
/** A press on a slider without effect neither moves it nor focuses it. */
function rowPointer(row: SliderRow, e: MouseEvent): void {
  if (!row.state.applies) e.preventDefault();
}

/**
 * The popover's live specimen (board 12): the start of the text the rows show, one line, in
 * the UI face, with the alignment, tracking, outlines and metric lines chosen now. Its size
 * is its own: the specimen shows how the settings look, the rows show the size.
 */
const specimenText = computed(() => {
  const text = letters.value
    ? glyphs.value.join(' ')
    : sampleTextFor(s.value, t('preview.options'), store.pangramText);
  return cardText(text, 16) || t('preview.presets.word');
});
</script>

<template>
  <div class="controls" role="group" :aria-label="t('preview.settings')">
    <div class="main">
      <div class="hero" :class="{ 'with-letters': showLetterButtons }" @click="focusField">
        <span
          v-if="showLetterButtons"
          class="letter-row"
          role="group"
          :aria-label="t('letters.row')"
          data-testid="letter-row"
        >
          <button
            v-for="g in glyphs"
            :key="g"
            type="button"
            class="letter"
            :class="{ shape: !!traitOf(g), chosen: chosenOf(g) !== null }"
            :title="traitOf(g) ? t('letters.shape') : undefined"
            :aria-label="letterName(g)"
            :aria-haspopup="traitOf(g) ? 'dialog' : undefined"
            :aria-expanded="traitOf(g) ? pickerGlyph === g : undefined"
            @click="pressLetter(g, $event)"
          >
            {{ g }}
          </button>
        </span>
        <label :for="`${uid}-field`" class="sr-only">{{ t('preview.customText') }}</label>
        <input
          :id="`${uid}-field`"
          ref="field"
          type="text"
          dir="auto"
          autocomplete="off"
          spellcheck="false"
          :class="{ letters, behind: showLetterButtons, empty: letters && !s.letters }"
          :aria-keyshortcuts="ariaShortcut(fieldChord)"
          :value="fieldText"
          :placeholder="letters ? '' : t('preview.heroPlaceholder')"
          @focus="
            editing = true;
            selectStock();
          "
          @blur="editing = false"
          @input="typeSample"
          @keydown.esc.prevent="leaveField"
        />
        <span
          v-if="letters && !s.letters"
          class="hint letters-hint"
          data-testid="preview-hint"
          @click="field?.focus()"
        >
          <span class="hint-text">{{ lettersHint.before }}</span>
          <button
            v-for="g in LETTERS_HINT"
            :key="g"
            type="button"
            class="add-letter"
            :aria-label="t('preview.addLetter', { letter: g })"
            @click="addLetter(g)"
          >
            {{ g }}
          </button>
          <span class="hint-text tail">{{ lettersHint.after }}</span>
        </span>
        <span v-else-if="hint && !(letters && editing)" class="hint" data-testid="preview-hint">{{
          hint
        }}</span>
        <LetterPicker
          v-if="pickerTrait"
          :open="pickerGlyph !== null"
          :anchor="pickerAnchor"
          :trait="pickerTrait"
          :chosen="chosenValue(search.traits, pickerTrait.id)"
          @choose="chooseShape"
          @close="closePicker"
        />
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
      <div
        class="language"
        :class="{ off: !languageState.applies }"
        :title="languageWhy"
        data-testid="language-picker"
      >
        <UiIcon name="globe" />
        <label :for="`${uid}-language`" class="sr-only">{{ t('preview.language') }}</label>
        <UiSelect
          :id="`${uid}-language`"
          :model-value="s.language"
          :options="languageOptions"
          :fit="false"
          :inactive="!languageState.applies"
          :title="languageWhy"
          :aria-describedby="languageWhy ? `${uid}-language-why` : undefined"
          @update:model-value="store.chooseLanguage(view, $event)"
          @repick="store.chooseLanguage(view, $event)"
        />
        <span v-if="languageWhy" :id="`${uid}-language-why`" class="sr-only">{{
          languageWhy
        }}</span>
      </div>
      <PreviewSizeSlider
        v-if="inlineSize"
        :view="view"
        :size-range="sizeRange"
        :mode="mode ?? 'list'"
      />
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
        :aria-labelledby="`${uid}-options-title`"
        data-testid="preview-options"
        @keydown.esc="closeOptions"
      >
        <div class="head">
          <span :id="`${uid}-options-title`" class="title">{{ t('preview.options') }}</span>
          <UiButton @click="store.reset(view)">{{ t('common.reset') }}</UiButton>
        </div>
        <div
          class="specimen"
          :class="{ outline: s.outline }"
          :style="{ justifyContent: s.align, letterSpacing: `${s.trackingEm}em` }"
          aria-hidden="true"
          data-testid="options-specimen"
        >
          <span class="specimen-text" dir="auto">
            <template v-if="s.metrics">
              <span class="guide cap" />
              <span class="guide x" />
              <span class="guide base" />
            </template>
            {{ specimenText }}
          </span>
        </div>

        <section class="group" :aria-labelledby="`${uid}-g-text`">
          <h3 :id="`${uid}-g-text`" class="heading">{{ t('preview.groups.text') }}</h3>
          <div class="row">
            <label :for="`${uid}-sample`" class="label">{{ t('preview.sample') }}</label>
            <UiSelect
              :id="`${uid}-sample`"
              class="wide"
              :model-value="s.sample"
              :options="SAMPLE_KINDS"
              @update:model-value="set({ sample: $event })"
            />
          </div>
          <div class="row">
            <span class="label" aria-hidden="true">{{ t('preview.align') }}</span>
            <UiSegmentedControl
              class="align"
              :model-value="s.align"
              :options="alignOptions"
              :label="t('preview.align')"
              @update:model-value="set({ align: $event })"
            />
          </div>
        </section>

        <section class="group" :aria-labelledby="`${uid}-g-size`">
          <h3 :id="`${uid}-g-size`" class="heading">{{ t('preview.groups.sizeSpacing') }}</h3>
          <template v-for="row in rowsInOrder" :key="row === 'autoSize' ? row : row.id">
            <div v-if="row === 'autoSize'" class="row">
              <span class="label" aria-hidden="true" :title="t('preview.autoSizeHint')">{{
                t('preview.autoSize')
              }}</span>
              <UiSegmentedControl
                :model-value="s.autoSize"
                :options="autoSizeOptions"
                :label="t('preview.autoSize')"
                :title="t('preview.autoSizeHint')"
                @update:model-value="set({ autoSize: $event })"
              />
            </div>
            <template v-else>
              <!-- A group per row, so a row without effect is one disabled control: its faded
                   label is part of an inactive component (WCAG 1.4.3 exempts it). -->
              <div
                class="row slider-row"
                :class="{ off: !row.state.applies }"
                role="group"
                :aria-labelledby="`${uid}-${row.id}-label`"
                :aria-disabled="row.state.applies ? undefined : 'true'"
                :data-testid="`options-${row.id}`"
              >
                <span :id="`${uid}-${row.id}-label`" class="label" aria-hidden="true">{{
                  row.label
                }}</span>
                <input
                  type="range"
                  :aria-label="row.slider"
                  :aria-disabled="row.state.applies ? undefined : 'true'"
                  :aria-describedby="why(row) ? `${uid}-${row.id}-why` : undefined"
                  :min="row.bounds.min"
                  :max="row.bounds.max"
                  :step="row.bounds.step"
                  :value="row.value"
                  @input="slideRow(row, $event)"
                  @keydown="rowKey(row, $event)"
                  @pointerdown="rowPointer(row, $event)"
                  @mousedown="rowPointer(row, $event)"
                />
                <UiNumberField
                  class="number"
                  :aria-label="row.field"
                  :aria-describedby="why(row) ? `${uid}-${row.id}-why` : undefined"
                  :min="row.bounds.min"
                  :max="row.bounds.max"
                  :step="row.bounds.step"
                  :decimals="row.decimals"
                  :inactive="!row.state.applies"
                  :model-value="row.value"
                  @update:model-value="typeRow(row, $event, false)"
                  @commit="typeRow(row, $event, true)"
                />
                <span class="unit" aria-hidden="true">{{ row.unit }}</span>
              </div>
              <p v-if="why(row)" :id="`${uid}-${row.id}-why`" class="why">{{ why(row) }}</p>
            </template>
          </template>
        </section>

        <section class="group" :aria-labelledby="`${uid}-g-guides`">
          <h3 :id="`${uid}-g-guides`" class="heading">{{ t('preview.groups.guides') }}</h3>
          <div v-for="g in guides" :key="g.key" class="row toggle-row">
            <span class="toggle-text">
              <span :id="`${uid}-${g.key}`">{{ g.label }}</span>
              <span :id="`${uid}-${g.key}-hint`" class="desc">{{ g.hint }}</span>
            </span>
            <UiSwitch
              :checked="s[g.key]"
              :aria-labelledby="`${uid}-${g.key}`"
              :aria-describedby="`${uid}-${g.key}-hint`"
              @click="set({ [g.key]: !s[g.key] })"
            />
          </div>
        </section>
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
.hero.with-letters {
  position: relative;
}
/* The note sits at the field's end, after the letters (board 8-letters-picker.png). */
.hero.with-letters .hint {
  margin-inline-start: auto;
}
/* The letters as buttons (E49.6): 20 px Inter 500, each its own control. */
.letter-row {
  position: relative;
  z-index: 1;
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.letter {
  position: relative;
  min-width: 28px;
  height: 32px;
  padding: 0 4px;
  border: 0;
  border-radius: 0;
  background-color: transparent;
  color: inherit;
  font-family: var(--fm-font-ui);
  font-size: 20px;
  font-weight: 500;
  line-height: 1;
  cursor: text;
}
/* The hairline is drawn, not a border: a 1.5 px border snaps to 1 px. */
.letter.shape::after {
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  height: 1.5px;
  background-color: color-mix(in srgb, var(--fm-fg) 22%, transparent);
  content: '';
}
.letter.shape {
  cursor: pointer;
}
.letter.shape:hover::after {
  background-color: color-mix(in srgb, var(--fm-fg) 60%, transparent);
}
.letter.chosen::after {
  background-color: var(--fm-accent);
}
.letter:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
/* Behind the letter buttons the field still takes a press beside them, and typing. */
.hero input.behind {
  position: absolute;
  inset: 0;
  opacity: 0;
}
.hero:focus-within {
  outline: 2px solid var(--fm-accent);
  outline-offset: -1px;
}
/* The text keeps more than half of a narrow field: the note beside it gives way first. */
.hero input {
  flex: 1 0 55%;
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
/* No glyph yet: the hint is the content, the field only holds the caret. */
.hero input.empty {
  flex: 1 1 0;
  min-width: 1ch;
}
/* The hint's words are part of the field: a press on them starts the typing. */
.letters-hint {
  display: inline-flex;
  align-items: center;
  white-space: pre;
  cursor: text;
}
/* In a narrow field the words give way, the tail first; the five letters always stay. */
.hint-text {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}
.hint-text.tail {
  flex-shrink: 1000;
}
.add-letter {
  flex: none;
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
/* Off (E51.4): the same place and size, at 40 %. */
.language.off {
  opacity: 0.4;
}
.options-toggle {
  display: inline-flex;
  flex: none;
}
/* In the group's DOM, not a <body> layer: unlike UiPopover it takes the focus and the tab
   order, and it is taller than UiPopover's 320 px cap. Board 12: 424 px wide, 16 px inset. */
.more {
  position: absolute;
  top: 100%;
  z-index: var(--fm-z-popover);
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  width: min(424px, calc(100% - 52px));
  max-height: calc(100vh - 160px);
  padding: 12px 16px 14px;
  overflow-y: auto;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface-2);
  color: var(--fm-fg);
  box-shadow: var(--fm-shadow-2);
  font-size: var(--fm-text-md);
  inset-inline-end: 26px;
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 32px;
  margin-inline-end: -8px;
}
.title {
  font-weight: 600;
}
/* The live specimen: the theme's text on the theme's ground, one line, no border. */
.specimen {
  display: flex;
  align-items: center;
  box-sizing: border-box;
  height: 64px;
  margin: 8px 0 4px;
  padding: 0 14px;
  overflow: hidden;
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
  font-family: var(--fm-font-ui);
  font-size: 34px;
  line-height: 1;
  white-space: nowrap;
}
.specimen-text {
  position: relative;
  font-weight: 600;
}
.specimen.outline .specimen-text {
  -webkit-text-stroke: 1px currentColor;
  -webkit-text-fill-color: transparent;
}
/* Metric lines across the specimen, from the UI face's metrics at line-height 1: the
   baseline 0.865 em from the top, cap height 0.727 em and x-height 0.545 em above it. */
.guide {
  position: absolute;
  inset-inline: -100vw;
  height: 0;
  border-top: 1px dashed var(--fm-border-strong);
}
.guide.cap {
  top: 0.138em;
}
.guide.x {
  top: 0.32em;
}
.guide.base {
  top: 0.865em;
  border-top-style: solid;
}
.group {
  display: flex;
  flex-direction: column;
  padding: 10px 0 6px;
}
.group + .group {
  border-top: 1px solid var(--fm-border);
}
.heading {
  margin: 0 0 4px;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 36px;
}
.label {
  flex: none;
  width: 103px;
}
.wide {
  flex: 1;
  min-width: 0;
}
.slider-row input[type='range'] {
  flex: 1;
  min-width: 0;
  accent-color: var(--fm-accent);
}
.slider-row .number {
  flex: none;
  width: 80px;
}
.unit {
  flex: none;
  width: 20px;
  color: var(--fm-muted);
}
/* No effect here (E51.4, E54.8): the same place and size, at 40 %: label, slider and field. */
.slider-row.off .label,
.slider-row.off input[type='range'],
.slider-row.off .unit {
  opacity: 0.4;
}
.slider-row.off input[type='range'] {
  cursor: not-allowed;
}
.why {
  margin: 0 0 4px;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.toggle-row {
  justify-content: space-between;
  min-height: 44px;
}
.toggle-text {
  display: flex;
  flex-direction: column;
}
.desc {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
/* The align icons point to the start: mirrored with the reading direction. */
[dir='rtl'] .align :deep(svg) {
  transform: scaleX(-1);
}
.chip:focus-visible,
.more :focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
