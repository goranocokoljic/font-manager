import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type {
  ActivationMode,
  ActivationOutcome,
  ApiError,
  Event,
  FaceSummary,
  FamilyId,
  SetId,
  Target,
} from '../api/types';
import { bulkMessage, planToggle, undoSteps, type UndoStep } from '../list/activation';
import type { ListRow } from '../list/rows';
import { useSelectionStore } from './selection';
import { useSettingsStore } from './settings';
import { useToastStore } from './toast';

/**
 * How many faces the app has active, for the status bar. Read from fm-api at
 * start and again on every `activationChanged` event; the UI never polls.
 * Also runs the bulk actions (the selected rows, "Deactivate all"); each ends
 * with a toast that can undo it.
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

  /** A bulk request or its undo is in flight. */
  const busy = ref(false);

  async function exclusive(work: () => Promise<void>): Promise<void> {
    if (busy.value) return;
    busy.value = true;
    try {
      await work();
    } catch (e) {
      useToastStore().show((e as ApiError).message ?? 'The activation failed.');
    } finally {
      busy.value = false;
    }
    // The inspector's face may be one of those that changed.
    await useSelectionStore().reload();
  }

  /**
   * Activates or deactivates `targets` (`undefined`: everything the app
   * activated; a deactivation leaves the faces of `except` active), then shows
   * a toast with the outcome and, when something changed, an Undo button that
   * puts every face back.
   */
  function bulk(
    deactivate: boolean,
    targets: (Target | undefined)[],
    mode?: ActivationMode,
    except?: Target,
  ) {
    return exclusive(async () => {
      const client = useClient();
      const settings = useSettingsStore();
      const how = mode ?? settings.activationMode;
      const before = await client.listActive();
      const outcomes: ActivationOutcome[] = [];
      const request = { mode: how, onConflict: settings.onConflict };
      for (const target of targets) {
        if (deactivate) outcomes.push(...(await client.deactivate({ target, except })));
        else if (target) outcomes.push(...(await client.activate({ target, ...request })));
      }
      const steps = undoSteps(before, outcomes);
      useToastStore().show(
        bulkMessage(deactivate, how, outcomes),
        steps.length ? { label: 'Undo', run: () => undo(steps) } : undefined,
      );
    });
  }

  /** Runs the undo of a bulk change. */
  function undo(steps: readonly UndoStep[]): Promise<void> {
    return exclusive(async () => {
      const client = useClient();
      for (const step of steps) {
        const target: Target = { kind: 'faces', faces: step.faces };
        // The faces were active before, so a clash then was already accepted.
        if (step.kind === 'activate')
          await client.activate({ target, mode: step.mode, onConflict: 'replace' });
        else await client.deactivate({ target });
      }
      useToastStore().show('Undone.');
    });
  }

  /** A / Shift+A on the selected rows (see `planToggle`). */
  async function toggleRows(rows: readonly ListRow[], mode?: ActivationMode): Promise<void> {
    if (busy.value) return;
    // Shift+A needs the modes of a family's faces; read them fresh, not from a list
    // that the last change may not have reloaded yet.
    const familyFaces = new Map<FamilyId, FaceSummary[]>();
    if (mode === 'permanent')
      try {
        for (const row of rows)
          if (row.kind === 'family')
            familyFaces.set(row.family.id, (await useClient().getFamily(row.family.id)).faces);
      } catch {
        // The family summaries decide then.
      }
    const plan = planToggle(rows, mode, familyFaces);
    if (plan) await bulk(plan.deactivate, plan.targets, mode);
  }

  /** Deactivates every face the app activated (never one it did not, rule 3). */
  function deactivateAll(): Promise<void> {
    return bulk(true, [undefined]);
  }

  /** Deactivates every face the app activated except those of project set `set`. */
  function deactivateAllExcept(set: SetId): Promise<void> {
    return bulk(true, [undefined], undefined, { kind: 'set', set });
  }

  return { count, busy, load, stop, refresh, toggleRows, deactivateAll, deactivateAllExcept };
});
