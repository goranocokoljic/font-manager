// What each rendered row previews, shared by the list, grid and waterfall views:
// a style row its face, a family row its first shown face; the text is the API's
// answer for the view's sample (the sample, or a fallback the face covers). A
// virtual style's row previews its face at the style's coordinates. With auto size on
// (E16.2), each row also carries its face's size factor; the factors of the rows on
// screen are asked for in one call.
import { watchEffect } from 'vue';
import type { AxisValue, FaceSummary, SampleFallback } from '../api/types';
import { sampleText } from '../preview/sampleText';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import { usePreviewStore, type PreviewSettings } from '../stores/preview';
import { useSamplesStore } from '../stores/samples';
import { useSizeFactorsStore } from '../stores/sizeFactors';
import type { ListRow } from './rows';

export interface RowPreview {
  /** The face the row previews; absent until a family's faces are loaded. */
  face: FaceSummary | undefined;
  /** The view's sample text for the face. */
  sample: string;
  /** The text to show: the API's answer, the sample until it arrives. */
  text: string;
  fallback: SampleFallback | null;
  missing: string[];
  /** A virtual style's coordinates; absent for every other row. */
  variations?: AxisValue[];
  /** What the view's size is multiplied by: the auto size factor, 1 when auto size is off. */
  sizeFactor: number;
}

/**
 * `rendered` gives the rows on screen: their preview text is asked for (one call
 * per tick). `around` gives those rows plus a margin: their families' faces are
 * fetched, so rows scrolled in already have a face to preview. `autoSize` is false
 * for a view that keeps its own sizes (the grid's thumbnails, the waterfall's labelled
 * sizes): its rows keep factor 1 and no factor is asked for.
 */
export function useRowPreviews(
  preview: () => PreviewSettings,
  rendered: () => readonly ListRow[],
  around: () => readonly ListRow[],
  autoSize = true,
) {
  const list = useFontListStore();
  const library = useLibraryStore();
  const samples = useSamplesStore();
  const sizes = useSizeFactorsStore();
  const previews = usePreviewStore();

  const faceOf = (row: ListRow) =>
    row.kind === 'face' ? row.face : list.previewFace(row.family.id);

  function previewOf(row: ListRow): RowPreview {
    const face = faceOf(row);
    const sample = face ? sampleText(preview(), face, previews.pangramText) : '';
    const got = face ? samples.get(face.id, sample) : undefined;
    const basis = autoSize ? preview().autoSize : 'off';
    return {
      face,
      sample,
      text: got?.text ?? sample,
      fallback: got?.fallback ?? null,
      missing: got?.missing ?? [],
      sizeFactor: face && basis !== 'off' ? sizes.get(face.id, basis) : 1,
      ...(row.kind === 'face' && row.virtual ? { variations: row.virtual.coordinates } : {}),
    };
  }

  watchEffect(() => {
    for (const row of rendered()) {
      const face = faceOf(row);
      if (face) samples.want(face.id, sampleText(preview(), face, previews.pangramText));
    }
  });

  watchEffect(() => {
    const basis = autoSize ? preview().autoSize : 'off';
    if (basis === 'off') return;
    for (const row of rendered()) {
      const face = faceOf(row);
      if (face) sizes.want(face.id, basis);
    }
  });

  watchEffect(() => {
    for (const row of around()) {
      const id = row.family.id;
      if (row.kind === 'family' && !library.faces.has(id) && !library.failed.has(id))
        void library.facesOf(id).catch(() => undefined);
    }
  });

  return { previewOf };
}
