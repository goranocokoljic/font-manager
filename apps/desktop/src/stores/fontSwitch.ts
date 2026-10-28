import { defineStore } from 'pinia';
import { useClient } from '../api/client';
import { errorMessage, t } from '../i18n';
import type { FaceId, FaceSummary, FontSwitchPayload } from '../api/types';
import { leftWindow, startFontDrag } from '../list/drag';
import type { ListRow, RowKey } from '../list/rows';
import { useFontListStore } from './fontList';
import { useSettingsStore } from './settings';
import { useToastStore } from './toast';

/**
 * Dragging a font to a design app (E34.1). A drag's data must be set while
 * `dragstart` runs, so the Font Switch payload of a row's face is read from
 * fm-api when the pointer goes down on the row (`prime`). When the
 * drag leaves the window, an inactive face is activated for this session so
 * the app it is dropped on can apply it; nothing is deactivated afterwards.
 */
export const useFontSwitchStore = defineStore('fontSwitch', () => {
  const payloads = new Map<FaceId, FontSwitchPayload>();
  const reading = new Map<FaceId, Promise<FontSwitchPayload>>();

  /** Asks fm-api for the payload of `face` (one call in flight per face) and keeps it. */
  function read(face: FaceId): Promise<FontSwitchPayload> {
    let pending = reading.get(face);
    if (!pending) {
      pending = useClient()
        .fontSwitchPayload(face)
        .then((p) => {
          payloads.set(face, p);
          return p;
        })
        .finally(() => reading.delete(face));
      reading.set(face, pending);
    }
    return pending;
  }

  /** The face a design app gets for `row`: a style row's face, a family row's first face. */
  function faceOf(row: ListRow): FaceSummary | undefined {
    return row.kind === 'face' ? row.face : useFontListStore().previewFace(row.family.id);
  }

  /**
   * Reads the payload of `row`'s face again ahead of a drag (the file may have
   * moved since); until the answer comes, a drag carries the last one read. A
   * failed read leaves the drag without it.
   */
  function prime(row: ListRow): void {
    const face = faceOf(row);
    if (face) void read(face.id).catch(() => undefined);
  }

  /** The face of the drag in flight, until it leaves the window or ends. */
  let dragging: FaceSummary | null = null;

  function onLeave(e: DragEvent): void {
    if (!dragging || !leftWindow(e, window.innerWidth, window.innerHeight)) return;
    const face = dragging;
    stopWatching();
    void activateForDrop(face);
  }

  function stopWatching(): void {
    dragging = null;
    document.removeEventListener('dragleave', onLeave);
    document.removeEventListener('dragend', stopWatching);
  }

  /**
   * Starts a font drag of `keys` on `e`; the face of `row`, the row the drag
   * starts on, is the one a design app gets, when its payload is known.
   */
  function startDrag(e: DragEvent, keys: RowKey[], row: ListRow): void {
    const face = faceOf(row);
    const known = face ? payloads.get(face.id) : undefined;
    startFontDrag(e, keys, known);
    stopWatching();
    if (!face || !known) return;
    dragging = face;
    document.addEventListener('dragleave', onLeave);
    document.addEventListener('dragend', stopWatching);
  }

  /** Activates `face` for this session unless it is active; a failure is a toast. */
  async function activateForDrop(face: FaceSummary): Promise<void> {
    if (face.active) return;
    const name = `${face.familyName} ${face.style}`;
    try {
      const [out] = await useClient().activate({
        target: { kind: 'faces', faces: [face.id] },
        mode: 'session',
        onConflict: useSettingsStore().onConflict,
      });
      const outcome = out?.outcome;
      if (outcome?.status === 'done' || outcome?.status === 'unchanged') return;
      const reason =
        outcome?.status === 'skipped'
          ? outcome.reason
          : outcome?.status === 'failed'
            ? errorMessage(outcome.error) || t('activation.failed')
            : t('activation.failed');
      useToastStore().show(t('fontSwitch.activationFailed', { name, reason }));
    } catch (e) {
      const reason = errorMessage(e) || t('activation.failed');
      useToastStore().show(t('fontSwitch.activationFailed', { name, reason }));
    }
  }

  /** What "Copy for Font Switch" puts on the clipboard: the payload of `face` as JSON. */
  async function clipboardText(face: FaceId): Promise<string> {
    return JSON.stringify(await read(face));
  }

  return { prime, startDrag, clipboardText };
});
