import { afterEach, describe, expect, it } from 'vitest';
import { setPaletteOrigin, sheetReturnTarget } from './returnFocus';

function button(parent: HTMLElement = document.body): HTMLButtonElement {
  const b = document.createElement('button');
  parent.append(b);
  return b;
}

describe('sheetReturnTarget', () => {
  afterEach(() => {
    setPaletteOrigin(null);
    document.body.replaceChildren();
  });

  it('is the focused element outside any dialog', () => {
    const b = button();
    b.focus();
    expect(sheetReturnTarget()).toBe(b);
  });

  it('is the palette origin when the focus is in a dialog', () => {
    const origin = button();
    setPaletteOrigin(origin);
    const dialog = document.createElement('div');
    dialog.setAttribute('role', 'dialog');
    document.body.append(dialog);
    button(dialog).focus();
    expect(sheetReturnTarget()).toBe(origin);
  });

  it('is the palette origin when the focus is nowhere', () => {
    const origin = button();
    setPaletteOrigin(origin);
    expect(sheetReturnTarget()).toBe(origin);
  });

  it('falls back to the list column when the origin is gone', () => {
    const origin = button();
    setPaletteOrigin(origin);
    origin.remove();
    const main = document.createElement('main');
    document.body.append(main);
    expect(sheetReturnTarget()).toBe(main);
  });
});
