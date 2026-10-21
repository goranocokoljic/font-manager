import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient, type FullSettings } from '../api/client';
import type { ApiError, AvailableUpdate, UpdateSupport } from '../api/types';

/** Where "Check for updates" is: what the Settings section and the dialog show. */
export type UpdateStatus = 'idle' | 'checking' | 'upToDate' | 'available' | 'installing';

/**
 * In-app updates (E18.2). The app asks the release server only when the user clicks
 * "Check for updates" or has turned on "Check automatically" (rule 7). fm-api decides
 * how big a step an update is (`kind`) and refuses a major install without the second
 * confirmation; this store only keeps what the user was shown and what they confirmed.
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

  async function loadSupport(): Promise<UpdateSupport | null> {
    try {
      support.value = await useClient().updateSupport();
    } catch (e) {
      error.value = e as ApiError;
    }
    return support.value;
  }

  /**
   * Asks the release server. A found update opens the dialog. `quiet` (the check at
   * start) shows nothing when the app is up to date or the server cannot be reached.
   */
  async function check({ quiet = false } = {}): Promise<void> {
    if (status.value === 'checking' || status.value === 'installing') return;
    status.value = 'checking';
    if (!quiet) error.value = null;
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
      if (!quiet) error.value = e as ApiError;
    }
  }

  /** At app start: checks once, quietly, when the user turned "Check automatically" on. */
  async function checkAtStart(settings: FullSettings | null): Promise<void> {
    if (!settings?.checkUpdatesAutomatically) return;
    const s = support.value ?? (await loadSupport());
    if (s?.offered) await check({ quiet: true });
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
    checkAtStart,
    install,
    dismiss,
  };
});
