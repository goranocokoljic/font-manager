import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type {
  ApiError,
  DuplicateGroup,
  Event,
  Problem,
  ProblemKind,
  ConflictSide,
  Resolution,
  ResolutionId,
  SidecarConflict,
  SidecarConflictId,
  SidecarField,
  TrashMoveStatus,
} from '../api/types';
import type { IconName } from '../components/ui/icons';
import { t } from '../i18n';
import { useToastStore, type ToastAction } from './toast';

/** "1 copy", "2 copies". */
export function copies(n: number): string {
  return t('problems.copies', { count: n });
}

/** One line about a "Keep this one" action: what moved, what stayed. */
export function resolutionSummary(r: Resolution): string {
  const count = (s: TrashMoveStatus) => r.moves.filter((m) => m.status === s).length;
  const moved = count('trashed') + count('restored');
  const parts = [
    t(r.undone ? 'problems.summary.putBack' : 'problems.summary.moved', { count: moved }),
  ];
  const stayed = count('skipped') + count('failed');
  if (stayed) parts.push(t('problems.summary.stayed', { count: stayed }));
  return parts.join(' ');
}

/**
 * A filter of the Problems view: a group of `ProblemKind`s under one label. Duplicates are
 * no group: they are the Duplicates module's (E56.15), and the view shows one line for them.
 */
export type ProblemGroupId = 'broken' | 'legacy' | 'sidecar' | 'activation' | 'library';

export interface ProblemGroup {
  id: ProblemGroupId;
  label: string;
  /** One line that says what the group is (board 16, E57.9). */
  line: string;
  /** The icon of its rows' tile. */
  icon: IconName;
  kinds: ProblemKind[];
}

/** A group whose `label` and `line` follow the UI's language (getters over the catalog). */
function group(id: ProblemGroupId, icon: IconName, kinds: ProblemKind[]): ProblemGroup {
  return {
    id,
    get label() {
      return t(`problems.groups.${id}`);
    },
    get line() {
      return t(`problems.groupLines.${id}`);
    },
    icon,
    kinds,
  };
}

/** The kinds the Problems view lists: every kind but the Duplicates module's. */
type ListedKind = Exclude<ProblemKind, 'duplicateFile' | 'duplicateFace'>;

/**
 * The one action a problem row offers (E57.9): copy the file's path, open Settings › Sources
 * for a folder that is not found, or open the Conflicts view for an activation conflict.
 */
export type ProblemAction = 'copyPath' | 'openSources' | 'decide';

const ACTIONS: Record<ListedKind, ProblemAction> = {
  broken: 'copyPath',
  noUnicodeCmap: 'copyPath',
  unsupported: 'copyPath',
  sidecarConflict: 'copyPath',
  indexRebuilt: 'copyPath',
  missingSource: 'openSources',
  systemConflict: 'decide',
  activeConflict: 'decide',
};

/** A row of the Problems view: one entry `list_problems` reported, ready to show. */
export interface ProblemRow {
  type: 'problem';
  key: string;
  /** The file or folder name. */
  name: string;
  /** What kind of problem, a few words beside the name. */
  kindLabel: string;
  /** The sentence fm-api gives. */
  message: string;
  path: string;
  action: ProblemAction;
}

/** A row of the Problems view for a sync conflict: both values, each with its button. */
export interface ConflictRow {
  type: 'conflict';
  key: string;
  /** The family. */
  name: string;
  /** The field, in words. */
  kindLabel: string;
  /** The sync file the other value came from. */
  path: string;
  conflict: SidecarConflict;
}

export type ProblemListRow = ProblemRow | ConflictRow;

/** A group as the Problems view lists it: its count and its rows. */
export interface ProblemSection {
  group: ProblemGroup;
  count: number;
  rows: ProblemListRow[];
}

/** The last part of a path, with either separator; the path itself when it has none. */
export function fileName(path: string): string {
  return (
    path
      .split(/[\\/]/)
      .filter((s) => s)
      .pop() ?? path
  );
}

/** The name of a sidecar conflict's field, in the UI's language. */
export function fieldName(field: SidecarField): string {
  return t(`problems.sidecar.fields.${field}`);
}

/** The row of one entry of `list_problems` (a listed kind). */
export function problemRow(p: Problem, index: number): ProblemRow {
  const kind = p.kind as ListedKind;
  return {
    type: 'problem',
    key: `${p.kind}\u0000${p.path}\u0000${p.other ?? ''}\u0000${index}`,
    name: fileName(p.path),
    kindLabel: t(`problems.kinds.${kind}`),
    message: p.message,
    path: p.path,
    action: ACTIONS[kind],
  };
}

/** The row of a sync conflict. */
export function conflictRow(c: SidecarConflict): ConflictRow {
  return {
    type: 'conflict',
    key: `conflict-${c.id}`,
    name: c.family,
    kindLabel: fieldName(c.field),
    path: c.sidecarPath,
    conflict: c,
  };
}

/**
 * The filters of the Problems view, in display order. A label table over the kinds
 * fm-api reports, like the facet labels in `search/chips.ts`; every kind but the duplicate
 * ones (`duplicateFile`, `duplicateFace`: the Duplicates module's, E56.15) is in one group.
 */
