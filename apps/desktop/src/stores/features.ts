import { defineStore } from 'pinia';
import { reactive } from 'vue';
import { useClient } from '../api/client';
import type { FaceId, FeatureInfo } from '../api/types';

/**
 * OpenType features switched in the inspector's Features tab, per face, while
 * the app runs (not saved). Only switches that differ from the feature's
 * default are kept, so "Reset" is forgetting the face. Preview rows of the
 * face read `settings` for their `font-feature-settings`.
 */
export const useFeaturesStore = defineStore('features', () => {
  /** Per face: tag → on, for features switched away from their default. */
  const changed = reactive(new Map<FaceId, Record<string, boolean>>());

  /** Whether `feature` is on for `face`. Automatic features are always on. */
  function isOn(face: FaceId, feature: FeatureInfo): boolean {
    if (feature.automatic) return true;
    return changed.get(face)?.[feature.tag] ?? feature.defaultOn ?? false;
  }

  /** Switches `feature` of `face` on or off. Automatic features cannot be switched. */
  function set(face: FaceId, feature: FeatureInfo, on: boolean): void {
    if (feature.automatic) return;
    const own = { ...changed.get(face) };
    if (on === (feature.defaultOn ?? false)) delete own[feature.tag];
    else own[feature.tag] = on;
    if (Object.keys(own).length) changed.set(face, own);
    else changed.delete(face);
  }

  /** Back to the defaults for `face`. */
  function reset(face: FaceId): void {
    changed.delete(face);
  }

  /** Whether any feature of `face` differs from its default. */
  function isChanged(face: FaceId): boolean {
    return changed.has(face);
  }

  /** The CSS `font-feature-settings` for `face`: `"smcp" 1, "liga" 0`, or undefined. */
  function settings(face: FaceId): string | undefined {
    const own = changed.get(face);
    if (!own) return undefined;
    return Object.keys(own)
      .sort()
      .map((tag) => `"${tag}" ${own[tag] ? 1 : 0}`)
      .join(', ');
  }

  /** "Copy CSS": `font-feature-settings` for the features of `face` switched from their default (fm-core). */
  async function css(face: FaceId): Promise<string> {
    const own = changed.get(face) ?? {};
    const features = Object.keys(own).map((tag) => ({ tag, on: own[tag]! }));
    const r = await useClient().cssValues({ face, variations: [], features });
    return r.featureSettings;
  }

  return { isOn, set, reset, isChanged, settings, css };
});
