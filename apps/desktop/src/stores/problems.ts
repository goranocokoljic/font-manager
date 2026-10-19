import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type { ApiError, Event, Problem, ProblemKind } from '../api/types';

/** A filter of the Problems view: a group of `ProblemKind`s under one label. */
export type ProblemGroupId =
  'duplicates' | 'broken' | 'legacy' | 'sidecar' | 'activation' | 'library';

export interface ProblemGroup {
  id: ProblemGroupId;
  label: string;
  kinds: ProblemKind[];
}

/**
 * The filters of the Problems view, in display order. A label table over the kinds
 * fm-api reports, like the facet labels in `search/chips.ts`; every kind is in one group.
 */
export const PROBLEM_GROUPS: ProblemGroup[] = [
  { id: 'duplicates', label: 'Duplicates', kinds: ['duplicateFile', 'duplicateFace'] },
  { id: 'broken', label: 'Broken files', kinds: ['broken', 'noUnicodeCmap'] },
  { id: 'legacy', label: 'Legacy formats', kinds: ['unsupported'] },
  { id: 'sidecar', label: 'Sidecar conflicts', kinds: ['sidecarConflict'] },
  { id: 'activation', label: 'Activation conflicts', kinds: ['systemConflict', 'activeConflict'] },
  { id: 'library', label: 'Folders and index', kinds: ['missingSource', 'indexRebuilt'] },
];

/** The group `id` names, or `undefined` for anything else (a stale URL). */
export function problemGroup(id: unknown): ProblemGroup | undefined {
  return PROBLEM_GROUPS.find((g) => g.id === id);
}

/** The group a kind is listed under. */
export function groupOf(kind: ProblemKind): ProblemGroup | undefined {
  return PROBLEM_GROUPS.find((g) => g.kinds.includes(kind));
}

/**
 * The Problems view and its counts (sidebar, status bar): `list_problems` as fm-api
 * reports it. Read again on every library, scan and activation event; the UI never polls.
 */
export const useProblemsStore = defineStore('problems', () => {
  const problems = ref<Problem[]>([]);
  const error = ref<ApiError | null>(null);
  const loaded = ref(false);
  let unsubscribe: Promise<Unsubscribe> | undefined;
  /** Bumped by every read; an answer to an older read is dropped. */
  let round = 0;

  const total = computed(() => problems.value.length);
  /** Entries per group, every group present (0 when it has none). */
  const counts = computed(() => {
    const out = Object.fromEntries(PROBLEM_GROUPS.map((g) => [g.id, 0])) as Record<
      ProblemGroupId,
      number
    >;
    for (const p of problems.value) {
      const group = groupOf(p.kind);
      if (group) out[group.id] += 1;
    }
    return out;
  });

  /** The entries of `group`, or all of them without one. */
  function inGroup(group: ProblemGroupId | undefined): Problem[] {
    const kinds = group && problemGroup(group)?.kinds;
    return kinds ? problems.value.filter((p) => kinds.includes(p.kind)) : problems.value;
  }

  async function refresh(): Promise<void> {
    const mine = ++round;
    try {
      const list = await useClient().listProblems();
      if (mine !== round) return;
      problems.value = list;
      error.value = null;
    } catch (e) {
      if (mine === round) error.value = e as ApiError;
    } finally {
      if (mine === round) loaded.value = true;
    }
  }

  function onEvent(event: Event): void {
    if (
      event.type === 'libraryChanged' ||
      event.type === 'scanFinished' ||
      event.type === 'activationChanged'
    )
      void refresh();
  }

  /** Subscribes to the events that change the list (once) and reads it. */
  async function load(): Promise<void> {
    unsubscribe ??= useClient().onEvent(onEvent);
    await unsubscribe.catch(() => undefined);
    await refresh();
  }

  /** Stops listening to events. */
  async function stop(): Promise<void> {
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  return { problems, error, loaded, total, counts, inGroup, load, stop, refresh };
});
