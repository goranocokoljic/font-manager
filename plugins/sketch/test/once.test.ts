import { FmApiError, NOT_RUNNING, NotRunningError, type Transport } from '@fm/plugin-shared';
import { describe, expect, it, vi } from 'vitest';
import { notRunningOnce, rememberingReached } from '../src/once';
import { mockSettings } from './mock-sketch';

const failing = (error: Error): Transport => ({
  call: async () => {
    throw error;
  },
});

describe('notRunningOnce', () => {
  it('passes every other message through, each time', () => {
    const ui = { notify: vi.fn(), confirm: vi.fn(async () => true) };
    const once = notRunningOnce(ui, mockSettings());
    once.notify('1 font missing: A');
    once.notify('1 font missing: A');
    once.notify(NOT_RUNNING);
    once.notify(NOT_RUNNING);
    expect(ui.notify.mock.calls).toEqual([
      ['1 font missing: A'],
      ['1 font missing: A'],
      [NOT_RUNNING],
    ]);
  });

  it('says "not running" again once the app answered with an error', async () => {
    const settings = mockSettings();
    const ui = { notify: vi.fn(), confirm: vi.fn(async () => true) };
    notRunningOnce(ui, settings).notify(NOT_RUNNING);
    const answered = failing(new FmApiError({ code: 'notFound', message: 'No such face.' }));
    await expect(rememberingReached(answered, settings).call('x', {})).rejects.toThrow();
    notRunningOnce(ui, settings).notify(NOT_RUNNING);
    expect(ui.notify).toHaveBeenCalledTimes(2);
  });

  it('stays quiet while the app is still not reachable', async () => {
    const settings = mockSettings();
    const ui = { notify: vi.fn(), confirm: vi.fn(async () => true) };
    notRunningOnce(ui, settings).notify(NOT_RUNNING);
    const down = failing(new NotRunningError('no endpoint.json'));
    await expect(rememberingReached(down, settings).call('x', {})).rejects.toThrow(NOT_RUNNING);
    notRunningOnce(ui, settings).notify(NOT_RUNNING);
    expect(ui.notify).toHaveBeenCalledTimes(1);
  });
});
