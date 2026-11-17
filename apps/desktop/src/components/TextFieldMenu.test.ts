import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import TextFieldMenu from './TextFieldMenu.vue';

let wrapper: VueWrapper | undefined;
let exec: ReturnType<typeof vi.fn>;

async function setup(attrs = '') {
  document.body.innerHTML = `<input id="f" type="text" value="Grumpy wizards" ${attrs}><div id="plain"></div>`;
  const client = createMockClient();
  client.readClipboardText = async () => 'pasted';
  setClient(client);
  wrapper = mount(TextFieldMenu, { attachTo: document.body });
  const field = document.getElementById('f') as HTMLInputElement;
  field.focus();
  return field;
}

const menu = () => document.querySelector<HTMLElement>('[data-testid="text-field-menu"]');
const items = () => [...(menu()?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])];
const item = (name: string) => items().find((b) => b.querySelector('.label')?.textContent === name);

async function rightClick(el: Element, x = 30, y = 40) {
  el.dispatchEvent(
    new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      button: 2,
      buttons: 2,
      clientX: x,
      clientY: y,
    }),
  );
  await flushPromises();
}
async function key(k: string, opts: KeyboardEventInit = {}) {
  const target = document.activeElement ?? document.body;
  const e = new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...opts });
  target.dispatchEvent(e);
  await flushPromises();
  return e;
}

beforeEach(() => {
  exec = vi.fn(() => true);
  document.execCommand = exec as unknown as typeof document.execCommand;
});
afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  document.body.innerHTML = '';
  setClient(undefined);
});

describe('TextFieldMenu', () => {
  it('opens on a text field with exactly Cut, Copy, Paste and Select all', async () => {
    const field = await setup();
    await rightClick(field);
    expect(menu()?.getAttribute('role')).toBe('menu');
    expect(items().map((b) => b.querySelector('.label')?.textContent)).toEqual([
      'Cut',
      'Copy',
      'Paste',
      'Select all',
    ]);
  });

  it('opens nowhere else', async () => {
    await setup();
    await rightClick(document.getElementById('plain')!);
    expect(menu()).toBeNull();
  });

  it('disables Cut and Copy without a selection', async () => {
    const field = await setup();
    field.setSelectionRange(2, 2);
    await rightClick(field);
    expect(items().map((b) => b.disabled)).toEqual([true, true, false, false]);
  });

  it('disables Cut and Paste in a read-only field', async () => {
    const field = await setup('readonly');
    field.setSelectionRange(0, 6);
    await rightClick(field);
    expect(items().map((b) => b.disabled)).toEqual([true, false, true, false]);
  });

  it('keeps the focus and the selection in the field while open and after', async () => {
    const field = await setup();
    field.setSelectionRange(0, 6);
    await rightClick(field);
    expect(document.activeElement).toBe(field);
    await key('Escape');
    expect(menu()).toBeNull();
    expect(document.activeElement).toBe(field);
    expect([field.selectionStart, field.selectionEnd]).toEqual([0, 6]);
  });

  it('is keyboard-complete: arrows highlight, Enter runs, Esc closes and stays in the field', async () => {
    const field = await setup();
    field.setSelectionRange(0, 6);
    await rightClick(field);
    await key('ArrowDown');
    expect(item('Cut')?.classList.contains('active')).toBe(true);
    expect(field.getAttribute('aria-activedescendant')).toBe('fm-text-menu-cut');
    await key('ArrowDown');
    expect(item('Copy')?.classList.contains('active')).toBe(true);
    await key('End');
    expect(item('Select all')?.classList.contains('active')).toBe(true);
    await key('ArrowDown');
    expect(item('Cut')?.classList.contains('active')).toBe(true);
    await key('ArrowUp');
    await key('ArrowUp');
    expect(item('Paste')?.classList.contains('active')).toBe(true);
    const esc = await key('Escape');
    expect(esc.defaultPrevented).toBe(true);
    expect(menu()).toBeNull();
    expect(field.hasAttribute('aria-activedescendant')).toBe(false);

    await rightClick(field);
    await key('ArrowDown');
    await key('ArrowDown');
    const enter = await key('Enter');
    expect(enter.defaultPrevented).toBe(true);
    expect(exec).toHaveBeenCalledWith('copy', false, undefined);
    expect(menu()).toBeNull();
    expect(document.activeElement).toBe(field);
  });

  it('skips disabled items with the arrows', async () => {
    const field = await setup();
    field.setSelectionRange(3, 3);
    await rightClick(field);
    await key('ArrowDown');
    expect(item('Paste')?.classList.contains('active')).toBe(true);
    await key('ArrowDown');
    expect(item('Select all')?.classList.contains('active')).toBe(true);
  });

  it('highlights the first enabled item when opened from the keyboard', async () => {
    const field = await setup();
    field.setSelectionRange(3, 3);
    field.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, button: 0, buttons: 0 }));
    await flushPromises();
    expect(item('Paste')?.classList.contains('active')).toBe(true);
  });

  it('a click runs the item: Select all, Paste', async () => {
    const field = await setup();
    field.setSelectionRange(3, 3);
    await rightClick(field);
    item('Select all')!.click();
    await flushPromises();
    expect([field.selectionStart, field.selectionEnd]).toEqual([0, 14]);
    expect(menu()).toBeNull();

    await rightClick(field);
    item('Paste')!.click();
    await flushPromises();
    expect(exec).toHaveBeenCalledWith('insertText', false, 'pasted');
  });

  it('another key closes the menu and goes through', async () => {
    const field = await setup();
    await rightClick(field);
    await key('Shift');
    expect(menu()).not.toBeNull();
    const e = await key('c', { ctrlKey: true });
    expect(e.defaultPrevented).toBe(false);
    expect(menu()).toBeNull();
    expect(document.activeElement).toBe(field);
  });

  it('closes when the field loses the focus', async () => {
    const field = await setup();
    await rightClick(field);
    field.blur();
    await flushPromises();
    expect(menu()).toBeNull();
  });

  it('opens at the pointer', async () => {
    const field = await setup();
    await rightClick(field, 120, 90);
    const layer = document.querySelector<HTMLElement>('[data-ui-popover]');
    expect(layer?.style.left).toBe('120px');
    expect(layer?.style.top).toBe('94px');
  });
});
