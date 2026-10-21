import { describe, expect, it } from 'vitest';
import {
  ACTIONS,
  KEYMAP,
  ariaShortcut,
  bindingFor,
  chordOf,
  formatChord,
  matches,
  yieldsToFocus,
  type Chord,
} from './keymap';

const key = (init: KeyboardEventInit) => new KeyboardEvent('keydown', init);

function on(target: Element, init: KeyboardEventInit): KeyboardEvent {
  const e = key({ ...init, bubbles: true });
  let seen: KeyboardEvent = e;
  target.addEventListener('keydown', (ev) => (seen = ev as KeyboardEvent), { once: true });
  document.body.append(target);
  target.dispatchEvent(e);
  target.remove();
  return seen;
}

describe('keymap', () => {
  // Spec, UX design → Keyboard model.
  const spec: [string, Chord][] = [
    [ACTIONS.search, { key: '/' }],
    [ACTIONS.palette, { key: 'k', mod: true }],
    [ACTIONS.quickLook, { key: 'space' }],
    [ACTIONS.activateSession, { key: 'a' }],
    [ACTIONS.activatePermanent, { key: 'a', shift: true }],
    [ACTIONS.compare, { key: 'c' }],
    [ACTIONS.quickCollection, { key: 'b' }],
    [ACTIONS.tag, { key: 't' }],
    [ACTIONS.viewMode(1), { key: '1' }],
    [ACTIONS.viewMode(2), { key: '2' }],
    [ACTIONS.viewMode(3), { key: '3' }],
    [ACTIONS.viewMode(4), { key: '4' }],
    [ACTIONS.viewMode(5), { key: '5' }],
    [ACTIONS.settings, { key: ',', mod: true }],
  ];

  it.each(spec)('binds %s as the spec says', (action, chord) => {
    expect(chordOf(action)).toEqual(chord);
  });

  it('gives every chord one action', () => {
    const chords = KEYMAP.map((b) => JSON.stringify(b.chord));
    expect(new Set(chords).size).toBe(chords.length);
  });

  it('keeps Ctrl/Cmd+I for the inspector', () => {
    expect(bindingFor(key({ key: 'i', ctrlKey: true }))?.action).toBe(ACTIONS.inspector);
    expect(bindingFor(key({ key: 'I', metaKey: true }))?.action).toBe(ACTIONS.inspector);
  });

  it('accepts Ctrl or Cmd as the modifier', () => {
    const k = chordOf(ACTIONS.palette)!;
    expect(matches(key({ key: 'k', ctrlKey: true }), k)).toBe(true);
    expect(matches(key({ key: 'k', metaKey: true }), k)).toBe(true);
    expect(matches(key({ key: 'k' }), k)).toBe(false);
    expect(matches(key({ key: 'k', ctrlKey: true, altKey: true }), k)).toBe(false);
  });

  it('tells A from Shift+A and a plain key from a modified one', () => {
    expect(bindingFor(key({ key: 'a' }))?.action).toBe(ACTIONS.activateSession);
    expect(bindingFor(key({ key: 'A', shiftKey: true }))?.action).toBe(ACTIONS.activatePermanent);
    expect(bindingFor(key({ key: 'a', ctrlKey: true }))).toBeUndefined();
    expect(bindingFor(key({ key: 'I', ctrlKey: true, shiftKey: true }))).toBeUndefined();
  });

  it('matches the physical key on a non-Latin layout', () => {
    // Russian layout: the K key types "л", the A key "ф", the comma key "б".
    expect(bindingFor(key({ key: 'л', code: 'KeyK', ctrlKey: true }))?.action).toBe(
      ACTIONS.palette,
    );
    expect(bindingFor(key({ key: 'ф', code: 'KeyA' }))?.action).toBe(ACTIONS.activateSession);
  });

  it('follows the typed character on another Latin layout', () => {
    // Dvorak: the physical K key types "t"; Ctrl+T is not the palette.
    expect(bindingFor(key({ key: 't', code: 'KeyK', ctrlKey: true }))).toBeUndefined();
    expect(bindingFor(key({ key: 'k', code: 'KeyV', ctrlKey: true }))?.action).toBe(
      ACTIONS.palette,
    );
  });

  it('accepts / and , however the layout reaches them', () => {
    // German layout: "/" is Shift+7.
    expect(bindingFor(key({ key: '/', code: 'Digit7', shiftKey: true }))?.action).toBe(
      ACTIONS.search,
    );
    expect(bindingFor(key({ key: ',', ctrlKey: true }))?.action).toBe(ACTIONS.settings);
  });

  it('maps the digits 1–5 to the view modes and nothing else', () => {
    for (const n of [1, 2, 3, 4, 5] as const)
      expect(bindingFor(key({ key: String(n), code: `Digit${n}` }))?.action).toBe(
        ACTIONS.viewMode(n),
      );
    expect(bindingFor(key({ key: '6' }))).toBeUndefined();
  });
});

