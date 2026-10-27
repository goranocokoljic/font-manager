// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FontsNeededRequest, NeededFont } from '../src/api';
import { FmClient, NOT_RUNNING } from '../src/client';
import { MissingFontsPanel, type PanelDocument } from '../src/panel';
import { PANEL_STRINGS } from '../src/panel-strings';
import { fakeEndpoint } from '../src/testing';

const poster = { key: 'InDesign: Poster.indd #7', name: 'Poster.indd' };
const flyer = { key: 'InDesign: Flyer.indd #9', name: 'Flyer.indd' };
// Each font has its own name, face and state, so a row bound to the wrong
// font shows the wrong label.
const lora: NeededFont = { postscriptName: 'Lora-Bold', family: 'Lora', style: 'Bold' };
const barlow: NeededFont = { postscriptName: 'Barlow-Italic', family: 'Barlow', style: 'Italic' };
const inter: NeededFont = { postscriptName: 'Inter-Regular', family: 'Inter', style: 'Regular' };
const nope: NeededFont = { postscriptName: 'Nope-Black', family: 'Nope', style: 'Black' };
const FACES = new Map([
  ['Lora-Bold', 11],
  ['Barlow-Italic', 12],
  ['Inter-Regular', 13],
]);
// Inter clashes with an installed font.
const CLASHING = new Set([13]);

type Outcome = { status: string; reason?: string; error?: { code: string; message: string } };

function setup(
  missing: NeededFont[] = [lora, inter, nope],
  outcomes: Record<number, Outcome> = {},
) {
  const fake = fakeEndpoint({
    fonts_needed: (body) => {
      const req = body as FontsNeededRequest;
      const matched = req.fonts
        .filter((f) => FACES.has(f.postscriptName ?? ''))
        .map((font) => ({
          font,
          face: FACES.get(font.postscriptName ?? ''),
          matchedBy: 'postscriptName',
        }));
      const unmatched = req.fonts.filter((f) => !FACES.has(f.postscriptName ?? ''));
      return {
        matched,
        unmatched,
        missing: unmatched.length,
        outcomes:
          req.activate === false
            ? []
            : matched.map(({ face }) => ({
                face,
                outcome:
                  outcomes[face as number] ??
                  (CLASHING.has(face as number) && req.onConflict !== 'replace'
                    ? { status: 'conflict', with: { kind: 'system', path: '/fonts/Inter.otf' } }
                    : { status: 'done' }),
              })),
      };
    },
    conflicts: (body) =>
      (body as { faces: number[] }).faces
        .filter((f) => CLASHING.has(f))
        .map((face) => ({
          face,
          postscriptName: 'Inter-Regular',
          with: { kind: 'system', path: '/fonts/Inter.otf' },
        })),
  });
  let active: PanelDocument | null = { doc: poster, missing };
  const source = { activeDocument: () => active };
  const container = document.createElement('div');
  document.body.append(container);
  const panel = new MissingFontsPanel(container, new FmClient(fake), source);
  const set = (doc: PanelDocument | null) => {
    active = doc;
  };
  return { fake, panel, container, set };
}

const count = (c: HTMLElement) => c.querySelector('[role="status"]')?.textContent;
const rows = (c: HTMLElement) =>
  [...c.querySelectorAll('li')].map((li) => [
    li.querySelector('.fm-missing-name')?.textContent,
    li.querySelector('.fm-missing-state')?.textContent,
    li.querySelector('button')?.getAttribute('aria-label') ?? null,
  ]);
const button = (c: HTMLElement, name: string) => {
  const b = [...c.querySelectorAll('button')].find(
    (x) => (x.getAttribute('aria-label') ?? x.textContent) === name,
  );
  if (!b) throw new Error(`no button ${name}`);
  return b;
};
const activateAll = (c: HTMLElement) => button(c, PANEL_STRINGS.activateAll);
const question = (c: HTMLElement) => c.querySelector<HTMLElement>('[role="group"]');

beforeEach(() => {
  document.body.innerHTML = '';
});

