import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type { ApiError, AvailableUpdate, Event, UpdateSupport } from '../api/types';

/** Where "Check for updates" is: what the Settings section and the dialog show. */
export type UpdateStatus = 'idle' | 'checking' | 'upToDate' | 'available' | 'installing';

/**
 * In-app updates (E18.2). The app asks the release server only when the user clicks
 * "Check for updates" or has turned on "Check automatically" (rule 7). The automatic
 * check (at start and once a day) runs in the shell, not here (E55.7): it announces a
 * found update with the `updateAvailable` event, and this store only records it; the
 * dialog opens on a click (the tray's "Update to <version>…", Settings → About). fm-api
 * decides how big a step an update is (`kind`) and refuses a major install without the
 * second confirmation; this store only keeps what the user was shown and what they
 * confirmed.
 */
export const useUpdatesStore = defineStore('updates', () => {
  const support = ref<UpdateSupport | null>(null);
  const status = ref<UpdateStatus>('idle');
  const update = ref<AvailableUpdate | null>(null);
  /** The update dialog is open. */
  const open = ref(false);
  /** The dialog asks the second time, for a major update. */
  const confirmingMajor = ref(false);
  const error = ref<ApiError | null>(null);
  let unsubscribe: Promise<Unsubscribe> | undefined;

  async function loadSupport(): Promise<UpdateSupport | null> {
    try {
      support.value = await useClient().updateSupport();
    } catch (e) {
      error.value = e as ApiError;
    }
    return support.value;
  }

  /** "Check for updates": asks the release server. A found update opens the dialog. */
  async function check(): Promise<void> {
    if (status.value === 'checking' || status.value === 'installing') return;
    status.value = 'checking';
    error.value = null;
    try {
      const found = await useClient().checkForUpdate();
      update.value = found.update ?? null;
      status.value = update.value ? 'available' : 'upToDate';
      if (update.value) {
        confirmingMajor.value = false;
        error.value = null;
        open.value = true;
      }
    } catch (e) {
      status.value = 'idle';
      error.value = e as ApiError;
    }
  }

  /** Records an update the shell found by itself; nothing opens. */
  function offer(found: AvailableUpdate): void {
    if (status.value === 'installing' || status.value === 'checking') return;
    if (update.value?.version !== found.version) confirmingMajor.value = false;
    update.value = found;
    status.value = 'available';
  }

  function onEvent(event: Event): void {
    if (event.type === 'updateAvailable') offer(event.update);
  }

  /**
   * At app start: listens for the shell's automatic finds (once) and reads the one it may
   * have announced before the window listened. Asks the release server nothing.
   */
  async function start(): Promise<void> {
    unsubscribe ??= useClient().onEvent(onEvent);
    await unsubscribe.catch(() => undefined);
    try {
      const found = await useClient().pendingUpdate();
      if (found) offer(found);
    } catch {
      // Nothing on offer that the UI can show; "Check for updates" still works.
    }
  }

  /** Stops listening. */
  async function stop(): Promise<void> {
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  /** The tray's "Update to <version>…": opens the dialog for the update on offer. */
  async function openDialog(): Promise<void> {
    if (!update.value) {
      const found = await useClient()
        .pendingUpdate()
        .catch(() => null);
      if (found) offer(found);
    }
    if (!update.value || status.value === 'installing') return;
    error.value = null;
    open.value = true;
  }

  /**
   * "Install": a major update first asks a second time; the second yes sends
   * `confirmMajor`. The app then quits as Quit does, installs and restarts.
   */
  async function install(): Promise<void> {
    const u = update.value;
    if (!u || status.value === 'installing') return;
    if (u.kind === 'major' && !confirmingMajor.value) {
      confirmingMajor.value = true;
      return;
    }
    status.value = 'installing';
    error.value = null;
    try {
      await useClient().installUpdate({ version: u.version, confirmMajor: confirmingMajor.value });
    } catch (e) {
      error.value = e as ApiError;
      status.value = 'available';
    }
  }

  /** "Later" or "Cancel": closes the dialog; the update stays on offer. */
  function dismiss(): void {
    if (status.value === 'installing') return;
    open.value = false;
    confirmingMajor.value = false;
  }

  return {
    support,
    status,
    update,
    open,
    confirmingMajor,
    error,
    loadSupport,
    check,
    start,
    stop,
    onEvent,
    openDialog,
    install,
    dismiss,
  };
});
