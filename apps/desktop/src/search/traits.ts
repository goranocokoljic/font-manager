// Glyph-trait filters (E49.6): the letters of the Letters preset are the controls. A letter
// whose glyph has a trait the picker offers opens a picker of that trait's values; the
// choice is a `TraitFilter` of the search (`SearchRequest.glyphTraits`, E49.5) and shows as
// an ordinary chip. The catalog and every count come from the API (`listGlyphTraits`,
// `search`); this module only maps between the catalog, the request and the words shown.
import type { GlyphTraitInfo, GlyphTraitValue, TraitFilter } from '../api/types';

/** A chosen value of one trait, with the catalog words a chip and the count line show. */
export interface TraitChip {
  traitId: string;
  value: string;
  /** The character the trait is measured on (`R`). */
  glyph: string;
  /** What of the glyph the trait is about (`leg` of "R leg"); empty when the name is only the glyph. */
  aspect: string;
  /** The value's label, lower case (`straight`). */
  valueLabel: string;
  /** The search-box form (`R:straight`). */
  text: string;
}

/** The trait the picker offers for a typed glyph, if any: one trait per glyph in the catalog. */
export function traitForGlyph(
  catalog: readonly GlyphTraitInfo[],
  glyph: string,
): GlyphTraitInfo | undefined {
  return catalog.find((t) => t.enabled && t.glyph === glyph);
}

/** The value id chosen for `traitId`, if any. */
export function chosenValue(traits: readonly TraitFilter[], traitId: string): string | null {
  const f = traits.find((t) => t.kind === 'value' && t.traitId === traitId);
  return f?.kind === 'value' ? f.value : null;
}

/**
 * The trait filters with `value` chosen for `traitId` in place of what was chosen for it
 * (one value per letter); `null` is "Any" and takes the trait off.
 */
export function chooseTrait(
  traits: readonly TraitFilter[],
  traitId: string,
  value: string | null,
): TraitFilter[] {
  const rest = traits.filter((t) => t.traitId !== traitId);
  return value === null ? rest : [...rest, { kind: 'value', traitId, value }];
}

/** The trait filters without `traitId`: what a value of it is counted against. */
export function withoutTrait(traits: readonly TraitFilter[], traitId: string): TraitFilter[] {
  return traits.filter((t) => t.traitId !== traitId);
}

/**
 * Trait names that say no more than "the glyph": "O shape", "I form". The chip and the count
 * line leave them out, as board 8-letters-filtered.png does ("O round", not "O shape round").
 */
const GENERIC_ASPECTS = new Set(['shape', 'form']);

/** "R leg" without its glyph: "leg"; empty for a generic name ("O shape"). */
function aspectOf(t: GlyphTraitInfo): string {
  const name = t.name.trim();
  const aspect = name.startsWith(t.glyph) ? name.slice(t.glyph.length).trim() : name;
  return GENERIC_ASPECTS.has(aspect.toLowerCase()) ? '' : aspect;
}

/** The value of a trait by id. */
function valueOf(t: GlyphTraitInfo, id: string): GlyphTraitValue | undefined {
  return t.values.find((v) => v.id === id);
}

/**
 * The chips of the chosen value filters, in the order they were chosen. A filter the
 * catalog does not know (not loaded yet) shows its ids, so it can still be taken off.
 */
export function traitChips(
  traits: readonly TraitFilter[],
  catalog: readonly GlyphTraitInfo[],
): TraitChip[] {
  const chips: TraitChip[] = [];
  for (const f of traits) {
    if (f.kind !== 'value') continue;
    const t = catalog.find((c) => c.id === f.traitId);
    const glyph = t?.glyph ?? f.traitId.split('.')[0] ?? f.traitId;
    chips.push({
      traitId: f.traitId,
      value: f.value,
      glyph,
      aspect: t ? aspectOf(t) : (f.traitId.split('.')[1] ?? ''),
      valueLabel: (t && valueOf(t, f.value)?.label.toLowerCase()) || f.value,
      text: `${glyph}:${f.value}`,
    });
  }
  return chips;
}

/** A chip's words for the count line: "R leg straight". */
export const traitLineLabel = (c: TraitChip): string =>
  [c.glyph, c.aspect, c.valueLabel].filter(Boolean).join(' ');

/** What a chip shows after its semibold glyph: "· leg · straight". */
export const traitChipTail = (c: TraitChip): string =>
  ['', c.aspect, c.valueLabel]
    .filter((w, i) => i === 0 || w !== '')
    .join(' · ')
    .trimStart();

/** The search-box form of the value filters: `R:straight O:round`. */
export function traitsText(
  traits: readonly TraitFilter[],
  catalog: readonly GlyphTraitInfo[],
): string {
  return traitChips(traits, catalog)
    .map((c) => c.text)
    .join(' ');
}

/** `<glyph>:<value>` as a word of its own; the value may be its id or its label. */
const TOKEN = /(^|\s)(\S):([^\s:]+)(?=\s|$)/gu;

/**
 * Takes the `<glyph>:<value>` words out of a search text (`R:straight O:round`): each that
 * names a trait of the catalog and one of its values (id or label, any case) becomes a
 * trait filter; a later word for the same trait wins. Words that name no trait or value
 * stay in the text, so a search for "1:1" still searches.
 */
export function parseTraitWords(
  text: string,
  catalog: readonly GlyphTraitInfo[],
): { text: string; traits: TraitFilter[] } {
  let traits: TraitFilter[] = [];
  const rest = text.replace(TOKEN, (word, lead: string, glyph: string, value: string) => {
    const t = catalog.find((c) => c.glyph === glyph);
    const wanted = value.toLowerCase();
    const v = t?.values.find(
      (x) => x.id.toLowerCase() === wanted || x.label.toLowerCase() === wanted,
    );
    if (!t || !v) return word;
    traits = chooseTrait(traits, t.id, v.id);
    return lead;
  });
  return { text: traits.length ? rest.replace(/\s+/g, ' ').trim() : text, traits };
}
