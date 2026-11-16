// Where the size slider and the Language picker have an effect (E51.4). One table, read by
// the toolbar's slider and the preview row's picker: a control that does not apply is
// disabled (never hidden, so the toolbar keeps its shape, E50.2), and its tooltip says why
// and what to do. The stored size and language are not touched: they apply again as soon
// as the control does.
import type { MessageKey } from '../i18n';
import { wallLayout, type WallPreset } from '../list/grid';
import type { ViewMode } from '../stores/views';
import type { SampleKind } from './sampleText';

/**
 * What the rows show, as the table reads it: the six chips, plus your own text typed over a
 * chip other than Word (typing keeps the chip's layout, `PreviewControls.typeSample`).
 */
export type PreviewPreset =
  'pangram' | 'word' | 'custom' | 'alphabet' | 'numerals' | 'paragraph' | 'letters';

/** Every preset the table knows, for the tests that walk all of them. */
export const PREVIEW_PRESETS: readonly PreviewPreset[] = [
  'pangram',
  'word',
  'custom',
  'alphabet',
  'numerals',
  'paragraph',
  'letters',
];

/** A view mode, or the Layout split's pane (its text and size are the layout's own). */
export type PreviewSurface = ViewMode | 'layoutPane';

/** One control's state: whether it has an effect, and else the tooltip that says why. */
export type ControlState = { applies: true } | { applies: false; reason: MessageKey };

export interface Applicability {
  size: ControlState;
  language: ControlState;
}

const APPLIES: ControlState = { applies: true };
const off = (reason: MessageKey): ControlState => ({ applies: false, reason });

/** Why the size slider does nothing in a view mode, when it does nothing there at all. */
const SIZE_OFF: Partial<Record<PreviewSurface, MessageKey>> = {
  table: 'preview.inactive.sizeTable',
  waterfall: 'preview.inactive.sizeWaterfall',
  layoutPane: 'preview.inactive.sizeLayout',
};

/** The size slider and the Language picker on `surface` with `preset` on the rows. */
export function applicability(surface: PreviewSurface, preset: PreviewPreset): Applicability {
  const sizeOff = SIZE_OFF[surface];
  const size = sizeOff
    ? off(sizeOff)
    : preset === 'paragraph'
      ? off('preview.inactive.sizeParagraph')
      : APPLIES;
  const language =
    surface === 'layoutPane'
      ? off('preview.inactive.languageLayout')
      : preset === 'pangram'
        ? APPLIES
        : off('preview.inactive.language');
  return { size, language };
}

/** The settings the preset is read from (a slice of `PreviewSettings`). */
export interface PresetSource {
  preset: WallPreset;
  sample: SampleKind;
  customText: string;
}

/**
 * The preset the rows show now. The reading rows (the Paragraph chip, or your own text long
 * enough to read as one, `wallLayout`) are Paragraph: both are set at the body size. Else
 * the sample decides, since the popover's Sample can differ from the chip: your own text is
 * Word on the Word chip and `custom` on any other; Lorem and the font name show on the
 * Word layout and read as Word.
 */
export function previewPreset(s: PresetSource): PreviewPreset {
  if (wallLayout(s.preset, s.sample, s.customText) === 'paragraph') return 'paragraph';
  switch (s.sample) {
    case 'pangram':
    case 'alphabet':
    case 'numerals':
    case 'letters':
      return s.sample;
    // The body text on another chip's layout is set at the slider's size, like your own text.
    case 'paragraph':
      return 'custom';
    case 'custom':
      return s.preset === 'word' ? 'word' : 'custom';
    default:
      return 'word';
  }
}
