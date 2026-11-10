import { defineStore } from 'pinia';
import { computed, shallowRef } from 'vue';

/** A button in a toast, e.g. Undo. */
export interface ToastAction {
  label: string;
  run: () => unknown;
}

/** A short message at the top right of the window, under the toolbar (E48.6). */
export interface Toast {
  id: number;
  message: string;
  action?: ToastAction;
}

/** How long a toast stays, in ms; long enough to reach its Undo button. */
export const TOAST_MS = 10_000;

/** How many toasts show at once (E48.6); a fourth pushes the oldest out. */
export const TOAST_STACK = 3;

/** The toasts on screen, newest first, at most `TOAST_STACK`; each goes after its own time. */
export const useToastStore = defineStore('toast', () => {
  const toasts = shallowRef<Toast[]>([]);
  /** The newest toast, or null. */
  const toast = computed<Toast | null>(() => toasts.value[0] ?? null);
  let nextId = 1;
  const timers = new Map<number, ReturnType<typeof setTimeout>>();
  /** How long each toast stays. */
  const stays = new Map<number, number>();

  function forget(id: number): void {
    clearTimeout(timers.get(id));
    timers.delete(id);
    stays.delete(id);
  }

  /** Takes a toast away: the one with `id`, or the newest. */
  function dismiss(id?: number): void {
    const gone = id ?? toast.value?.id;
    if (gone === undefined) return;
    forget(gone);
    toasts.value = toasts.value.filter((t) => t.id !== gone);
  }

  /** Takes every toast away. */
  function clear(): void {
    for (const t of toasts.value) forget(t.id);
    toasts.value = [];
  }

  function arm(id: number): void {
    clearTimeout(timers.get(id));
    timers.set(
      id,
      setTimeout(() => dismiss(id), stays.get(id) ?? TOAST_MS),
    );
  }

  /** Shows a toast on top; it goes away after `ms` (default `TOAST_MS`) or when dismissed. */
  function show(message: string, action?: ToastAction, ms: number = TOAST_MS): void {
    const id = nextId++;
    stays.set(id, ms);
    const next = [{ id, message, action }, ...toasts.value];
    for (const old of next.slice(TOAST_STACK)) forget(old.id);
    toasts.value = next.slice(0, TOAST_STACK);
    arm(id);
  }

  /** The pointer or the focus is on a toast (the newest by default): it stays until `release`. */
  function hold(id?: number): void {
    const on = id ?? toast.value?.id;
    if (on === undefined) return;
    clearTimeout(timers.get(on));
    timers.delete(on);
  }

  /** The pointer or the focus left the toast: it goes away after its time again. */
  function release(id?: number): void {
    const on = id ?? toast.value?.id;
    if (on === undefined || !toasts.value.some((t) => t.id === on)) return;
    arm(on);
  }

  /** Runs a toast's action (the newest toast's by default) and dismisses that toast. */
  function act(id?: number): void {
    const target = id === undefined ? toast.value : toasts.value.find((t) => t.id === id);
    if (!target) return;
    dismiss(target.id);
    void target.action?.run();
  }

  return { toasts, toast, show, dismiss, clear, act, hold, release };
});
