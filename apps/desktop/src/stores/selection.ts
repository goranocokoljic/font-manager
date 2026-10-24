import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient } from '../api/client';
import type {
  ActivationMode,
  ActivationOutcome,
  ApiError,
  Conflict,
  FaceDetail,
  FaceId,
  LicenseDeclaration,
  OutcomeStatus,
  ReceiptPlace,
  Target,
} from '../api/types';
import { t } from '../i18n';
import { useSettingsStore } from './settings';
import { useTagsStore } from './tags';

/** A face id from a route query value (`?face=3`), or null when there is none. */
export function parseFaceId(value: unknown): FaceId | null {
  const raw = Array.isArray(value) ? value[0] : value;
  if (typeof raw !== 'string' || !/^\d+$/.test(raw)) return null;
  const id = Number(raw);
  return Number.isSafeInteger(id) && id <= 0xffff_ffff ? id : null;
}

/** The selected face and its full detail, as fm-api reports it. */
export const useSelectionStore = defineStore('selection', () => {
  const faceId = ref<FaceId | null>(null);
  const detail = ref<FaceDetail | null>(null);
  const error = ref<ApiError | null>(null);
  const loading = ref(false);
  /** An activation request is in flight. */
  const busy = ref(false);
  /** What the last activation or deactivation did to the selected face. */
  const outcome = ref<OutcomeStatus | null>(null);

  // Answers to an earlier selection are dropped when a newer one is pending.
  let request = 0;

  async function load(id: FaceId): Promise<void> {
    const mine = ++request;
    loading.value = true;
    try {
      const face = await useClient().getFace(id);
      if (mine !== request) return;
      detail.value = face;
      error.value = null;
    } catch (e) {
      if (mine !== request) return;
      detail.value = null;
      error.value = e as ApiError;
    } finally {
      if (mine === request) loading.value = false;
    }
  }

  /** Selects a face (or nothing) and fetches its detail. */
  async function select(id: FaceId | null): Promise<void> {
    if (id === faceId.value && (detail.value || loading.value)) return;
    faceId.value = id;
    outcome.value = null;
    pending.value = null;
    if (id === null) {
      request++;
      detail.value = null;
      error.value = null;
      loading.value = false;
      return;
    }
    await load(id);
  }

  /** Fetches the selected face's detail again, e.g. after a bulk activation changed it. */
  async function reload(): Promise<void> {
    if (faceId.value !== null) await load(faceId.value);
  }

  /** The selected face can be activated: it is loaded, healthy and no request is in flight. */
  const canActivate = computed(
    () => detail.value?.face.health.kind === 'ok' && !busy.value && !loading.value,
  );

  /**
   * An activation waiting for the user's answer: the selected face has the
   * PostScript name of a system or already-active font.
   */
  const pending = ref<{ face: FaceId; mode: ActivationMode; conflicts: Conflict[] } | null>(null);

  /** Runs one activation or deactivation of `face` and fetches its detail again. */
  async function run(face: FaceId, work: () => Promise<ActivationOutcome[]>): Promise<void> {
    busy.value = true;
    try {
      const results = await work();
      outcome.value = results.find((r) => r.face === face)?.outcome ?? null;
    } catch (e) {
      outcome.value = { status: 'failed', error: e as ApiError };
    } finally {
      busy.value = false;
    }
    if (faceId.value === face) await load(face);
  }

  /**
   * No mode (the Activate button): deactivates the selected face when active, or
   * activates it in the mode chosen in Settings. Session: the same, always for the
   * session. Permanent: activates it permanently (a session activation becomes
   * permanent), or deactivates it when it is already permanent. A name clash is
   * not activated yet: it waits in `pending` for {@link resolveConflict}, unless
   * Settings say to replace without asking.
   */
  async function toggleActivation(mode?: ActivationMode): Promise<void> {
    const face = detail.value?.face;
    if (!face || busy.value) return;
    const settings = useSettingsStore();
    const target: Target = { kind: 'faces', faces: [face.id] };
    const deactivate = mode === 'permanent' ? face.active === 'permanent' : face.active !== null;
    const client = useClient();
    pending.value = null;
    if (deactivate) return run(face.id, () => client.deactivate({ target }));
    const wanted = mode ?? settings.activationMode;
    // Settings may answer every clash with "replace" without asking.
    if (settings.onConflict === 'replace')
      return run(face.id, () => client.activate({ target, mode: wanted, onConflict: 'replace' }));
    busy.value = true;
    let conflicts: Conflict[];
    try {
      conflicts = await client.conflicts(target);
    } catch (e) {
      busy.value = false;
      outcome.value = { status: 'failed', error: e as ApiError };
      return;
    }
    busy.value = false;
    if (conflicts.length) {
      outcome.value = null;
      pending.value = { face: face.id, mode: wanted, conflicts };
      return;
    }
    await run(face.id, () => client.activate({ target, mode: wanted, onConflict: 'keepExisting' }));
  }

  /**
   * The user's answer to a pending clash. Keep: the other font stays and the
   * face is not activated. Replace: the face is activated for this session only.
   * Cancel: nothing happens.
   */
  async function resolveConflict(choice: 'keep' | 'replace' | 'cancel'): Promise<void> {
    const p = pending.value;
    if (!p || busy.value) return;
    pending.value = null;
    if (choice === 'cancel') return;
    await run(p.face, () =>
      useClient().activate({
        target: { kind: 'faces', faces: [p.face] },
        mode: p.mode,
        onConflict: choice === 'replace' ? 'replace' : 'keepExisting',
      }),
    );
  }

  /** The last tag, favorite, rating or notes edit the API refused; cleared by the next one. */
  const editError = ref<ApiError | null>(null);
  /** The last license declaration the API refused; cleared by the next one. */
  const licenseError = ref<ApiError | null>(null);

  /**
   * Runs one edit of the selected face's tags or user metadata, then fetches
   * its detail and the tag list again; true when the API took it.
   */
  async function edit(work: (face: FaceId) => Promise<unknown>): Promise<boolean> {
    const id = detail.value?.face.id;
    if (id === undefined) return false;
    let ok = true;
    try {
      await work(id);
      editError.value = null;
    } catch (e) {
      editError.value = e as ApiError;
      ok = false;
    }
    await useTagsStore().refresh();
    if (faceId.value === id) await load(id);
    return ok;
  }

  /** Puts a tag on the selected face; a new name makes a new tag. */
  function addTag(name: string): Promise<boolean> {
    return edit((face) => useClient().tagFaces({ faces: [face], tags: [name] }));
  }

  /** Takes a tag (by name, as the detail lists it) off the selected face. */
  function removeTag(name: string): Promise<boolean> {
    const tag = useTagsStore().idOf(name);
    if (tag === null) return Promise.resolve(false);
    return edit((face) => useClient().untagFaces({ faces: [face], tags: [tag] }));
  }

  /** Makes the selected face a favorite, or not. */
  function setFavorite(favorite: boolean): Promise<boolean> {
    return edit((face) => useClient().updateUserMeta({ faces: [face], favorite }));
  }

  /** Rates the selected face 1–5 stars; 0 clears the rating. */
  function setRating(rating: number): Promise<boolean> {
    return edit((face) => useClient().updateUserMeta({ faces: [face], rating }));
  }

  /** Sets the selected face's notes; an empty string clears them. */
  function setNotes(notes: string): Promise<boolean> {
    return edit((face) => useClient().updateUserMeta({ faces: [face], notes }));
  }

  /** Sets the notes of the selected face's family; an empty string clears them. */
  function setFamilyNotes(notes: string): Promise<boolean> {
    const family = detail.value?.face.family;
    if (family === undefined) return Promise.resolve(false);
    return edit(() => useClient().updateFamilyNotes({ family, notes }));
  }

  /**
   * Declares the license of the selected face's family (E15.2); the request
   * replaces what was declared, so a field left out is cleared.
   */
  async function declareLicense(req: Omit<LicenseDeclaration, 'family'>): Promise<boolean> {
    const family = detail.value?.face.family;
    if (family === undefined) return false;
    const ok = await edit(() => useClient().declareLicense({ ...req, family }));
    // The license form shows its own error, not the notes section.
    licenseError.value = ok ? null : editError.value;
    if (!ok) editError.value = null;
    return ok;
  }

  /**
   * Lets the user pick a receipt file and attaches a copy of it, kept in
   * `place`, with the rest of the declaration in `req`; false on a cancel.
   */
  async function attachReceipt(
    req: Omit<LicenseDeclaration, 'family' | 'receipt'>,
    place: ReceiptPlace,
  ): Promise<boolean> {
    const path = await useClient().pickFile(t('selection.chooseReceipt'));
    if (path === null) return false;
    return declareLicense({ ...req, receipt: { kind: 'attach', path, place } });
  }

  return {
    faceId,
    detail,
    error,
    loading,
    busy,
    outcome,
    editError,
    licenseError,
    canActivate,
    select,
    reload,
    toggleActivation,
    pending,
    resolveConflict,
    addTag,
    removeTag,
    setFavorite,
    setRating,
    setNotes,
    setFamilyNotes,
    declareLicense,
    attachReceipt,
  };
});
