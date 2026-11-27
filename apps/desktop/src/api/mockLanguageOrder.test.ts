import { describe, expect, it } from 'vitest';
import cases from '../../../../fixtures/language-order/cases.json';
import { createMockClient, shellLocale } from './mock';
import { arrangeLanguages, mostUsedLanguages } from './mockLanguageOrder';
import type { Event, LanguagePreset } from './types';

const presets: LanguagePreset[] = cases.presets.map((p) => ({ ...p }));

describe('the language order of the mock (E58.1)', () => {
  it('gives the core answer for every shared case', () => {
    expect(cases.cases.length).toBeGreaterThanOrEqual(7);
    for (const c of cases.cases) {
      const got = arrangeLanguages(presets, cases.mostUsed, c.myLanguages, c.system);
      expect(
        got.map((p) => [p.code, p.group]),
        c.name,
      ).toEqual(c.expected);
    }
  });

  it('reads the same order file as the core', () => {
    expect(mostUsedLanguages.slice(0, 4)).toEqual(['en', 'es', 'fr', 'de']);
  });

  it('orders listLanguages by my languages and the shell language, and announces a change', async () => {
    const client = createMockClient();
    const events: Event[] = [];
    await client.onEvent((e) => events.push(e));
    await client.setShellLocale('de');
    expect(events.filter((e) => e.type === 'languageOrderChanged')).toHaveLength(1);
    let list = await client.listLanguages();
    expect(list.slice(0, 3).map((p) => [p.code, p.group])).toEqual([
      ['de', 'mine'],
      ['en', 'mostUsed'],
      ['fr', 'mostUsed'],
    ]);
    await client.updateSettings({ myLanguages: ['sr-Latn', 'pl', 'fr'] });
    expect(events.filter((e) => e.type === 'languageOrderChanged')).toHaveLength(2);
    list = await client.listLanguages();
    expect(list.map((p) => p.code).slice(0, 4)).toEqual(['sr-Latn', 'pl', 'fr', 'en']);
    // Each preset once.
    expect(new Set(list.map((p) => p.code)).size).toBe(list.length);
    // The same list again changes nothing.
    await client.updateSettings({ myLanguages: ['sr-Latn', 'pl', 'fr'] });
    expect(events.filter((e) => e.type === 'languageOrderChanged')).toHaveLength(2);
    shellLocale.value = '';
  });
});
