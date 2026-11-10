import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TOAST_MS, TOAST_STACK, useToastStore } from './toast';

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

  it('stacks a new toast on top, and each one goes after its own time (E48.6)', () => {
    const toasts = useToastStore();
    toasts.show('First.');
    vi.advanceTimersByTime(TOAST_MS - 10);
    toasts.show('Second.');
    expect(toasts.toasts.map((t) => t.message)).toEqual(['Second.', 'First.']);
    expect(toasts.toast?.message).toBe('Second.');
    vi.advanceTimersByTime(10);
    expect(toasts.toasts.map((t) => t.message)).toEqual(['Second.']);
    vi.advanceTimersByTime(TOAST_MS - 20);
    expect(toasts.toast?.message).toBe('Second.');
    vi.advanceTimersByTime(10);
    expect(toasts.toasts).toEqual([]);
  });

  it('keeps at most three, newest first: a fourth pushes the oldest out (E48.6)', () => {
    expect(TOAST_STACK).toBe(3);
    const toasts = useToastStore();
    const run = vi.fn();
    toasts.show('One.', { label: 'Undo', run });
    toasts.show('Two.');
    toasts.show('Three.');
    toasts.show('Four.');
    expect(toasts.toasts.map((t) => t.message)).toEqual(['Four.', 'Three.', 'Two.']);
    // The one pushed out takes its timer with it; the rest keep theirs.
    vi.advanceTimersByTime(TOAST_MS);
    expect(toasts.toasts).toEqual([]);
    expect(run).not.toHaveBeenCalled();
  });

  it('dismisses, holds and acts on one toast of the stack by id (E48.6)', () => {
    const toasts = useToastStore();
    const run = vi.fn();
    toasts.show('Undoable.', { label: 'Undo', run });
    const undoable = toasts.toast!.id;
    toasts.show('Plain.');
    toasts.show('Other.');
    toasts.dismiss(toasts.toasts[1]!.id);
    expect(toasts.toasts.map((t) => t.message)).toEqual(['Other.', 'Undoable.']);
    toasts.hold(undoable);
    vi.advanceTimersByTime(TOAST_MS * 2);
    expect(toasts.toasts.map((t) => t.message)).toEqual(['Undoable.']);
    toasts.act(undoable);
    expect(run).toHaveBeenCalledTimes(1);
    expect(toasts.toasts).toEqual([]);
    toasts.show('A.');
    toasts.show('B.');
    toasts.clear();
    expect(toasts.toasts).toEqual([]);
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
