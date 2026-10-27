import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type { Event } from '../api/types';
import { t } from '../i18n';
import { useToastStore } from './toast';

/** An AI assistant's call that waits for the user's Allow or Deny (E27.6). */
export interface McpConfirmation {
  id: number;
  tool: string;
  /** How many faces the call acts on, when fm-core knows. */
  faces: number | null;
  timeoutSecs: number;
}

/**
 * The confirmations fm-core asks for, from the event stream: one toast each, oldest
 * first. fm-core decides and times out; this store only shows the question and sends
 * the answer.
 */
export const useMcpConfirmStore = defineStore('mcpConfirm', () => {
  const waiting = ref<McpConfirmation[]>([]);
  /** The answer in flight, so the buttons cannot send it twice. */
  const busy = ref(false);
  let unsubscribe: Promise<Unsubscribe> | undefined;

  /** The confirmation on screen: the oldest one waiting. */
  const current = computed(() => waiting.value[0] ?? null);

  function remove(id: number): void {
    waiting.value = waiting.value.filter((c) => c.id !== id);
  }

  function onEvent(event: Event): void {
    if (event.type === 'mcpConfirmationRequested') {
      waiting.value = [
        ...waiting.value,
        {
          id: event.id,
          tool: event.tool,
          faces: event.faces ?? null,
          timeoutSecs: event.timeoutSecs,
        },
      ];
    } else if (event.type === 'mcpConfirmationClosed') {
      // Answered (here or elsewhere) or timed out: the toast goes away.
      remove(event.id);
    }
  }

  /** Subscribes to the confirmations (once). */
  async function start(): Promise<void> {
    unsubscribe ??= useClient().onEvent(onEvent);
    await unsubscribe.catch(() => undefined);
  }

  /** Stops listening. */
  async function stop(): Promise<void> {
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  /** Sends Allow or Deny for confirmation `id`, then says what happened. */
  async function answer(id: number, allow: boolean): Promise<void> {
    if (busy.value) return;
    busy.value = true;
    try {
      await useClient().answerMcpConfirmation({ id, allow });
      useToastStore().show(t(allow ? 'mcpConfirm.allowed' : 'mcpConfirm.denied'));
    } catch {
      // Timed out meanwhile: fm-core already refused the call.
      useToastStore().show(t('mcpConfirm.denied'));
    } finally {
      remove(id);
      busy.value = false;
    }
  }

  return { waiting, current, busy, start, stop, answer, onEvent };
});
