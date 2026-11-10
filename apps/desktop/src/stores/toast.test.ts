import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TOAST_MS, useToastStore } from './toast';

beforeEach(() => {
  setActivePinia(createPinia());
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('toast store', () => {
  it('shows a toast and hides it after TOAST_MS', () => {
    const toasts = useToastStore();
    toasts.show('Done.');
    expect(toasts.toast?.message).toBe('Done.');
    vi.advanceTimersByTime(TOAST_MS - 1);
    expect(toasts.toast).not.toBeNull();
    vi.advanceTimersByTime(1);
    expect(toasts.toast).toBeNull();
  });

  it('keeps a toast for its own time, also after a hold (E48.5)', () => {
    const toasts = useToastStore();
    toasts.show('Short.', { label: 'Undo', run: () => undefined }, 5_000);
    vi.advanceTimersByTime(4_999);
    expect(toasts.toast).not.toBeNull();
    toasts.hold();
    vi.advanceTimersByTime(TOAST_MS);
    toasts.release();
    vi.advanceTimersByTime(4_999);
    expect(toasts.toast).not.toBeNull();
    vi.advanceTimersByTime(1);
    expect(toasts.toast).toBeNull();
    toasts.show('Long.');
    vi.advanceTimersByTime(5_000);
    expect(toasts.toast?.message).toBe('Long.');
  });

  it('replaces the toast and restarts the time', () => {
    const toasts = useToastStore();
    toasts.show('First.');
    vi.advanceTimersByTime(TOAST_MS - 10);
    toasts.show('Second.');
    vi.advanceTimersByTime(TOAST_MS - 10);
    expect(toasts.toast?.message).toBe('Second.');
  });

  it('stays while held and goes TOAST_MS after release', () => {
    const toasts = useToastStore();
    toasts.show('Held.');
    toasts.hold();
    vi.advanceTimersByTime(TOAST_MS * 3);
    expect(toasts.toast?.message).toBe('Held.');
    toasts.release();
    vi.advanceTimersByTime(TOAST_MS);
    expect(toasts.toast).toBeNull();
  });

  it('runs the action once and dismisses the toast', () => {
    const toasts = useToastStore();
    const run = vi.fn();
    toasts.show('Changed.', { label: 'Undo', run });
    toasts.act();
    toasts.act();
    expect(run).toHaveBeenCalledTimes(1);
    expect(toasts.toast).toBeNull();
  });
});
