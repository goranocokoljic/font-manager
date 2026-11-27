import { describe, expect, it } from 'vitest';
import { MOCK_SETTINGS_KEY, createMockClient } from './mock';
import type { Event } from './types';

/** A storage the test can read, as the browser's session storage. */
function memoryStorage() {
  const saved = new Map<string, string>();
  return {
    saved,
    getItem: (k: string) => saved.get(k) ?? null,
    setItem: (k: string, v: string) => void saved.set(k, v),
  };
}

describe('MockApi interface size (E58.5)', () => {
  it('starts at Default and answers the factor of each size, as fm-api does', async () => {
    const settings = await createMockClient().getSettings();
    expect(settings.interfaceSize).toBe('default');
    expect(settings.interfaceSizes).toEqual([
      { size: 'default', factor: 1 },
      { size: 'large', factor: 1.25 },
      { size: 'extraLarge', factor: 1.5 },
    ]);
    expect(settings.droppedInterfaceSize).toBeNull();
  });

  it('saves a size, announces a change with its factor and nothing for the same size', async () => {
    const client = createMockClient();
    const events: Event[] = [];
    await client.onEvent((e) => events.push(e));
    const saved = await client.updateSettings({ interfaceSize: 'extraLarge' });
    expect(saved.interfaceSize).toBe('extraLarge');
    expect(events).toEqual([
      { type: 'interfaceSizeChanged', interfaceSize: { size: 'extraLarge', factor: 1.5 } },
    ]);
    await client.updateSettings({ interfaceSize: 'extraLarge', density: 'compact' });
    expect(events).toHaveLength(1);
    expect((await client.getSettings()).interfaceSize).toBe('extraLarge');
    await expect(
      // @ts-expect-error a size fm-api does not have
      client.updateSettings({ interfaceSize: 'huge' }),
    ).rejects.toMatchObject({ code: 'invalidRequest' });
  });

  it('reads a saved size it does not have as Default and reports it once', async () => {
    const storage = memoryStorage();
    storage.setItem(
      MOCK_SETTINGS_KEY,
      JSON.stringify({ interfaceSize: 'gigantic', theme: 'dark' }),
    );
    const client = createMockClient({ settingsStorage: storage });
    const first = await client.getSettings();
    expect(first.interfaceSize).toBe('default');
    expect(first.droppedInterfaceSize).toBe('gigantic');
    expect(first.theme).toBe('dark');
    expect(JSON.parse(storage.saved.get(MOCK_SETTINGS_KEY)!).interfaceSize).toBe('default');
    expect((await client.getSettings()).droppedInterfaceSize).toBeNull();
  });
});