describe('focus guard', () => {
  it('lets a text field keep plain keys but not modified ones', () => {
    for (const el of [
      document.createElement('input'),
      document.createElement('textarea'),
      document.createElement('select'),
    ]) {
      expect(bindingFor(on(el, { key: 'a' }))).toBeUndefined();
      expect(bindingFor(on(el, { key: '/' }))).toBeUndefined();
      expect(bindingFor(on(el, { key: 'k', ctrlKey: true }))?.action).toBe(ACTIONS.palette);
    }
  });

  it('treats a contenteditable element as a text field', () => {
    const div = document.createElement('div');
    div.contentEditable = 'true';
    // jsdom does not compute isContentEditable; the browser does.
    Object.defineProperty(div, 'isContentEditable', { value: true });
    expect(bindingFor(on(div, { key: 'c' }))).toBeUndefined();
  });

  it('lets a button keep Space but not letters', () => {
    const button = document.createElement('button');
    expect(bindingFor(on(button, { key: ' ' }))).toBeUndefined();
    expect(bindingFor(on(button, { key: 'a' }))?.action).toBe(ACTIONS.activateSession);
    const tab = document.createElement('div');
    tab.setAttribute('role', 'tab');
    expect(yieldsToFocus(on(tab, { key: ' ' }), { key: 'space' })).toBe(true);
  });

  it('runs Space as quick-look outside controls', () => {
    const region = document.createElement('main');
    expect(bindingFor(on(region, { key: ' ' }))?.action).toBe(ACTIONS.quickLook);
  });

  it('runs A on a checkbox (only text fields keep letters)', () => {
    const box = document.createElement('input');
    box.type = 'checkbox';
    expect(bindingFor(on(box, { key: 'a' }))?.action).toBe(ACTIONS.activateSession);
    expect(bindingFor(on(box, { key: ' ' }))).toBeUndefined();
  });
});

describe('chord labels', () => {
  it('shows Ctrl on Windows and Linux, symbols on macOS', () => {
    expect(formatChord({ key: 'k', mod: true }, false)).toBe('Ctrl+K');
    expect(formatChord({ key: 'k', mod: true }, true)).toBe('⌘K');
    expect(formatChord({ key: 'a', shift: true }, false)).toBe('Shift+A');
    expect(formatChord({ key: 'a', shift: true }, true)).toBe('⇧A');
    expect(formatChord({ key: 'space' }, false)).toBe('Space');
    expect(formatChord({ key: ',', mod: true }, false)).toBe('Ctrl+,');
  });

  it('lists both modifiers for assistive technology', () => {
    expect(ariaShortcut({ key: 'k', mod: true })).toBe('Control+K Meta+K');
    expect(ariaShortcut({ key: 'a', shift: true })).toBe('Shift+A');
    expect(ariaShortcut({ key: 'space' })).toBe('Space');
    expect(ariaShortcut({ key: '/' })).toBe('/');
  });
});