describe('MissingFontsPanel', () => {
  it('shows the count and each missing font as the app sees it, without activating', async () => {
    const { fake, panel, container } = setup();
    await panel.refresh();
    expect(fake.calls('fonts_needed')).toEqual([
      { fonts: [lora, inter, nope], document: poster.key, activate: false },
    ]);
    expect(fake.calls('conflicts')).toEqual([{ kind: 'faces', faces: [11, 13] }]);
    expect(count(container)).toBe('3 fonts missing');
    expect(rows(container)).toEqual([
      ['Lora-Bold', 'In your library', 'Activate Lora-Bold'],
      ['Inter-Regular', 'Conflicts with an installed font', 'Activate Inter-Regular'],
      ['Nope-Black', 'Not in your library', null],
    ]);
    expect(activateAll(container).hidden).toBe(false);
  });

  it('names a font by full name, then family and style, when it has no PostScript name', async () => {
    const { panel, container } = setup([
      { family: 'Nope', style: 'Black' },
      { fullName: 'Nope Thin' },
    ]);
    await panel.refresh();
    expect(rows(container).map((r) => r[0])).toEqual(['Nope Black', 'Nope Thin']);
  });

  it('lists a font the host reports twice once', async () => {
    const { fake, panel, container } = setup([lora, lora, nope]);
    await panel.refresh();
    expect((fake.calls('fonts_needed')[0] as FontsNeededRequest).fonts).toEqual([lora, nope]);
    expect(count(container)).toBe('2 fonts missing');
  });

  it('activates every library font with one click on Activate all, then updates the count and rows', async () => {
    const { fake, panel, container } = setup([lora, barlow, nope]);
    await panel.refresh();
    activateAll(container).click();
    await vi.waitFor(() => expect(count(container)).toBe('1 font missing'));
    expect(fake.calls('fonts_needed')[1]).toEqual({
      fonts: [lora, barlow],
      document: poster.key,
      activate: true,
      onConflict: 'keepExisting',
    });
    expect(rows(container)).toEqual([['Nope-Black', 'Not in your library', null]]);
    expect(activateAll(container).hidden).toBe(true);
    expect(question(container)?.hidden).toBe(true);
  });

  it('activates one font with its own Activate button', async () => {
    const { fake, panel, container } = setup([lora, barlow, nope]);
    await panel.refresh();
    button(container, 'Activate Barlow-Italic').click();
    await vi.waitFor(() => expect(count(container)).toBe('2 fonts missing'));
    expect(fake.calls('fonts_needed')[1]).toEqual({
      fonts: [barlow],
      document: poster.key,
      activate: true,
      onConflict: 'keepExisting',
    });
    expect(rows(container).map((r) => r[0])).toEqual(['Lora-Bold', 'Nope-Black']);
  });

  it('asks keep or replace before Activate all touches a conflict; keep activates the others only', async () => {
    const { fake, panel, container } = setup();
    await panel.refresh();
    activateAll(container).click();
    await vi.waitFor(() => expect(question(container)?.hidden).toBe(false));
    expect(question(container)?.textContent).toContain(
      'Inter-Regular conflicts with an installed font',
    );
    // Nothing is activated while the question is open.
    expect(fake.calls('fonts_needed')).toHaveLength(1);
    expect(activateAll(container).disabled).toBe(true);
    button(container, PANEL_STRINGS.keep).click();
    await vi.waitFor(() => expect(count(container)).toBe('2 fonts missing'));
    expect(fake.calls('fonts_needed')[1]).toEqual({
      fonts: [lora],
      document: poster.key,
      activate: true,
      onConflict: 'keepExisting',
    });
    expect(rows(container).map((r) => r[1])).toEqual([
      'Conflicts with an installed font',
      'Not in your library',
    ]);
  });

  it('replaces the installed font when the user answers replace', async () => {
    const { fake, panel, container } = setup();
    await panel.refresh();
    activateAll(container).click();
    await vi.waitFor(() => expect(question(container)?.hidden).toBe(false));
    button(container, PANEL_STRINGS.replace).click();
    await vi.waitFor(() => expect(count(container)).toBe('1 font missing'));
    expect(fake.calls('fonts_needed')[1]).toEqual({
      fonts: [lora, inter],
      document: poster.key,
      activate: true,
      onConflict: 'replace',
    });
    expect(rows(container).map((r) => r[0])).toEqual(['Nope-Black']);
  });

  it('asks before a per-row Activate of a conflict, and keep sends nothing', async () => {
    const { fake, panel, container } = setup();
    await panel.refresh();
    button(container, 'Activate Inter-Regular').click();
    await vi.waitFor(() => expect(question(container)?.hidden).toBe(false));
    // Escape is keep.
    question(container)?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await vi.waitFor(() => expect(question(container)?.hidden).toBe(true));
    expect(fake.calls('fonts_needed')).toHaveLength(1);
    expect(count(container)).toBe('3 fonts missing');
  });

  it('keeps a font that was not activated and says why', async () => {
    const { panel, container } = setup([lora, nope], {
      11: { status: 'skipped', reason: 'The font file is missing.' },
    });
    await panel.refresh();
    button(container, 'Activate Lora-Bold').click();
    await vi.waitFor(() =>
      expect(container.querySelector('.fm-missing-message')?.textContent).toBe(
        'Lora-Bold was not activated: The font file is missing.',
      ),
    );
    expect(count(container)).toBe('2 fonts missing');
  });

  it('follows the active document and says when none is open', async () => {
    const { fake, panel, container, set } = setup([lora, nope]);
    await panel.refresh();
    set({ doc: flyer, missing: [barlow] });
    await panel.refresh();
    expect((fake.calls('fonts_needed')[1] as FontsNeededRequest).document).toBe(flyer.key);
    expect(rows(container).map((r) => r[0])).toEqual(['Barlow-Italic']);
    set(null);
    await panel.refresh();
    expect(count(container)).toBe('No document is open.');
    expect(rows(container)).toEqual([]);
    expect(activateAll(container).hidden).toBe(true);
    expect(fake.calls('fonts_needed')).toHaveLength(2);
  });

  it('says there is nothing missing without a call', async () => {
    const { fake, panel, container } = setup([]);
    await panel.refresh();
    expect(count(container)).toBe('No fonts missing');
    expect(fake.requests).toEqual([]);
  });

  it('drops an answer for a document that is no longer active', async () => {
    const { panel, container, set } = setup([lora]);
    const first = panel.refresh();
    set({ doc: flyer, missing: [barlow] });
    const second = panel.refresh();
    await Promise.all([first, second]);
    expect(rows(container).map((r) => r[0])).toEqual(['Barlow-Italic']);
  });

  it('says when Font Manager is not running', async () => {
    const { fake, panel, container } = setup();
    fake.listening = false;
    await panel.refresh();
    expect(count(container)).toBe(NOT_RUNNING);
    expect(rows(container)).toEqual([]);
  });

  it('has labelled, keyboard-reachable controls and announces the count in a live region', async () => {
    const { panel, container } = setup();
    await panel.refresh();
    const section = container.querySelector('section');
    const title = document.getElementById(section?.getAttribute('aria-labelledby') ?? '');
    expect(title?.textContent).toBe('Missing fonts');
    const live = container.querySelector('[role="status"]');
    expect(live?.getAttribute('aria-live')).toBe('polite');
    expect(live?.getAttribute('aria-atomic')).toBe('true');
    expect(container.querySelector('ul')?.getAttribute('aria-label')).toBe(
      'Missing fonts of the active document',
    );
    const visible = [...container.querySelectorAll('button')].filter(
      (b) => !b.hidden && !b.closest('[hidden]'),
    );
    expect(visible.length).toBe(3);
    for (const b of visible) {
      // A native button with visible text that its accessible name contains.
      expect(b.type).toBe('button');
      expect(b.tabIndex).toBe(0);
      expect(b.textContent?.trim()).not.toBe('');
      expect(b.getAttribute('aria-label') ?? b.textContent).toContain(b.textContent ?? '');
    }
    // The question is a labelled group whose first answer takes the focus.
    button(container, 'Activate Inter-Regular').click();
    await vi.waitFor(() => expect(question(container)?.hidden).toBe(false));
    const group = question(container);
    expect(
      document.getElementById(group?.getAttribute('aria-labelledby') ?? '')?.textContent,
    ).toContain('conflicts with an installed font');
    expect(document.activeElement).toBe(button(container, PANEL_STRINGS.keep));
    // The live region is the same element after a change, so it is announced.
    button(container, PANEL_STRINGS.replace).click();
    await vi.waitFor(() => expect(count(container)).toBe('2 fonts missing'));
    expect(container.querySelector('[role="status"]')).toBe(live);
    // Focus left on a removed row stays in the panel.
    expect(container.contains(document.activeElement)).toBe(true);
  });
});
