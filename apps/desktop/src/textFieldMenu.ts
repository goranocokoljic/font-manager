/**
 * E54.2: the text fields' own menu — Cut, Copy, Paste, Select all — in place of the webview's,
 * which E54.1 blocks everywhere. These are the rules of the menu, apart from its look
 * (`components/TextFieldMenu.vue`): which element is a text field, which items it enables, and
 * what each item does. Each item does what its keyboard shortcut does: Cut and Copy run the
 * webview's own commands on the field's selection, Select all selects the field's text, Paste
 * inserts the clipboard's text as typing it would (one undo step, an `input` event). The
 * webview may not read the clipboard without asking, so the app reads it natively
 * (`readClipboardText`, ADR 0025). The shortcuts themselves are the webview's and untouched.
 */

/** The menu's items, in the menu's order. */
export const TEXT_COMMANDS = ['cut', 'copy', 'paste', 'selectAll'] as const;
export type TextCommand = (typeof TEXT_COMMANDS)[number];

/** A field the menu serves. */
export type TextField = HTMLInputElement | HTMLTextAreaElement;

/** The `<input>` types a person types text into; the others (checkbox, range, color…) get no menu. */
const TEXT_TYPES = new Set(['text', 'search', 'url', 'tel', 'email', 'password', 'number']);

/** The text field a context menu event is for, or `null`: not a field, or a disabled one. */
export function textFieldOf(target: EventTarget | null): TextField | null {
  if (target instanceof HTMLTextAreaElement) return target.disabled ? null : target;
  if (target instanceof HTMLInputElement && TEXT_TYPES.has(target.type) && !target.disabled)
    return target;
  return null;
}

/**
 * Whether the field has selected text. A field without a selection API (`number`, `email`)
 * answers through the document's selection, which holds the field's while it has the focus.
 */
export function hasSelection(field: TextField): boolean {
  let start: number | null = null;
  let end: number | null = null;
  try {
    start = field.selectionStart;
    end = field.selectionEnd;
  } catch {
    // Some input types throw instead of answering null.
  }
  if (start !== null && end !== null) return end > start;
  return (field.ownerDocument.getSelection()?.toString() ?? '') !== '';
}

/** Which items are enabled: Cut and Copy need a selection; Cut and Paste a field that is not read-only. */
export function enabledCommands(field: TextField): Record<TextCommand, boolean> {
  const selected = hasSelection(field);
  const writable = !field.readOnly;
  return { cut: selected && writable, copy: selected, paste: writable, selectAll: true };
}

/** What the items need from outside the field; the defaults are the webview's and the app's. */
export interface TextCommandHost {
  /** `document.execCommand`: the webview's own editing commands. */
  exec(command: 'copy' | 'cut' | 'delete' | 'insertText', value?: string): boolean;
  /** The text on the system clipboard (the app reads it natively). */
  readClipboard(): Promise<string>;
  /** Puts text on the clipboard: Copy's and Cut's fallback where the webview refuses its command. */
  writeClipboard(text: string): Promise<void>;
}

/** The field's selected text, for the fallback copy. */
function selectedText(field: TextField): string {
  const { selectionStart: start, selectionEnd: end } = field;
  if (start !== null && end !== null) return field.value.slice(start, end);
  return field.ownerDocument.getSelection()?.toString() ?? '';
}

/**
 * Runs one item on `field`, as its keyboard shortcut would. The focus and the selection are
 * the field's when this runs (the menu never takes them). Answers whether it did anything; a
 * disabled item does nothing.
 */
export async function runTextCommand(
  command: TextCommand,
  field: TextField,
  host: TextCommandHost,
): Promise<boolean> {
  if (!enabledCommands(field)[command]) return false;
  field.focus({ preventScroll: true });
  switch (command) {
    case 'selectAll':
      field.select();
      return true;
    case 'copy': {
      if (host.exec('copy')) return true;
      const text = selectedText(field);
      await host.writeClipboard(text);
      return true;
    }
    case 'cut': {
      if (host.exec('cut')) return true;
      const text = selectedText(field);
      await host.writeClipboard(text);
      host.exec('delete');
      return true;
    }
    case 'paste': {
      const text = await host.readClipboard();
      if (!text || !field.isConnected || field.readOnly) return false;
      field.focus({ preventScroll: true });
      if (host.exec('insertText', text)) return true;
      // A webview without `insertText`: the same text in place, and the `input` event typing fires.
      const start = field.selectionStart ?? field.value.length;
      const end = field.selectionEnd ?? start;
      field.setRangeText(text, start, end, 'end');
      field.dispatchEvent(
        new InputEvent('input', { bubbles: true, inputType: 'insertFromPaste', data: text }),
      );
      return true;
    }
  }
}

/** The chord of each item, as the webview binds it; `mod` is Ctrl, or Cmd on macOS. */
export const TEXT_COMMAND_KEYS: Record<TextCommand, string> = {
  cut: 'x',
  copy: 'c',
  paste: 'v',
  selectAll: 'a',
};

/**
 * Whether a context menu event came from the pointer: a right-click, or a Ctrl+click on macOS.
 * The Menu key and Shift+F10 send one with no button held (some webviews a plain `Event`).
 */
export function fromPointer(e: Event): boolean {
  if (!(e instanceof MouseEvent)) return false;
  return e.button === 2 || e.buttons !== 0 || e.ctrlKey;
}

/**
 * Where the menu opens: at the pointer for a right-click (or a Ctrl+click on macOS); under
 * the field's start edge for the Menu key or Shift+F10, which have no pointer.
 */
export function menuPoint(e: MouseEvent, field: Element): { x: number; y: number } {
  if (fromPointer(e)) return { x: e.clientX, y: e.clientY };
  const r = field.getBoundingClientRect();
  const rtl = getComputedStyle(field).direction === 'rtl';
  return { x: rtl ? r.right : r.left, y: r.bottom };
}
