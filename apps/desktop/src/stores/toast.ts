import { defineStore } from 'pinia';
import { shallowRef } from 'vue';

/** A button in a toast, e.g. Undo. */
export interface ToastAction {
  label: string;
  run: () => unknown;
}

/** A short message at the bottom of the window. */
export interface Toast {
  id: number;
  message: string;
  action?: ToastAction;
}

/** How long a toast stays, in ms; long enough to reach its Undo button. */
export const TOAST_MS = 10_000;

/** The one toast on screen; a new one replaces it. */
export const useToastStore = defineStore('toast', () => {
  const toast = shallowRef<Toast | null>(null);
  let nextId = 1;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function dismiss(): void {
    clearTimeout(timer);
    timer = undefined;
    toast.value = null;
  }

  function arm(id: number): void {
    clearTimeout(timer);
    timer = setTimeout(() => {
      if (toast.value?.id === id) dismiss();
    }, TOAST_MS);
  }

  /** Shows a toast; it goes away after `TOAST_MS` or when dismissed. */
  function show(message: string, action?: ToastAction): void {
    const id = nextId++;
    toast.value = { id, message, action };
    arm(id);
  }

  /** The pointer or the focus is on the toast: it stays until `release`. */
  function hold(): void {
    clearTimeout(timer);
    timer = undefined;
  }

  /** The pointer or the focus left the toast: it goes away after `TOAST_MS` again. */
  function release(): void {
    if (toast.value) arm(toast.value.id);
  }

  /** Runs the toast's action and dismisses the toast. */
  function act(): void {
    const action = toast.value?.action;
    dismiss();
    void action?.run();
  }

  return { toast, show, dismiss, act, hold, release };
});
