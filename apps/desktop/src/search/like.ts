// The words of the likeness sort (E49.9): 'Fonts with an R like this'.
import { t } from '../i18n';

/** Latin letters whose English name starts with a vowel sound: "an R", "an f", but "a g". */
const AN_LETTERS = /^[aefhilmnorsx]$/i;

/** The action that sorts the wall by `glyph` of a face: 'Fonts with an R like this'. */
export function likeThisLabel(glyph: string): string {
  return t(AN_LETTERS.test(glyph) ? 'grid.likeThisAn' : 'grid.likeThis', { glyph });
}
