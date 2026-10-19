import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type { ApiError, Clash, Event, FaceId } from '../api/types';

/**
 * The Conflicts view: every clash of an active face and its winner, as fm-api
 * reports it, with the names of the faces involved. Read again on every
 * `activationChanged` event; the UI never polls.
 */
export const useConflictsStore = defineStore('conflicts', () => {
  const clashes = ref<Clash[]>([]);
  /** "Family Style" of each face a clash names. */
  const names = ref<Record<FaceId, string>>({});
  const error = ref<ApiError | null>(null);
  const loaded = ref(false);
  let unsubscribe: Promise<Unsubscribe> | undefined;
  /** Bumped by every read; an answer to an older read is dropped. */
  let round = 0;

  async function refresh(): Promise<void> {
    const mine = ++round;
    const client = useClient();
    try {
      const list = await client.listConflicts();
      const faces = new Set<FaceId>();
      for (const c of list) {
        faces.add(c.face);
        if (c.with.kind === 'active') faces.add(c.with.face);
      }
      const found: Record<FaceId, string> = {};
      await Promise.all(
        [...faces].map(async (id) => {
          const d = await client.getFace(id);
          found[id] = `${d.face.familyName} ${d.face.style}`;
        }),
      );
      if (mine !== round) return;
      clashes.value = list;
      names.value = found;
      error.value = null;
    } catch (e) {
      if (mine === round) error.value = e as ApiError;
    } finally {
      if (mine === round) loaded.value = true;
    }
  }

  function onEvent(event: Event): void {
    if (event.type === 'activationChanged') void refresh();
  }

  /** Subscribes to activation changes (once) and reads the list. */
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

  /** The display name of `face`, or its id while the name is unknown. */
  function nameOf(face: FaceId): string {
    return names.value[face] ?? `Face ${face}`;
  }

  return { clashes, error, loaded, load, stop, refresh, nameOf };
});
