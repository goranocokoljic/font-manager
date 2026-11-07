// The alternates of the glyph detail card: what the glyph looks like with each stylistic feature
// fm-core says replaces it, and at both ends of each axis. The webview draws them live.
import type { Axis, AxisValue, GlyphDetail } from '../api/types';
import { variationSettings } from '../preview/variations';

/** One drawing of the glyph, with the CSS that makes it that alternate. */
export interface GlyphAlternate {
  key: string;
  /** The feature's or axis's name; `value` is set for an axis end. */
  name: string;
  value?: number;
  fontFeatureSettings?: string;
  fontVariationSettings?: string;
}

const CSS_TAG = /^[\x20-\x7e]{4}$/;

/**
 * The stylistic features that replace the glyph, then both ends of each axis. An axis end keeps
 * the other axes where the user has `moved` them, so the alternate differs in that axis only.
 */
export function glyphAlternates(
  detail: Pick<GlyphDetail, 'replacedBy'> | null,
  axes: readonly Axis[],
  moved: readonly AxisValue[],
): GlyphAlternate[] {
  const features = (detail?.replacedBy ?? [])
    .filter((f) => f.group === 'stylistic' && !f.automatic && CSS_TAG.test(f.tag))
    .map<GlyphAlternate>((f) => ({
      key: `feature-${f.tag}`,
      name: f.name ?? f.tag,
      fontFeatureSettings: `"${f.tag.replace(/["\\]/g, '\\$&')}" 1`,
    }));
  const ends = axes.flatMap((axis) => {
    if (axis.min == null || axis.max == null || axis.max <= axis.min) return [];
    return [axis.min, axis.max].map<GlyphAlternate>((value) => ({
      key: `axis-${axis.tag}-${value}`,
      name: axis.name || axis.tag,
      value,
      fontVariationSettings: variationSettings(
        [...axes],
        [{ tag: axis.tag, value }, ...moved.filter((m) => m.tag !== axis.tag)],
      ),
    }));
  });
  return [...features, ...ends];
}
