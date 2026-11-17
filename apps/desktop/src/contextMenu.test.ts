import { afterEach, describe, expect, it } from 'vitest';
import { blockWebviewMenu } from './contextMenu';

const contextMenu = () => new MouseEvent('contextmenu', { bubbles: true, cancelable: true });

describe('blockWebviewMenu (E54.1)', () => {
  let undo: (() => void) | undefined;
  afterEach(() => {
    undo?.();
    undo = undefined;
    document.body.replaceChildren();
  });

  it('cancels a context menu on any element, text fields included', () => {
    undo = blockWebviewMenu(window);
    const div = document.body.appendChild(document.createElement('div'));
    const input = document.body.appendChild(document.createElement('input'));
    for (const el of [div, input, document.body]) {
      const e = contextMenu();
      el.dispatchEvent(e);
      expect(e.defaultPrevented).toBe(true);
    }
  });

  it('still delivers the event to a component, so its own menu opens', () => {
    undo = blockWebviewMenu(window);
    const el = document.body.appendChild(document.createElement('button'));
    let seen = false;
    el.addEventListener('contextmenu', () => (seen = true));
    el.dispatchEvent(contextMenu());
    expect(seen).toBe(true);
  });

  it('cancels even when a component stops the propagation', () => {
    undo = blockWebviewMenu(window);
    const el = document.body.appendChild(document.createElement('div'));
    el.addEventListener('contextmenu', (e) => e.stopPropagation());
    const e = contextMenu();
    el.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(true);
  });

  it('leaves other events alone (the developer tools shortcut keeps working)', () => {
    undo = blockWebviewMenu(window);
    const e = new KeyboardEvent('keydown', { key: 'F12', bubbles: true, cancelable: true });
    document.body.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
  });

  it('stops cancelling once removed', () => {
    blockWebviewMenu(window)();
    const e = contextMenu();
    document.body.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
  });
});
