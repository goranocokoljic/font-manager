import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App.vue';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { createAppRouter } from '../router';
import { useCommandsStore } from '../stores/commands';
import { useLayoutStore } from '../stores/layout';
import { useSelectionStore } from '../stores/selection';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

let wrapper: VueWrapper | undefined;

async function mountApp(path = '/') {
  const pinia = createPinia();
  setActivePinia(pinia);
  const router = createAppRouter();
  await router.push(path);
  wrapper = mount(App, { global: { plugins: [pinia, router] }, attachTo: document.body });
  await router.isReady();
  await flushPromises();
  return wrapper;
}

async function press(
  init: KeyboardEventInit,
  target: EventTarget = document.activeElement ?? window,
) {
  target.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }));
  await flushPromises();
}

const options = () => [...document.querySelectorAll('[role="option"]')];
const input = () => document.querySelector<HTMLInputElement>('[role="combobox"]');

beforeEach(() => {
  localStorage.clear(); // the layout store saves the inspector state
  setClient(createMockClient());
});

afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  setClient(undefined);
});

describe('command palette', () => {
  it('opens with Ctrl+K or Cmd+K, focused on the query, and closes with the same key', async () => {
    await mountApp();
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    await press({ key: 'k', ctrlKey: true }, window);
    expect(document.querySelector('[role="dialog"]')?.getAttribute('aria-label')).toBe(
      'Command palette',
    );
    expect(document.activeElement).toBe(input());
    await press({ key: 'k', metaKey: true });
    expect(document.querySelector('[role="dialog"]')).toBeNull();
  });

  it('lists every registered action', async () => {
    await mountApp('/?face=1');
    useCommandsStore().openPalette();
    await flushPromises();
    const listed = options().map((o) => o.textContent);
    const actions = useCommandsStore().actions;
    expect(actions.length).toBeGreaterThanOrEqual(7);
    expect(listed).toHaveLength(actions.length);
    for (const a of actions) expect(listed.some((t) => t?.includes(a.title))).toBe(true);
  });

  it('filters with a fuzzy query and highlights the matched characters', async () => {
    await mountApp();
    useCommandsStore().openPalette();
    await flushPromises();
    await wrapper!.find('[role="combobox"]').setValue('gtsb');
    expect(options()[0].textContent).toContain('Go to the sidebar');
    expect([...options()[0].querySelectorAll('mark')].map((m) => m.textContent).join('')).toBe(
      'Gtsb',
    );
    await wrapper!.find('[role="combobox"]').setValue('qqq');
    expect(options()).toHaveLength(0);
    expect(document.body.textContent).toContain('No matching commands');
  });

  it('moves with the arrows and runs the active action with Enter', async () => {
    await mountApp();
    const layout = useLayoutStore();
    expect(layout.inspectorVisible).toBe(true);
    useCommandsStore().openPalette();
    await flushPromises();
    expect(input()!.getAttribute('aria-activedescendant')).toBe(options()[0].id);
    await press({ key: 'ArrowDown' });
    expect(input()!.getAttribute('aria-activedescendant')).toBe(options()[1].id);
    await press({ key: 'ArrowUp' });
    await press({ key: 'ArrowUp' });
    expect(options().at(-1)!.getAttribute('aria-selected')).toBe('true');
    await wrapper!.find('[role="combobox"]').setValue('hide insp');
    expect(options()[0].getAttribute('aria-selected')).toBe('true');
    expect(options()[0].textContent).toContain('Show or hide the inspector');
    await press({ key: 'Enter' });
    expect(layout.inspectorVisible).toBe(false);
    expect(document.querySelector('[role="dialog"]')).toBeNull();
  });

  it('closes with Escape and returns focus to where it was', async () => {
    await mountApp();
    const button = document.querySelector<HTMLButtonElement>('button.toggle')!;
    button.focus();
    await press({ key: 'k', ctrlKey: true });
    expect(document.activeElement).toBe(input());
    await press({ key: 'Escape' });
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    expect(document.activeElement).toBe(button);
  });

  it('keeps Tab inside the dialog', async () => {
    await mountApp();
    useCommandsStore().openPalette();
    await flushPromises();
    const tab = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
    input()!.dispatchEvent(tab);
    expect(tab.defaultPrevented).toBe(true);
  });

  it('does not run a disabled action', async () => {
    await mountApp(); // nothing selected: activation is disabled
    useCommandsStore().openPalette();
    await flushPromises();
    await wrapper!.find('[role="combobox"]').setValue('for this session');
    expect(options()[0].getAttribute('aria-disabled')).toBe('true');
    const spy = vi.spyOn(useSelectionStore(), 'toggleActivation');
    await press({ key: 'Enter' });
    expect(spy).not.toHaveBeenCalled();
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
  });

  it('shows each binding next to its action', async () => {
    await mountApp('/?face=1');
    useCommandsStore().openPalette();
    await flushPromises();
    const row = options().find((o) => o.textContent?.includes('Show all commands'))!;
    expect(row.querySelector('kbd')?.textContent).toBe('Ctrl+K');
    expect(row.getAttribute('aria-keyshortcuts')).toBe('Control+K Meta+K');
    const permanent = options().find((o) => o.textContent?.includes('permanently'))!;
    expect(permanent.querySelector('kbd')?.textContent).toBe('Shift+A');
  });

  it('runs an action on click', async () => {
    await mountApp();
    useCommandsStore().openPalette();
    await flushPromises();
    const row = options().find((o) => o.textContent?.includes('Go to the sidebar'))! as HTMLElement;
    row.click();
    await flushPromises();
    expect(document.activeElement?.id).toBe('fm-sidebar');
  });

  it('opens from the status bar hint', async () => {
    await mountApp();
    await wrapper!.find('button.hint').trigger('click');
    expect(useCommandsStore().open).toBe(true);
  });
});

