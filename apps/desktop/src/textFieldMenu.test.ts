import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  enabledCommands,
  menuPoint,
  runTextCommand,
  textFieldOf,
  type TextCommandHost,
} from './textFieldMenu';

function input(attrs: Record<string, string> = {}, value = ''): HTMLInputElement {
  const el = document.createElement('input');
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  el.value = value;
  document.body.append(el);
  return el;
}

function host(over: Partial<TextCommandHost> = {}): TextCommandHost & {
  exec: ReturnType<typeof vi.fn>;
} {
  return {
    exec: vi.fn(() => true),
    readClipboard: vi.fn(async () => 'pasted'),
    writeClipboard: vi.fn(async () => {}),
    ...over,
  } as TextCommandHost & { exec: ReturnType<typeof vi.fn> };
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('textFieldOf', () => {
  it('serves text inputs and text areas', () => {
    for (const type of ['text', 'search', 'url', 'number', 'email']) {
      const el = input({ type });
      expect(textFieldOf(el)).toBe(el);
    }
    const area = document.createElement('textarea');
    expect(textFieldOf(area)).toBe(area);
  });

  it('ignores other controls, disabled fields and plain elements', () => {
    expect(textFieldOf(input({ type: 'checkbox' }))).toBeNull();
    expect(textFieldOf(input({ type: 'range' }))).toBeNull();
    expect(textFieldOf(input({ type: 'text', disabled: '' }))).toBeNull();
    const area = document.createElement('textarea');
    area.disabled = true;
    expect(textFieldOf(area)).toBeNull();
    expect(textFieldOf(document.createElement('div'))).toBeNull();
    expect(textFieldOf(null)).toBeNull();
  });
});

describe('enabledCommands', () => {
  it('disables Cut and Copy without a selection', () => {
    const el = input({ type: 'text' }, 'Grumpy wizards');
    el.setSelectionRange(3, 3);
    expect(enabledCommands(el)).toEqual({ cut: false, copy: false, paste: true, selectAll: true });
  });

  it('enables every item with a selection in a writable field', () => {
    const el = input({ type: 'search' }, 'Grumpy wizards');
    el.setSelectionRange(0, 6);
    expect(enabledCommands(el)).toEqual({ cut: true, copy: true, paste: true, selectAll: true });
  });

  it('disables Cut and Paste in a read-only field', () => {
    const el = input({ type: 'text', readonly: '' }, 'Grumpy wizards');
    el.setSelectionRange(0, 6);
    expect(enabledCommands(el)).toEqual({ cut: false, copy: true, paste: false, selectAll: true });
  });
});

describe('runTextCommand', () => {
  it('Copy and Cut run the webview command on the selection', async () => {
    const el = input({ type: 'text' }, 'Grumpy wizards');
    el.setSelectionRange(0, 6);
    const h = host();
    expect(await runTextCommand('copy', el, h)).toBe(true);
    expect(h.exec).toHaveBeenCalledWith('copy');
    expect(await runTextCommand('cut', el, h)).toBe(true);
    expect(h.exec).toHaveBeenCalledWith('cut');
    expect(h.writeClipboard).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(el);
  });

  it('Copy and Cut fall back to the clipboard when the webview refuses', async () => {
    const el = input({ type: 'text' }, 'Grumpy wizards');
    el.setSelectionRange(7, 14);
    const h = host({ exec: vi.fn((c: string) => c === 'delete') });
    await runTextCommand('copy', el, h);
    expect(h.writeClipboard).toHaveBeenLastCalledWith('wizards');
    await runTextCommand('cut', el, h);
    expect(h.writeClipboard).toHaveBeenLastCalledWith('wizards');
    expect(h.exec).toHaveBeenLastCalledWith('delete');
  });

  it('does nothing for a disabled item', async () => {
    const el = input({ type: 'text', readonly: '' }, 'Grumpy');
    el.setSelectionRange(2, 2);
    const h = host();
    expect(await runTextCommand('copy', el, h)).toBe(false);
    expect(await runTextCommand('cut', el, h)).toBe(false);
    expect(await runTextCommand('paste', el, h)).toBe(false);
    expect(h.exec).not.toHaveBeenCalled();
    expect(h.readClipboard).not.toHaveBeenCalled();
  });

  it('Select all selects the whole text and keeps the focus in the field', async () => {
    const el = input({ type: 'text' }, 'Grumpy wizards');
    el.setSelectionRange(2, 2);
    expect(await runTextCommand('selectAll', el, host())).toBe(true);
    expect([el.selectionStart, el.selectionEnd]).toEqual([0, 14]);
    expect(document.activeElement).toBe(el);
  });

  it('Paste inserts the clipboard text as typing it would', async () => {
    const el = input({ type: 'text' }, 'Grumpy wizards');
    el.setSelectionRange(7, 14);
    const h = host();
    expect(await runTextCommand('paste', el, h)).toBe(true);
    expect(h.exec).toHaveBeenCalledWith('insertText', 'pasted');
  });

  it('Paste replaces the selection itself where the webview has no insertText', async () => {
    const el = input({ type: 'text' }, 'Grumpy wizards');
    el.setSelectionRange(7, 14);
    const seen: string[] = [];
    el.addEventListener('input', () => seen.push(el.value));
    expect(await runTextCommand('paste', el, host({ exec: vi.fn(() => false) }))).toBe(true);
    expect(el.value).toBe('Grumpy pasted');
    expect(el.selectionStart).toBe(13);
    expect(seen).toEqual(['Grumpy pasted']);
  });

  it('Paste with an empty clipboard changes nothing', async () => {
    const el = input({ type: 'text' }, 'Grumpy');
    const h = host({ readClipboard: vi.fn(async () => '') });
    expect(await runTextCommand('paste', el, h)).toBe(false);
    expect(el.value).toBe('Grumpy');
    expect(h.exec).not.toHaveBeenCalled();
  });
});

describe('menuPoint', () => {
  it('opens at the pointer for a right-click', () => {
    const el = input();
    const e = new MouseEvent('contextmenu', { button: 2, buttons: 2, clientX: 40, clientY: 60 });
    expect(menuPoint(e, el)).toEqual({ x: 40, y: 60 });
  });

  it('opens under the field for the Menu key', () => {
    const el = input();
    vi.spyOn(el, 'getBoundingClientRect').mockReturnValue(new DOMRect(10, 20, 200, 30));
    const e = new MouseEvent('contextmenu', { button: 0, buttons: 0 });
    expect(menuPoint(e, el)).toEqual({ x: 10, y: 50 });
  });
});
