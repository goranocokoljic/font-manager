import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type {
  ApiError,
  DuplicateGroup,
  Event,
  Problem,
  ProblemKind,
  Resolution,
  ResolutionId,
  TrashMoveStatus,
} from '../api/types';
import { useToastStore, type ToastAction } from './toast';

/** "1 copy", "2 copies". */
export function copies(n: number): string {
  return n === 1 ? '1 copy' : `${n} copies`;
}

/** One line about a "Keep this one" action: what moved, what stayed. */
export function resolutionSummary(r: Resolution): string {
  const count = (s: TrashMoveStatus) => r.moves.filter((m) => m.status === s).length;
  const moved = count('trashed') + count('restored');
  const parts = [r.undone ? `Put back ${copies(moved)}.` : `Moved ${copies(moved)} to the trash.`];
  const stayed = count('skipped') + count('failed');
  if (stayed)
    parts.push(`${copies(stayed)} stayed where ${stayed === 1 ? 'it was' : 'they were'}.`);
  return parts.join(' ');
}

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
 *
 * "Keep this one" (E8.3): the duplicate groups, the dry-run `plan` the user confirms,
 * the undo in the app toast (`useToastStore`, gone after `TOAST_MS` unless the pointer
 * or focus is on it, and kept across routes), and the `history` of actions, each
 * undoable from there for as long as it is listed.
 */
export const useProblemsStore = defineStore('problems', () => {
  const problems = ref<Problem[]>([]);
  const error = ref<ApiError | null>(null);
  const loaded = ref(false);
  const duplicates = ref<DuplicateGroup[]>([]);
  const history = ref<Resolution[]>([]);
  /** The dry run of a "Keep this one", waiting for the user to confirm. */
  const plan = ref<Resolution | null>(null);
  /** The error of the last keep or undo. */
  const actionError = ref<ApiError | null>(null);
  /** The Undo of the toast this store showed last, and the action it undoes. */
  let toastUndo: { action: ToastAction; id: ResolutionId } | undefined;
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
      const client = useClient();
      const [list, groups, actions] = await Promise.all([
        client.listProblems(),
        client.listDuplicates(),
        client.listResolutions(),
      ]);
      if (mine !== round) return;
      problems.value = list;
      duplicates.value = groups;
      history.value = actions;
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

  /** Offers the undo of `resolution` in the app toast. */
  function showToast(resolution: Resolution): void {
    const id = resolution.id;
    const toasts = useToastStore();
    if (id === null) {
      toasts.show(resolutionSummary(resolution));
      return;
    }
    const action: ToastAction = { label: 'Undo', run: () => undo(id) };
    toastUndo = { action, id };
    toasts.show(resolutionSummary(resolution), action);
  }

  /** Takes down the toast that offers the undo of `id`, if it is still up. */
  function dismissToastOf(id: ResolutionId): void {
    const toasts = useToastStore();
    if (toastUndo?.id === id && toasts.toast?.action === toastUndo.action) toasts.dismiss();
    if (toastUndo?.id === id) toastUndo = undefined;
  }

  /** Asks fm-api what keeping `keep` would move (a dry run) and holds it as the `plan`. */
  async function planKeep(group: DuplicateGroup, keep: string): Promise<void> {
    actionError.value = null;
    try {
      plan.value = await useClient().keepCopy({
        keep,
        trash: group.copies.filter((c) => c.path !== keep).map((c) => c.path),
        dryRun: true,
      });
    } catch (e) {
      actionError.value = e as ApiError;
    }
  }

  function cancelPlan(): void {
    plan.value = null;
  }

  /** Moves what the `plan` listed to the trash and offers the undo. */
  async function confirmPlan(): Promise<void> {
    const p = plan.value;
    if (!p) return;
    plan.value = null;
    const trash = p.moves.filter((m) => m.status === 'planned').map((m) => m.path);
    if (!trash.length) return;
    try {
      showToast(await useClient().keepCopy({ keep: p.kept, trash, dryRun: false }));
    } catch (e) {
      actionError.value = e as ApiError;
    }
    await refresh();
  }

  /** Puts the copies of an action back (from the toast or the history). */
  async function undo(id: ResolutionId): Promise<Resolution | undefined> {
    actionError.value = null;
    dismissToastOf(id);
    try {
      const r = await useClient().undoResolution(id);
      if (!r.undone) {
        const failed = r.moves.find((m) => m.status === 'failed');
        actionError.value = {
          code: 'io',
          message: failed?.message ?? 'Some copies could not be put back.',
          detail: null,
        };
      }
      return r;
    } catch (e) {
      actionError.value = e as ApiError;
      return undefined;
    } finally {
      await refresh();
    }
  }

  /** Subscribes to the events that change the list (once) and reads it. */
  async function load(): Promise<void> {
    unsubscribe ??= useClient().onEvent(onEvent);
    await unsubscribe.catch(() => undefined);
    await refresh();
  }

  /** Stops listening to events. The undo toast stays: it belongs to the app, not the view. */
  async function stop(): Promise<void> {
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  return {
    problems,
    error,
    loaded,
    total,
    counts,
    inGroup,
    load,
    stop,
    refresh,
    duplicates,
    history,
    plan,
    actionError,
    planKeep,
    cancelPlan,
    confirmPlan,
    undo,
  };
});
