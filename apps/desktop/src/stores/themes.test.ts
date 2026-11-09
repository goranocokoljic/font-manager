import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import moss from '../../../../data/themes/moss.json';
import { useSettingsStore } from './settings';
import { THEME_CHANGE_CLASS, useThemesStore } from './themes';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

let client: FmClient;
const root = document.documentElement;
const accent = () => root.style.getPropertyValue('--fm-accent');

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
  root.removeAttribute('style');
});

afterEach(() => setClient(undefined));

async function loaded() {
  const settings = useSettingsStore();
  const themes = useThemesStore();
  await Promise.all([settings.load(), themes.load()]);
  return { settings, themes };
}

describe('themes store (E47.2)', () => {
  it('writes nothing before the themes are read; Paper is theme.css itself', async () => {
    const themes = useThemesStore();
    themes.apply();
    expect(accent()).toBe('');
    await loaded();
    expect(themes.active?.id).toBe('paper');
    themes.apply();
    expect(accent(), 'the style sheet applies, no inline copy').toBe('');
  });

  it('takes the inline tokens off again when Paper is chosen back', async () => {
    const { themes } = await loaded();
    await themes.choose('moss');
    expect(accent()).toBe(moss.light.accent);
    await themes.choose('paper');
    expect(accent()).toBe('');
    expect(root.style.getPropertyValue('--fm-border')).toBe('');
    expect((await client.getSettings()).themeId).toBe('paper');
  });

  it('applies a chosen theme at once and saves its id', async () => {
    const { themes } = await loaded();
    const saving = themes.choose('moss');
    // Before the save answers, the tokens are already on :root.
    await Promise.resolve();
    expect(accent()).toBe(moss.light.accent);
    expect(root.style.getPropertyValue('--fm-surface-2')).toBe(moss.light['surface-2']);
    expect(await saving).toBe(true);
    expect((await client.getSettings()).themeId).toBe('moss');
    expect(themes.activeId).toBe('moss');
    expect(root.classList.contains(THEME_CHANGE_CLASS), 'the transition class is gone').toBe(false);
  });

  it('uses the dark set under the Dark setting', async () => {
    const { settings, themes } = await loaded();
    await settings.update({ theme: 'dark' });
    await themes.choose('moss');
    expect(themes.scheme).toBe('dark');
    expect(accent()).toBe(moss.dark.accent);
  });

  it('follows the OS appearance under System', async () => {
    let listener: ((e: { matches: boolean }) => void) | undefined;
    const matchMedia = vi.fn(() => ({
      matches: false,
      addEventListener: (_: string, fn: typeof listener) => (listener = fn),
    }));
    vi.stubGlobal('matchMedia', matchMedia);
    try {
      const { themes } = await loaded();
      await themes.choose('moss');
      expect(themes.scheme).toBe('light');
      expect(accent()).toBe(moss.light.accent);
      listener!({ matches: true });
      expect(themes.scheme).toBe('dark');
      themes.apply();
      expect(accent()).toBe(moss.dark.accent);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('goes back to the saved theme when the save is refused', async () => {
    const { themes } = await loaded();
    vi.spyOn(client, 'updateSettings').mockRejectedValueOnce({ message: 'no' });
    expect(await themes.choose('moss')).toBe(false);
    expect(themes.activeId).toBe('paper');
    expect(accent(), 'back to theme.css').toBe('');
  });
});
