// What each rendered row previews, shared by the list, grid and waterfall views:
// a style row its face, a family row its first shown face; the text is the API's
// answer for the view's sample (the sample, or a fallback the face covers). A
// virtual style's row previews its face at the style's coordinates.
import { watchEffect } from 'vue';
import type { AxisValue, FaceSummary, SampleFallback } from '../api/types';
import { sampleText } from '../preview/sampleText';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import type { PreviewSettings } from '../stores/preview';
import { useSamplesStore } from '../stores/samples';
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
}

/**
 * `rendered` gives the rows on screen: their preview text is asked for (one call
 * per tick). `around` gives those rows plus a margin: their families' faces are
 * fetched, so rows scrolled in already have a face to preview.
 */
export function useRowPreviews(
  preview: () => PreviewSettings,
  rendered: () => readonly ListRow[],
  around: () => readonly ListRow[],
) {
  const list = useFontListStore();
  const library = useLibraryStore();
  const samples = useSamplesStore();

  const faceOf = (row: ListRow) =>
    row.kind === 'face' ? row.face : list.previewFace(row.family.id);

  function previewOf(row: ListRow): RowPreview {
    const face = faceOf(row);
    const sample = face ? sampleText(preview(), face) : '';
    const got = face ? samples.get(face.id, sample) : undefined;
    return {
      face,
      sample,
      text: got?.text ?? sample,
      fallback: got?.fallback ?? null,
      missing: got?.missing ?? [],
      ...(row.kind === 'face' && row.virtual ? { variations: row.virtual.coordinates } : {}),
    };
  }

  watchEffect(() => {
    for (const row of rendered()) {
      const face = faceOf(row);
      if (face) samples.want(face.id, sampleText(preview(), face));
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