describe('keyboard model in the app', () => {
  it('A toggles session activation and Shift+A permanent activation of the selected face', async () => {
    await mountApp('/?face=1');
    const selection = useSelectionStore();
    await press({ key: 'a' }, window);
    expect(selection.detail?.face.active).toBe('session');
    await press({ key: 'A', shiftKey: true }, window);
    expect(selection.detail?.face.active).toBe('permanent');
    await press({ key: 'A', shiftKey: true }, window);
    expect(selection.detail?.face.active).toBeNull();
    await press({ key: 'a' }, window);
    await press({ key: 'a' }, window);
    expect(selection.detail?.face.active).toBeNull();
  });

  it('A does nothing for an unsupported face or with nothing selected', async () => {
    await mountApp('/?face=7');
    const spy = vi.spyOn(useSelectionStore(), 'toggleActivation');
    await press({ key: 'a' }, window);
    expect(spy).not.toHaveBeenCalled();
  });

  it('Ctrl/Cmd+I still toggles the inspector', async () => {
    await mountApp();
    const layout = useLayoutStore();
    await press({ key: 'i', ctrlKey: true }, window);
    expect(layout.inspectorVisible).toBe(false);
    await press({ key: 'i', metaKey: true }, window);
    expect(layout.inspectorVisible).toBe(true);
  });

  it('makes each column a labelled tab stop in order', async () => {
    await mountApp();
    const columns = [...document.querySelectorAll<HTMLElement>('.column')];
    expect(columns.map((c) => c.getAttribute('aria-label'))).toEqual([
      'Library',
      'Fonts',
      'Inspector',
    ]);
    for (const c of columns) expect(c.tabIndex).toBe(0);
  });

  it('"Go to the inspector" reopens a hidden inspector and focuses it', async () => {
    await mountApp();
    const layout = useLayoutStore();
    layout.toggleInspector();
    await flushPromises();
    useCommandsStore().run('focus.inspector');
    await flushPromises();
    expect(layout.inspectorVisible).toBe(true);
    expect(document.activeElement?.id).toBe('fm-inspector');
  });
});
