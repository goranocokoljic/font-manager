import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ref } from 'vue';
import { ACTIONS } from '../keyboard/keymap';
import { useCommandsStore, type Action } from './commands';

const key = (init: KeyboardEventInit) =>
  new KeyboardEvent('keydown', { cancelable: true, ...init });

function action(id: string, title: string, extra: Partial<Action> = {}): Action {
  return { id, title, group: 'Test', run: vi.fn(), ...extra };
}

beforeEach(() => setActivePinia(createPinia()));

describe('commands store', () => {
  it('registers, replaces and unregisters actions', () => {
    const commands = useCommandsStore();
    const undo = commands.register(action('x', 'First'));
    commands.register(action('x', 'Second'));
    expect(commands.actions.map((a) => a.title)).toEqual(['Second']);
    undo(); // The first registration is gone already; this must not drop the second.
    expect(commands.actions).toHaveLength(1);
  });

  it('runs an enabled action and refuses a disabled or unknown one', () => {
    const commands = useCommandsStore();
    const on = action('on', 'On');
    const off = action('off', 'Off', { enabled: () => false });
    commands.register(on);
    commands.register(off);
    expect(commands.run('on')).toBe(true);
    expect(on.run).toHaveBeenCalledOnce();
    expect(commands.run('off')).toBe(false);
    expect(off.run).not.toHaveBeenCalled();
    expect(commands.run('nope')).toBe(false);
  });

  it('follows a reactive enabled flag', () => {
    const commands = useCommandsStore();
    const allowed = ref(false);
    commands.register(action('x', 'X', { enabled: allowed }));
    expect(commands.items[0].enabled).toBe(false);
    allowed.value = true;
    expect(commands.items[0].enabled).toBe(true);
  });

  it('lists every action, enabled or not, in registration order for no query', () => {
    const commands = useCommandsStore();
    commands.register(action('b', 'Beta', { enabled: false }));
    commands.register(action('a', 'Alpha'));
    expect(commands.items.map((i) => i.action.id)).toEqual(['b', 'a']);
    expect(commands.items.map((i) => i.enabled)).toEqual([false, true]);
  });

  it('filters and ranks by fuzzy query, with the matched characters', () => {
    const commands = useCommandsStore();
    commands.register(action('insp', 'Show or hide the inspector'));
    commands.register(action('side', 'Go to the sidebar'));
    commands.register(action('list', 'Go to the font list'));
    commands.query = 'side';
    expect(commands.items.map((i) => i.action.id)).toEqual(['side', 'insp']);
    expect(commands.items[0].indices).toEqual([10, 11, 12, 13]);
    commands.query = 'zzz';
    expect(commands.items).toEqual([]);
  });

  it('matches the group name too, without highlighting the title', () => {
    const commands = useCommandsStore();
    commands.register({ ...action('a', 'Toggle permanently'), group: 'Activation' });
    commands.query = 'activation';
    expect(commands.items.map((i) => i.action.id)).toEqual(['a']);
    expect(commands.items[0].indices).toEqual([]);
  });

  it('shows the binding of an action', () => {
    const commands = useCommandsStore();
    commands.register(action(ACTIONS.palette, 'Show all commands'));
    commands.register(action('x', 'Unbound'));
    expect(commands.items[0].chord).toEqual({ key: 'k', mod: true });
    expect(commands.items[1].chord).toBeUndefined();
  });

  it('opens with an empty query, closes and toggles', () => {
    const commands = useCommandsStore();
    commands.query = 'old';
    commands.openPalette();
    expect(commands.open).toBe(true);
    expect(commands.query).toBe('');
    commands.togglePalette();
    expect(commands.open).toBe(false);
  });
});

describe('shortcut dispatch', () => {
  it('runs the action bound to the key and consumes the key', () => {
    const commands = useCommandsStore();
    const activate = action(ACTIONS.activateSession, 'Activate');
    commands.register(activate);
    const e = key({ key: 'a' });
    expect(commands.handleKey(e)).toBe(true);
    expect(activate.run).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it('lets a key through when its action is not registered or disabled', () => {
    const commands = useCommandsStore();
    // Settings (E5.9) is bound but not built yet.
    const e = key({ key: ',', ctrlKey: true });
    expect(commands.handleKey(e)).toBe(false);
    expect(e.defaultPrevented).toBe(false);
    commands.register(action(ACTIONS.compare, 'Compare', { enabled: false }));
    const c = key({ key: 'c' });
    expect(commands.handleKey(c)).toBe(false);
    expect(c.defaultPrevented).toBe(false);
  });

  it('starts working once the owning story registers the action', () => {
    const commands = useCommandsStore();
    const settings = action(ACTIONS.settings, 'Open settings');
    commands.register(settings);
    expect(commands.handleKey(key({ key: ',', metaKey: true }))).toBe(true);
    expect(settings.run).toHaveBeenCalledOnce();
  });

  it('ignores keys another handler already used, IME composition and plain-key repeat', () => {
    const commands = useCommandsStore();
    const activate = action(ACTIONS.activateSession, 'Activate');
    commands.register(activate);
    const used = key({ key: 'a' });
    used.preventDefault();
    expect(commands.handleKey(used)).toBe(false);
    expect(commands.handleKey(key({ key: 'a', isComposing: true }))).toBe(false);
    expect(commands.handleKey(key({ key: 'a', repeat: true }))).toBe(false);
    expect(activate.run).not.toHaveBeenCalled();
  });

  it('only lets the palette toggle through while the palette is open', () => {
    const commands = useCommandsStore();
    const activate = action(ACTIONS.activateSession, 'Activate');
    commands.register(activate);
    commands.register(action(ACTIONS.palette, 'Palette', { run: commands.togglePalette }));
    commands.openPalette();
    expect(commands.handleKey(key({ key: 'a' }))).toBe(false);
    expect(activate.run).not.toHaveBeenCalled();
    expect(commands.handleKey(key({ key: 'k', ctrlKey: true }))).toBe(true);
    expect(commands.open).toBe(false);
  });
});