export const PROBLEM_GROUPS: ProblemGroup[] = [
  group('broken', 'alert', ['broken', 'noUnicodeCmap']),
  group('legacy', 'clock', ['unsupported']),
  group('sidecar', 'rescan', ['sidecarConflict']),
  group('activation', 'power', ['systemConflict', 'activeConflict']),
  group('library', 'folder', ['missingSource', 'indexRebuilt']),
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
 *
 * Sidecar conflicts (E28.1): the values another computer set differently, each resolved
 * by keeping this computer's value or the other one; a resolved conflict leaves the list.
 */
export const useProblemsStore = defineStore('problems', () => {
  const problems = ref<Problem[]>([]);
  const error = ref<ApiError | null>(null);
  const loaded = ref(false);
  const duplicates = ref<DuplicateGroup[]>([]);
  const history = ref<Resolution[]>([]);
  const sidecarConflicts = ref<SidecarConflict[]>([]);
  /** The dry run of a "Keep this one", waiting for the user to confirm. */
  const plan = ref<Resolution | null>(null);
  /** The error of the last keep or undo. */
  const actionError = ref<ApiError | null>(null);
  /** The Undo of the toast this store showed last, and the action it undoes. */
  let toastUndo: { action: ToastAction; id: ResolutionId } | undefined;
  let unsubscribe: Promise<Unsubscribe> | undefined;
  /** Bumped by every read; an answer to an older read is dropped. */
  let round = 0;

  /** The entries of every group: the duplicate kinds are not counted (E56.15). */
  const listed = computed(() => problems.value.filter((p) => groupOf(p.kind)));
  const total = computed(() => listed.value.length);
  /** Entries per group, every group present (0 when it has none). */
  const counts = computed(() => {
    const out = Object.fromEntries(PROBLEM_GROUPS.map((g) => [g.id, 0])) as Record<
      ProblemGroupId,
      number
    >;
    for (const p of listed.value) {
      const group = groupOf(p.kind);
      if (group) out[group.id] += 1;
    }
    return out;
  });

  /** The entries of `group`, or all of them without one. */
  function inGroup(group: ProblemGroupId | undefined): Problem[] {
    const kinds = group && problemGroup(group)?.kinds;
    return kinds ? listed.value.filter((p) => kinds.includes(p.kind)) : listed.value;
  }

  /**
   * The sync entries of `list_problems` that no `list_sidecar_conflicts` entry shows: each
   * conflict hides the one entry about the same face and sync file. The rest (a sync file
   * that cannot be read, a collection or pinned virtual style that cannot be joined here)
   * are plain rows, so every counted entry is listed.
   */
  function unlisted(entries: Problem[]): Problem[] {
    const shown = sidecarConflicts.value.map((c) => `${c.face}\u0000${c.sidecarPath}`);
    return entries.filter((p) => {
      const i = p.face === null ? -1 : shown.indexOf(`${p.face}\u0000${p.other ?? ''}`);
      if (i >= 0) shown.splice(i, 1);
      return i < 0;
    });
  }

  /**
   * The groups the Problems view lists (E57.9): every group with a problem, or the one
   * group `filter` names (even with none). A sync conflict fm-core can resolve is one
   * `list_sidecar_conflicts` entry, shown with both values; its `list_problems` entry (the
   * one about the same face and sync file) is the same conflict and is not listed again;
   * every other sync entry is a plain row (`unlisted`).
   */
  function sections(filter: ProblemGroupId | undefined): ProblemSection[] {
    const shown = filter ? PROBLEM_GROUPS.filter((g) => g.id === filter) : PROBLEM_GROUPS;
    return shown
      .map((g): ProblemSection => {
        const entries = inGroup(g.id);
        const rows: ProblemListRow[] =
          g.id === 'sidecar'
            ? [...sidecarConflicts.value.map(conflictRow), ...unlisted(entries).map(problemRow)]
            : entries.map(problemRow);
        return { group: g, count: counts.value[g.id], rows };
      })
      .filter((s) => filter || s.rows.length > 0);
  }

  async function refresh(): Promise<void> {
    const mine = ++round;
    try {
      const client = useClient();
      const [list, groups, actions, conflicts] = await Promise.all([
        client.listProblems(),
        client.listDuplicates(),
        client.listResolutions(),
        client.listSidecarConflicts(),
      ]);
      if (mine !== round) return;
      problems.value = list;
      duplicates.value = groups;
      history.value = actions;
      sidecarConflicts.value = conflicts;
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
    const action: ToastAction = {
      get label() {
        return t('common.undo');
      },
      run: () => undo(id),
    };
    toastUndo = { action, id };
    toasts.show(resolutionSummary(resolution), action);
  }

  /** Takes down the toast that offers the undo of `id`, if it is still up. */
  function dismissToastOf(id: ResolutionId): void {
    const toasts = useToastStore();
    const offered = toastUndo;
    if (offered?.id === id) {
      const up = toasts.toasts.find((shown) => shown.action === offered.action);
      if (up) toasts.dismiss(up.id);
    }
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
          message: failed?.message ?? t('problems.undoFailed'),
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

  /** Keeps one value of a sidecar conflict, then reads the lists again. */
  async function resolveConflict(id: SidecarConflictId, keep: ConflictSide): Promise<void> {
    actionError.value = null;
    try {
      await useClient().resolveSidecarConflict({ id, keep });
    } catch (e) {
      actionError.value = e as ApiError;
    }
    await refresh();
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
    sections,
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
    sidecarConflicts,
    resolveConflict,
  };
});
