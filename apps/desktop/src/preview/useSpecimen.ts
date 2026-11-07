// The specimen of one face for a text the user chose (E43.4): the text, with the
// characters the face lacks as tofu. Asks the API (`sampleTexts`, through the samples
// store) which characters those are; until the answer arrives the text has no tofu.
// For the family page, Quick Look and Studio, which always show the chosen text.
import { computed, watchEffect, type ComputedRef } from 'vue';
import type { FaceId } from '../api/types';
import { useSamplesStore } from '../stores/samples';
import { specimenOf, type Specimen } from './tofu';

export function useSpecimen(
  face: () => FaceId | null | undefined,
  text: () => string,
): ComputedRef<Specimen> {
  const samples = useSamplesStore();
  watchEffect(() => {
    const id = face();
    if (id != null) samples.want(id, text());
  });
  return computed(() => {
    const id = face();
    const sample = text();
    return specimenOf(sample, id != null ? samples.get(id, sample) : undefined, true);
  });
}
