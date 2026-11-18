import { describe, expect, it } from 'vitest';
import cases from '../../../../fixtures/setup/status-cases.json';
import { createMockClient, mockOpenSetupSteps, type MockSetupFacts } from './mock';
import type { SetupStep } from './types';

describe('MockApi setup status (E55.2)', () => {
  it('gives the core answer for every shared case', () => {
    expect(cases.length).toBeGreaterThanOrEqual(10);
    for (const c of cases) {
      expect(mockOpenSetupSteps(c.state as MockSetupFacts), c.name).toEqual(c.open as SetupStep[]);
    }
  });

  it('follows the mock state, and an answer is kept with the mock, not in browser storage', async () => {
    const written: string[] = [];
    const storage = {
      getItem: () => null,
      setItem: (key: string) => void written.push(key),
      removeItem: () => {},
    } as unknown as Storage;
    const client = createMockClient({ settingsStorage: storage });
    const open = async () => (await client.setupStatus()).open;
    // The sample library has a watched folder; every catalog is off.
    expect(await open()).toEqual(['catalogs', 'updates']);
    await client.setCatalogEnabled({ id: 'fontshare', enabled: true });
    expect(await open()).toEqual(['downloadFolder', 'updates']);
    await expect(
      client.answerSetupStep({ step: 'downloadFolder', answer: 'skipped' }),
    ).rejects.toMatchObject({ code: 'invalidRequest' });
    await client.updateSettings({ downloadFolder: '/Users/demo/Font Fu Fonts' });
    expect(await open()).toEqual(['updates']);
    written.length = 0;
    const status = await client.answerSetupStep({ step: 'updates', answer: 'skipped' });
    expect(status.open).toEqual([]);
    expect(await open()).toEqual([]);
    expect(written, 'nothing written to browser storage').toEqual([]);
  });

  it('never asks about updates where a package manager installs them', async () => {
    const client = createMockClient({ update: 'managed' });
    expect((await client.setupStatus()).open).toEqual(['catalogs']);
  });
});
