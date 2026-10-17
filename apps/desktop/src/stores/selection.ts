import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient } from '../api/client';
import type {
  ActivationMode,
  ApiError,
  FaceDetail,
  FaceId,
  OutcomeStatus,
  Target,
} from '../api/types';

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
    if (id === null) {
      request++;
      detail.value = null;
      error.value = null;
      loading.value = false;
      return;
    }
    await load(id);
  }

  /** The selected face can be activated: it is loaded, healthy and no request is in flight. */
  const canActivate = computed(
    () => detail.value?.face.health.kind === 'ok' && !busy.value && !loading.value,
  );

  /**
   * Session (default): activates the selected face for the session, or deactivates it
   * when active. Permanent: activates it permanently (a session activation becomes
   * permanent), or deactivates it when it is already permanent.
   */
  async function toggleActivation(mode: ActivationMode = 'session'): Promise<void> {
    const face = detail.value?.face;
    if (!face || busy.value) return;
    const target: Target = { kind: 'faces', faces: [face.id] };
    const deactivate = mode === 'permanent' ? face.active === 'permanent' : face.active !== null;
    busy.value = true;
    try {
      const client = useClient();
      const results = deactivate
        ? await client.deactivate({ target })
        : await client.activate({ target, mode });
      outcome.value = results.find((r) => r.face === face.id)?.outcome ?? null;
    } catch (e) {
      outcome.value = { status: 'failed', error: e as ApiError };
    } finally {
      busy.value = false;
    }
    if (faceId.value === face.id) await load(face.id);
  }

  return {
    faceId,
    detail,
    error,
    loading,
    busy,
    outcome,
    canActivate,
    select,
    toggleActivation,
  };
});
