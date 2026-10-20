import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient } from '../api/client';
import type {
  ApiError,
  CoverageReport,
  FaceDetail,
  FaceId,
  FaceSummary,
  FamilyHit,
} from '../api/types';
import { COMPARE_SIZE, clampSize } from '../glyphs/compare';

/** Families the picker lists per search. */
export const COMPARE_RESULTS = 20;

/**
 * The glyph compare overlay: one character of the inspected face next to the
 * same character in a second face the user picks. The picker searches the
 * library by name (fm-core's search, grouped by family) and then lists the
 * family's styles; whether the second face has the character comes from
 * fm-core's `check_coverage`, never from the UI.
 */
export const useGlyphCompareStore = defineStore('glyphCompare', () => {
  const open = ref(false);
  const codePoint = ref<number | null>(null);
  /** The inspected face. */
  const base = ref<FaceDetail | null>(null);
  /** The face it is compared with, once picked. */
  const other = ref<FaceDetail | null>(null);
  /** Whether `other` has the character, from `check_coverage`. */
  const coverage = ref<CoverageReport | null>(null);
  /** The styles of the picked family, for the style picker. */
  const styles = ref<FaceSummary[]>([]);
  const size = ref<number>(COMPARE_SIZE.initial);
  const query = ref('');
  const results = ref<FamilyHit[]>([]);
  const error = ref<ApiError | null>(null);

  // Answers to an older search or pick are dropped.
  let searchRequest = 0;
  let pickRequest = 0;

  /** Opens the overlay for character `cp` of face `face`; a second face stays picked. */
  async function start(face: FaceId, cp: number): Promise<void> {
    open.value = true;
    codePoint.value = cp;
    error.value = null;
    if (base.value?.face.id !== face) base.value = null;
    const loads: Promise<void>[] = [search(query.value)];
    if (other.value) loads.push(pickFace(other.value.face.id));
    try {
      const detail = await useClient().getFace(face);
      if (codePoint.value === cp) base.value = detail;
    } catch (e) {
      error.value = e as ApiError;
    }
    await Promise.all(loads);
  }

  function close(): void {
    open.value = false;
  }

  /** Searches the library by name for the picker. */
  async function search(text: string): Promise<void> {
    query.value = text;
    const mine = ++searchRequest;
    try {
      const r = await useClient().search({
        query: text.trim(),
        page: { offset: 0, limit: COMPARE_RESULTS },
      });
      if (mine === searchRequest) results.value = r.families;
    } catch (e) {
      if (mine === searchRequest) error.value = e as ApiError;
    }
  }

  /** Picks a family from the results: its best face, and its styles for the style picker. */
  async function pickFamily(hit: FamilyHit): Promise<void> {
    const first = hit.faces[0];
    if (first === undefined) return;
    const mine = ++pickRequest;
    const [family] = await Promise.all([
      useClient()
        .getFamily(hit.family.id)
        .catch(() => null),
      pickFace(first, mine),
    ]);
    if (mine === pickRequest) styles.value = family?.faces ?? [];
  }

  /** Picks the second face: its detail (metrics, names) and whether it has the character. */
  async function pickFace(face: FaceId, mine = ++pickRequest): Promise<void> {
    const cp = codePoint.value;
    if (cp === null) return;
    const client = useClient();
    try {
      const [detail, report] = await Promise.all([
        client.getFace(face),
        client.checkCoverage({ face, target: { kind: 'text', text: String.fromCodePoint(cp) } }),
      ]);
      if (mine !== pickRequest || cp !== codePoint.value) return;
      other.value = detail;
      coverage.value = report;
      error.value = null;
    } catch (e) {
      if (mine === pickRequest) error.value = e as ApiError;
    }
  }

  function setSize(px: number): void {
    size.value = clampSize(px);
  }

  return {
    open,
    codePoint,
    base,
    other,
    coverage,
    styles,
    size,
    query,
    results,
    error,
    start,
    close,
    search,
    pickFamily,
    pickFace,
    setSize,
  };
});
