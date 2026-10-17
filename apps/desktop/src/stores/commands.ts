import { defineStore } from 'pinia';
import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  toValue,
  type MaybeRefOrGetter,
} from 'vue';
import { fuzzyMatch } from '../keyboard/fuzzy';
import { ACTIONS, bindingFor, chordOf, type Chord } from '../keyboard/keymap';

/** Something the user can do, listed in the command palette. */
export interface Action {
  id: string;
  title: string;
  /** Palette group, e.g. "View", "Activation". */
  group: string;
  /** False when the action cannot run in the current state; it is listed but dimmed. */
  enabled?: MaybeRefOrGetter<boolean>;
  run: () => unknown;
}

/** An action as the palette shows it. */
export interface PaletteItem {
  action: Action;
  chord: Chord | undefined;
  enabled: boolean;
  /** Matched character positions in the title. */
  indices: number[];
}

const isEnabled = (a: Action) => (a.enabled === undefined ? true : toValue(a.enabled));

/**
 * Every action the app offers right now, the palette's open state and query, and the
 * global key handler that runs an action from its binding in the keymap.
 */
export const useCommandsStore = defineStore('commands', () => {
  const actions = shallowRef<Action[]>([]);
  const open = ref(false);
  const query = ref('');

  /** Adds an action (replacing one with the same id); returns the undo. */
  function register(action: Action): () => void {
    actions.value = [...actions.value.filter((a) => a.id !== action.id), action];
    return () => {
      actions.value = actions.value.filter((a) => a !== action);
    };
  }

  /** Runs an action if it is registered and enabled; true when it ran. */
  function run(id: string): boolean {
    const action = actions.value.find((a) => a.id === id);
    if (!action || !isEnabled(action)) return false;
    void action.run();
    return true;
  }

  /** Every action, best fuzzy match first; all of them in registration order for no query. */
  const items = computed<PaletteItem[]>(() => {
    const scored: (PaletteItem & { score: number; order: number })[] = [];
    actions.value.forEach((action, order) => {
      const inTitle = fuzzyMatch(query.value, action.title);
      // The group name also matches ("activation" finds both activation toggles); such a
      // match highlights nothing in the title.
      const m = inTitle ?? fuzzyMatch(query.value, `${action.group} ${action.title}`);
      if (!m) return;
      scored.push({
        action,
        chord: chordOf(action.id),
        enabled: isEnabled(action),
        indices: inTitle?.indices ?? [],
        score: m.score,
        order,
      });
    });
    return scored
      .sort((a, b) => b.score - a.score || a.order - b.order)
      .map(({ action, chord, enabled, indices }) => ({ action, chord, enabled, indices }));
  });

  function openPalette(): void {
    query.value = '';
    open.value = true;
  }

  function closePalette(): void {
    open.value = false;
  }

  function togglePalette(): void {
    if (open.value) closePalette();
    else openPalette();
  }

  /** Runs the action bound to the key, if any; returns true when the key was used. */
  function handleKey(e: KeyboardEvent): boolean {
    if (e.defaultPrevented || e.isComposing) return false;
    const binding = bindingFor(e);
    if (!binding) return false;
    // While the palette is open only its own toggle works; it owns every other key.
    if (open.value && binding.action !== ACTIONS.palette) return false;
    if (e.repeat && !binding.chord.mod) return false;
    if (!run(binding.action)) return false;
    e.preventDefault();
    return true;
  }

  return {
    actions,
    open,
    query,
    items,
    register,
    run,
    openPalette,
    closePalette,
    togglePalette,
    handleKey,
  };
});

/** Registers actions for the lifetime of the calling component. */
export function useActions(list: Action[]): void {
  const commands = useCommandsStore();
  const undo: (() => void)[] = [];
  onMounted(() => undo.push(...list.map((a) => commands.register(a))));
  onBeforeUnmount(() => undo.splice(0).forEach((u) => u()));
}

/** Listens for shortcuts on the window for the lifetime of the calling component. */
export function useShortcuts(): void {
  const commands = useCommandsStore();
  const onKeyDown = (e: KeyboardEvent) => commands.handleKey(e);
  onMounted(() => window.addEventListener('keydown', onKeyDown));
  onBeforeUnmount(() => window.removeEventListener('keydown', onKeyDown));
}
