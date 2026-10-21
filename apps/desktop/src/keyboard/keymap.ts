// The keyboard model from the spec: which key runs which action. Actions are
// registered by the component that owns the feature (see stores/commands.ts); a
// binding whose action is not registered yet does nothing and lets the key through.

/** A key with its modifiers. `mod` is Ctrl or Cmd: either is accepted on every OS. */
export interface Chord {
  /** Lower-case character (`a`, `1`, `/`, `,`) or `space`. */
  key: string;
  mod?: boolean;
  shift?: boolean;
}

export interface Binding {
  action: string;
  chord: Chord;
}

/** Action ids with a binding. Stories that build the feature register these ids. */
export const ACTIONS = {
  palette: 'palette.toggle',
  search: 'search.focus',
  quickLook: 'face.quickLook',
  activateSession: 'activation.toggleSession',
  activatePermanent: 'activation.togglePermanent',
  compare: 'compare.add',
  quickCollection: 'quickCollection.toggle',
  viewMode: (n: 1 | 2 | 3 | 4 | 5) => `view.mode${n}`,
  settings: 'settings.open',
  inspector: 'view.toggleInspector',
} as const;

/**
 * Spec, UX design → Keyboard model: `/` search · Ctrl/Cmd+K command palette · Space
 * quick-look · A toggle session activation · Shift+A permanent · C add to compare ·
 * 1–5 view modes · Ctrl/Cmd+, settings. Ctrl/Cmd+I (inspector) comes from E5.1.
 * B adds the selected fonts to the Quick Collection or removes them (E13.3).
 * Arrows and Enter inside the font list belong to the list itself (E5.2).
 */
export const KEYMAP: readonly Binding[] = [
  { action: ACTIONS.search, chord: { key: '/' } },
  { action: ACTIONS.palette, chord: { key: 'k', mod: true } },
  { action: ACTIONS.quickLook, chord: { key: 'space' } },
  { action: ACTIONS.activateSession, chord: { key: 'a' } },
  { action: ACTIONS.activatePermanent, chord: { key: 'a', shift: true } },
  { action: ACTIONS.compare, chord: { key: 'c' } },
  { action: ACTIONS.quickCollection, chord: { key: 'b' } },
  ...([1, 2, 3, 4, 5] as const).map((n) => ({
    action: ACTIONS.viewMode(n),
    chord: { key: String(n) },
  })),
  { action: ACTIONS.settings, chord: { key: ',', mod: true } },
  { action: ACTIONS.inspector, chord: { key: 'i', mod: true } },
];

/** The binding of an action, if it has one. */
export function chordOf(action: string): Chord | undefined {
  return KEYMAP.find((b) => b.action === action)?.chord;
}

// Letters and digits also match on the physical key, so shortcuts keep working on
// non-Latin layouts (Cyrillic, Greek) where `key` is not an ASCII character.
function physicalKey(code: string): string | undefined {
  const m = /^(?:Key([A-Z])|Digit([0-9]))$/.exec(code);
  return m ? (m[1] ?? m[2]).toLowerCase() : undefined;
}

function keyName(e: KeyboardEvent): string {
  return e.key === ' ' ? 'space' : e.key.toLowerCase();
}

const isAlnum = (key: string) => /^[a-z0-9]$/.test(key);

/** True when the key event is this chord. */
export function matches(e: KeyboardEvent, chord: Chord): boolean {
  if (e.altKey) return false;
  if ((e.ctrlKey || e.metaKey) !== Boolean(chord.mod)) return false;
  const name = keyName(e);
  const ascii = /^[\x20-\x7e]$/.test(e.key);
  const keyHit = name === chord.key || (!ascii && physicalKey(e.code) === chord.key);
  if (!keyHit) return false;
  // Punctuation needs Shift on some layouts (`/` is Shift+7 on German), so Shift is only
  // significant for letters, digits and Space.
  if (isAlnum(chord.key) || chord.key === 'space') return e.shiftKey === Boolean(chord.shift);
  return true;
}

const TEXT_INPUT_TYPES = new Set([
  'text',
  'search',
  'email',
  'url',
  'tel',
  'password',
  'number',
  'date',
  'datetime-local',
  'month',
  'time',
  'week',
]);

function isTextEntry(el: Element): boolean {
  if (el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) return true;
  if (el instanceof HTMLInputElement) return TEXT_INPUT_TYPES.has(el.type);
  return el instanceof HTMLElement && el.isContentEditable;
}

const CONTROL_ROLES = /^(button|checkbox|switch|tab|option|menuitem|radio|link|combobox|slider)$/;

function isControl(el: Element): boolean {
  return (
    el instanceof HTMLButtonElement ||
    el instanceof HTMLAnchorElement ||
    el instanceof HTMLInputElement ||
    el.tagName === 'SUMMARY' ||
    CONTROL_ROLES.test(el.getAttribute('role') ?? '')
  );
}

/**
 * True when a chord must not fire for this event because the focused element needs the
 * key itself: a plain key while typing in a field, or Space on a button or other control.
 */
export function yieldsToFocus(e: KeyboardEvent, chord: Chord): boolean {
  const el = e.target instanceof Element ? e.target : null;
  if (!el || chord.mod) return false;
  if (isTextEntry(el)) return true;
  return chord.key === 'space' && isControl(el);
}

/** The binding the event triggers, if any. */
export function bindingFor(e: KeyboardEvent): Binding | undefined {
  return KEYMAP.find((b) => matches(e, b.chord) && !yieldsToFocus(e, b.chord));
}

/** True on macOS, where the modifier shows as ⌘. */
export function isMac(): boolean {
  return typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.userAgent);
}

const keyLabel = (key: string) => (key === 'space' ? 'Space' : key.toUpperCase());

/** How a chord is shown to the user: `Ctrl+K` / `⌘K`, `Shift+A` / `⇧A`. */
export function formatChord(chord: Chord, mac = isMac()): string {
  const parts: string[] = [];
  if (chord.mod) parts.push(mac ? '⌘' : 'Ctrl');
  if (chord.shift) parts.push(mac ? '⇧' : 'Shift');
  parts.push(keyLabel(chord.key));
  return parts.join(mac ? '' : '+');
}

/** The `aria-keyshortcuts` value of a chord; Ctrl and Cmd are both listed. */
export function ariaShortcut(chord: Chord): string {
  const key = chord.key === 'space' ? 'Space' : keyLabel(chord.key);
  const plain = [chord.shift ? 'Shift' : '', key].filter(Boolean).join('+');
  return chord.mod ? `Control+${plain} Meta+${plain}` : plain;
}
