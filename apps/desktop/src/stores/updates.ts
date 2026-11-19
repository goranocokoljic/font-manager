import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type {
  ApiError,
  AvailableUpdate,
  Event,
  UpdateNotice,
  UpdateStatus as ShellUpdateStatus,
  UpdateSupport,
} from '../api/types';
import { t } from '../i18n';
import { useToastStore } from './toast';

/** Where "Check for updates" is: what the Settings section and the dialog show. */
export type UpdateStatus = 'idle' | 'checking' | 'upToDate' | 'available' | 'installing';

/**
 * In-app updates (E18.2). The app asks the release server only when the user clicks
 * "Check for updates" or has turned on "Check automatically" (rule 7). The automatic
 * check (at start and once a day) runs in the shell, not here (E55.7): it announces a
 * found update with the `updateAvailable` event, and this store only records it; the
 * dialog opens on a click (the tray's "Update to <version>…", Settings → About). A quiet
 * find shows a notice with "See what's new" and a dot on Settings (E55.8); the notice can
 * be dismissed until the next start, the dot stays while the update is on offer. fm-api
 * decides how big a step an update is (`kind`) and refuses a major install without the
 * second confirmation; this store only keeps what the user was shown and what they
 * confirmed.
 *
 * "Install updates when I quit" (E55.9) also runs in the shell: it downloads a patch or minor
 * update in the background and installs it as the app quits. The start after it says once
 * what happened (`takeUpdateNotice`): "Updated to <version>" with a link to what is new, or
 * that the update could not be installed, with a button that checks again and opens the
 * dialog. A package the shell refused while the app runs arrives as `updateNotice`.
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
  /**
   * What Settings → About shows under the update control (E55.10): the level, the levels
   * offered, the last check and the version waiting for the quit. Read from the shell;
   * `updateStatusChanged` says when to read it again.
   */
  const overview = ref<ShellUpdateStatus | null>(null);
  /** The update on offer came from the shell's automatic check, not from a click. */
  const foundQuietly = ref(false);
  /** The user dismissed the notice; it stays away until the next start (the store is new). */
  const noticeDismissed = ref(false);
  /** The quiet notice shows: an automatic find, not dismissed, the dialog not open. */
  const notice = computed(
    () =>
      update.value !== null &&
      foundQuietly.value &&
      !noticeDismissed.value &&
      !open.value &&
      status.value === 'available',
  );
  /** The dot on Settings: an update is on offer until it is installed. */
  const dot = computed(() => update.value !== null);
  // The dialog, opened from anywhere, shows the update: it is no longer a quiet find.
  watch(open, (isOpen) => {
    if (isOpen) foundQuietly.value = false;
  });
  let unsubscribe: Promise<Unsubscribe> | undefined;

  async function loadSupport(): Promise<UpdateSupport | null> {
    try {
      support.value = await useClient().updateSupport();
    } catch (e) {
      error.value = e as ApiError;
    }
    return support.value;
  }

  /** Reads what the update control shows; a failure keeps the last answer. */
  async function loadOverview(): Promise<ShellUpdateStatus | null> {
    try {
      overview.value = await useClient().updateStatus();
    } catch {
      // The control keeps what it showed.
    }
    return overview.value;
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
      foundQuietly.value = false;
      if (update.value) {
        confirmingMajor.value = false;
        error.value = null;
        open.value = true;
      }
    } catch (e) {
      status.value = 'idle';
      error.value = e as ApiError;
    }
    void loadOverview();
  }

  /** Records an update the shell found by itself; nothing opens. */
  function offer(found: AvailableUpdate): void {
    if (status.value === 'installing' || status.value === 'checking') return;
    // A version the user already saw in the dialog stays shown that way: no notice for it.
    if (update.value?.version !== found.version) {
      confirmingMajor.value = false;
      foundQuietly.value = true;
    }
    update.value = found;
    status.value = 'available';
  }

  /** Says `notice` once, as a toast with its one action. */
  function say(notice: UpdateNotice): void {
    const toasts = useToastStore();
    if (notice.type === 'updated') {
      const url = notice.notesUrl;
      toasts.show(
        t('updates.notice.updated', { version: notice.version }),
        url
          ? { label: t('updates.notice.whatsNew'), run: () => useClient().openUrl(url) }
          : undefined,
      );
    } else {
      toasts.show(notice.message, { label: t('updates.notice.retry'), run: () => check() });
    }
  }

  function onEvent(event: Event): void {
    if (event.type === 'updateAvailable') offer(event.update);
    if (event.type === 'updateStatusChanged' || event.type === 'updateAvailable')
      void loadOverview();
    if (event.type === 'updateNotice') {
      say(event.notice);
      // Said now: the shell keeps it for the next start only until it is read.
      void useClient()
        .takeUpdateNotice()
        .catch(() => null);
    }
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
    try {
      const notice = await useClient().takeUpdateNotice();
      if (notice) say(notice);
    } catch {
      // Nothing to say about the last quit.
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

  /** The notice's close button: the notice goes until the next start; the dot stays. */
  function dismissNotice(): void {
    noticeDismissed.value = true;
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
    overview,
    notice,
    dot,
    loadSupport,
    loadOverview,
    check,
    start,
    stop,
    onEvent,
    openDialog,
    dismissNotice,
    install,
    dismiss,
  };
});
