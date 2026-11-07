// Tofu (E43.4): a character the face lacks is drawn as a box, never in another font.
// Which characters a face lacks is the API's answer (`sampleTexts`, fm-core); this
// file only cuts the text into what the face draws and the boxes between it.
import type { SampleFallback, SampleText } from '../api/types';
import { t } from '../i18n';

/** A piece of a specimen: text the face draws, or one character it lacks. */
export interface SpecimenRun {
  text: string;
  missing: boolean;
}

/** The class of a tofu box (`styles/theme.css`). */
export const TOFU_CLASS = 'fm-tofu';

/** The missing characters listed after the count before an ellipsis. */
const LISTED = 8;

/**
 * `text` cut into runs: every character in `missing` is a run of its own, marked
 * missing; the characters between are drawn as they are. Without `missing` the
 * text is one run.
 */
export function specimenRuns(text: string, missing: readonly string[]): SpecimenRun[] {
  if (!missing.length) return text ? [{ text, missing: false }] : [];
  const lacks = new Set(missing);
  const runs: SpecimenRun[] = [];
  let drawn = '';
  for (const c of text) {
    if (!lacks.has(c)) {
      drawn += c;
      continue;
    }
    if (drawn) runs.push({ text: drawn, missing: false });
    drawn = '';
    runs.push({ text: c, missing: true });
  }
  if (drawn) runs.push({ text: drawn, missing: false });
  return runs;
}

/** Whether the face draws at least one letter or digit of `text`. */
export function drawsAny(text: string, missing: readonly string[]): boolean {
  const lacks = new Set(missing);
  for (const c of text) if (/[\p{L}\p{N}]/u.test(c) && !lacks.has(c)) return true;
  return false;
}

/** What a specimen shows for a sample and the API's answer for it. */
export interface Specimen {
  /** The text to draw. */
  text: string;
  /** Set when `text` is the face's own text (E4.5), not the sample. */
  fallback: SampleFallback | null;
  /** Characters of the sample the face lacks. */
  missing: string[];
  /** The characters `text` draws as tofu; empty for a fallback text. */
  tofu: string[];
}

/**
 * The specimen of a face for `sample`, given the API's `answer` (absent until it
 * arrives). A face that draws some of the sample shows the sample, its missing
 * characters as tofu. A face that draws none of its letters or digits keeps the
 * API's fallback text (E4.5), unless `keepSample`: a page that shows the text the
 * user chose (the family page, Quick Look, Studio) always shows it, as tofu if need be.
 */
export function specimenOf(
  sample: string,
  answer: SampleText | undefined,
  keepSample = false,
): Specimen {
  const missing = answer?.missing ?? [];
  if (!answer || !missing.length) {
    const fallback = answer?.fallback ?? null;
    return { text: answer?.text ?? sample, fallback, missing, tofu: [] };
  }
  if (!keepSample && answer.fallback && !drawsAny(sample, missing))
    return { text: answer.text, fallback: answer.fallback, missing, tofu: [] };
  return { text: sample, fallback: null, missing, tofu: missing };
}

/** "6 glyphs missing · š đ č ć ž": the count, then the first characters. */
export function missingLine(missing: readonly string[]): string {
  const chars = `${missing.slice(0, LISTED).join(' ')}${missing.length > LISTED ? ' …' : ''}`;
  return t('tofu.missing', { count: missing.length, chars });
}

/** The accessible name of one tofu box: "missing glyph ž". */
export function tofuLabel(char: string): string {
  return t('tofu.glyph', { char });
}
