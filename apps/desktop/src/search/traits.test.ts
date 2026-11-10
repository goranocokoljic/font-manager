import { describe, expect, it } from 'vitest';
import type { GlyphTraitInfo, TraitFilter } from '../api/types';
import {
  chooseTrait,
  chosenValue,
  parseTraitWords,
  traitChips,
  traitChipTail,
  traitForGlyph,
  traitLineLabel,
  traitsText,
  withoutTrait,
} from './traits';

const trait = (
  id: string,
  glyph: string,
  name: string,
  values: [string, string][],
  enabled = true,
): GlyphTraitInfo => ({
  id,
  script: 'Latn',
  glyph,
  kind: 'enum',
  name,
  description: '',
  enabled,
  measures: [],
  values: values.map(([v, label]) => ({
    id: v,
    label,
    description: '',
    when: [],
    example: 'M0 0',
    faces: 0,
    families: 0,
  })),
  measured: 0,
  unsure: 0,
});

const catalog: GlyphTraitInfo[] = [
  trait('R.leg', 'R', 'R leg', [
    ['straight', 'Straight'],
    ['curved', 'Curved'],
  ]),
  trait('O.roundness', 'O', 'O shape', [
    ['round', 'Round'],
    ['oval', 'Oval'],
  ]),
  trait('Q.tail', 'Q', 'Q tail', [['inside', 'Inside']], false),
];

const value = (traitId: string, v: string): TraitFilter => ({ kind: 'value', traitId, value: v });

describe('glyph-trait filters (E49.6)', () => {
  it('offers a picker only for a glyph with an enabled trait', () => {
    expect(traitForGlyph(catalog, 'R')?.id).toBe('R.leg');
    // Q's detector misses the accuracy bar: listed disabled, so no picker.
    expect(traitForGlyph(catalog, 'Q')).toBeUndefined();
    expect(traitForGlyph(catalog, 'a')).toBeUndefined();
  });

  it('keeps one value per trait: a new choice replaces the old, Any takes it off', () => {
    let traits = chooseTrait([], 'R.leg', 'straight');
    traits = chooseTrait(traits, 'O.roundness', 'round');
    traits = chooseTrait(traits, 'R.leg', 'curved');
    expect(traits).toEqual([value('O.roundness', 'round'), value('R.leg', 'curved')]);
    expect(chosenValue(traits, 'R.leg')).toBe('curved');
    traits = chooseTrait(traits, 'R.leg', null);
    expect(traits).toEqual([value('O.roundness', 'round')]);
    expect(chosenValue(traits, 'R.leg')).toBeNull();
  });

  it('counts a trait against the others: withoutTrait drops only that trait', () => {
    const traits = [value('R.leg', 'straight'), value('O.roundness', 'round')];
    expect(withoutTrait(traits, 'R.leg')).toEqual([value('O.roundness', 'round')]);
    expect(withoutTrait(traits, 'g.storeys')).toEqual(traits);
  });

  it('names a chip "R · leg · straight" and the count line "R leg straight"', () => {
    const [r, o] = traitChips([value('R.leg', 'straight'), value('O.roundness', 'round')], catalog);
    expect(r).toMatchObject({
      glyph: 'R',
      aspect: 'leg',
      valueLabel: 'straight',
      text: 'R:straight',
    });
    expect(`${r!.glyph} ${traitChipTail(r!)}`).toBe('R · leg · straight');
    expect(traitLineLabel(r!)).toBe('R leg straight');
    // "O shape" names nothing beyond the glyph: the board says "O round" (criterion 4).
    expect(traitLineLabel(o!)).toBe('O round');
    expect(`${o!.glyph} ${traitChipTail(o!)}`).toBe('O · round');
  });

  it('shows the ids of a filter whose trait the catalog does not hold yet', () => {
    const [c] = traitChips([value('g.storeys', 'single')], []);
    expect(c).toMatchObject({ glyph: 'g', aspect: 'storeys', valueLabel: 'single' });
  });

  it('parses R:straight O:round to the same filters the picker makes, and back', () => {
    const parsed = parseTraitWords('R:straight O:round', catalog);
    const picked = chooseTrait(chooseTrait([], 'R.leg', 'straight'), 'O.roundness', 'round');
    expect(parsed).toEqual({ text: '', traits: picked });
    expect(traitsText(parsed.traits, catalog)).toBe('R:straight O:round');
    expect(parseTraitWords(traitsText(picked, catalog), catalog).traits).toEqual(picked);
  });

  it('takes a label in any case and keeps the rest of the text', () => {
    expect(parseTraitWords('grotesk r:Straight', catalog)).toEqual({
      text: 'grotesk r:Straight',
      traits: [],
    });
    expect(parseTraitWords('grotesk R:CURVED bold', catalog)).toEqual({
      text: 'grotesk bold',
      traits: [value('R.leg', 'curved')],
    });
  });

  it('leaves words that name no trait or value as search text', () => {
    expect(parseTraitWords('1:1 R:wiggly ratio:16', catalog)).toEqual({
      text: '1:1 R:wiggly ratio:16',
      traits: [],
    });
  });

  it('a later word for the same trait wins', () => {
    expect(parseTraitWords('R:straight R:curved', catalog).traits).toEqual([
      value('R.leg', 'curved'),
    ]);
  });
});
