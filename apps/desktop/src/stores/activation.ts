import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type { Event } from '../api/types';

/**
 * How many faces the app has active, for the status bar. Read from fm-api at
 * start and again on every `activationChanged` event; the UI never polls.
 */
export const useActivationStore = defineStore('activation', () => {
  const count = ref(0);
  let unsubscribe: Promise<Unsubscribe> | undefined;
  /** Bumped by every read; an answer to an older read is dropped. */
  let round = 0;

  async function refresh(): Promise<void> {
    const mine = ++round;
    try {
      const active = await useClient().listActive();
      if (mine === round) count.value = active.length;
    } catch {
      // The count keeps its value; the next change reads it again.
    }
  }

  function onEvent(event: Event): void {
    if (event.type === 'activationChanged') void refresh();
  }

  /** Subscribes to activation changes (once) and reads the count. */
  async function load(): Promise<void> {
    unsubscribe ??= useClient().onEvent(onEvent);
    await unsubscribe.catch(() => undefined);
    await refresh();
  }

  /** Stops listening to activation changes. */
  async function stop(): Promise<void> {
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  return { count, load, stop, refresh };
});
