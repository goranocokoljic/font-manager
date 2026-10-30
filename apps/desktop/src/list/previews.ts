// What each rendered row previews, shared by the list, grid and waterfall views:
// a style row its face, a family row its first shown face; the text is the API's
// answer for the view's sample (the sample, or a fallback the face covers). A
// virtual style's row previews its face at the style's coordinates. With auto size on
// (E16.2), each row also carries its face's size factor; the factors of the rows on
// screen are asked for in one call. With the metrics overlay on (E16.1), each row also
// carries its face's vertical metrics, asked for the rows on screen only. While the
// view moves fast (E35.8, `list/settle.ts`) nothing is asked for: the rows on screen
// then are gone a few frames later. Once it settles, the rows on screen ask.
import { watchEffect } from 'vue';
import type { AxisValue, FaceSummary, Metrics, SampleFallback } from '../api/types';
import { sampleText } from '../preview/sampleText';
import { useFaceMetricsStore } from '../stores/faceMetrics';
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
  /** The face's vertical metrics while the metrics overlay is on; absent until they arrive. */
  metrics?: Metrics;
}

/**
 * `rendered` gives the rows on screen: their preview text is asked for (one call
 * per tick). `around` gives those rows plus a margin: their families' faces are
 * fetched, so rows scrolled in already have a face to preview. `autoSize` is false
 * for a view that keeps its own sizes (the grid's thumbnails, the waterfall's labelled
 * sizes): its rows keep factor 1 and no factor is asked for. `metrics` is false for a
 * view that draws no metrics lines (the grid's cards): no metrics are asked for.
 * `settled` is false while the view moves fast: no text, factor, metrics or faces are
 * asked for then; what has arrived is still shown.
 */
export function useRowPreviews(
  preview: () => PreviewSettings,
  rendered: () => readonly ListRow[],
  around: () => readonly ListRow[],
  autoSize = true,
  metrics = true,
  settled: () => boolean = () => true,
) {
  const list = useFontListStore();
  const library = useLibraryStore();
  const samples = useSamplesStore();
  const sizes = useSizeFactorsStore();
  const faceMetrics = useFaceMetricsStore();
  const previews = usePreviewStore();

  const faceOf = (row: ListRow) =>
    row.kind === 'face' ? row.face : list.previewFace(row.family.id);

  function previewOf(row: ListRow): RowPreview {
    const face = faceOf(row);
    const sample = face ? sampleText(preview(), face, previews.pangramText) : '';
    const got = face ? samples.get(face.id, sample) : undefined;
    const basis = autoSize ? preview().autoSize : 'off';
    const lines = face && metrics && preview().metrics ? faceMetrics.get(face.id) : undefined;
    return {
      face,
      sample,
      text: got?.text ?? sample,
      fallback: got?.fallback ?? null,
      missing: got?.missing ?? [],
      sizeFactor: face && basis !== 'off' ? sizes.get(face.id, basis) : 1,
      ...(lines ? { metrics: lines } : {}),
      ...(row.kind === 'face' && row.virtual ? { variations: row.virtual.coordinates } : {}),
    };
  }

  watchEffect(() => {
    if (!settled()) return;
    for (const row of rendered()) {
      const face = faceOf(row);
      if (face) samples.want(face.id, sampleText(preview(), face, previews.pangramText));
    }
  });

  watchEffect(() => {
    const basis = autoSize ? preview().autoSize : 'off';
    if (basis === 'off' || !settled()) return;
    for (const row of rendered()) {
      const face = faceOf(row);
      if (face) sizes.want(face.id, basis);
    }
  });

  watchEffect(() => {
    if (!metrics || !preview().metrics || !settled()) return;
    for (const row of rendered()) {
      const face = faceOf(row);
      if (face) faceMetrics.want(face.id);
    }
  });

  watchEffect(() => {
    if (!settled()) return;
    for (const row of around()) {
      const id = row.family.id;
      if (row.kind === 'family' && !library.faces.has(id) && !library.failed.has(id))
        void library.facesOf(id).catch(() => undefined);
    }
  });

  return { previewOf };
}
