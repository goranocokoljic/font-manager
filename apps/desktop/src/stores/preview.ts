import { defineStore } from 'pinia';
import { ref, watch } from 'vue';
import type { SizeBasis } from '../api/types';
import {
  isPangramLanguage,
  SAMPLE_KINDS,
  type PangramLanguage,
  type SampleKind,
} from '../preview/sampleText';

/** The localStorage key of the saved preview settings (a view preference). */
export const PREVIEW_STORAGE_KEY = 'fm.preview.v1';

/** The view the "All fonts" list uses. Later views (collections, sets) pass their own id. */
export const ALL_FONTS_VIEW = 'all';

/** Bounds of the numeric settings. Size matches `PreviewRequest.size_px` (8–300). */
export const SIZE = { min: 8, max: 300, step: 1 } as const;
export const LINE_HEIGHT = { min: 0.8, max: 3, step: 0.05 } as const;
/** Tracking (letter spacing) in em. */
export const TRACKING = { min: -0.2, max: 1, step: 0.01 } as const;
/** Longest custom text kept, in UTF-16 code units. */
export const CUSTOM_TEXT_MAX = 1000;

export type Alignment = 'start' | 'center' | 'end';
export type Surface = 'auto' | 'light' | 'dark';
/** Auto size (E16.2): off, or the metric every row's size is matched on. */
export type AutoSize = 'off' | SizeBasis;

export interface PreviewSettings {
  sample: SampleKind;
  customText: string;
  language: PangramLanguage;
  sizePx: number;
  lineHeight: number;
  trackingEm: number;
  align: Alignment;
  /** CSS hex color of the sample text; `null` uses the surface's text color. */
  textColor: string | null;
  /** CSS hex color behind the rows; `null` uses the surface's background. */
  backgroundColor: string | null;
  /** `auto` follows the app theme. */
  surface: Surface;
  /** Scales each row so every face's x-height (or cap height) matches at `sizePx`. */
  autoSize: AutoSize;
}

export const DEFAULT_PREVIEW: Readonly<PreviewSettings> = Object.freeze({
  sample: 'pangram',
  customText: '',
  language: 'en',
  sizePx: 28,
  lineHeight: 1.2,
  trackingEm: 0,
  align: 'start',
  textColor: null,
  backgroundColor: null,
  surface: 'auto',
  autoSize: 'off',
});

const ALIGNMENTS: readonly Alignment[] = ['start', 'center', 'end'];
const SURFACES: readonly Surface[] = ['auto', 'light', 'dark'];
const AUTO_SIZES: readonly AutoSize[] = ['off', 'xHeight', 'capHeight'];
const HEX_COLOR = /^#[0-9a-f]{6}$/i;

/** Keeps `value` inside `bounds`, rounded to its step; anything not a finite number gives `fallback`. */
export function clampSetting(
  value: unknown,
  bounds: { min: number; max: number; step: number },
  fallback: number,
): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return fallback;
  const stepped = Math.round(value / bounds.step) * bounds.step;
  // Round off float noise from the step (0.05 * 23 = 1.1500000000000001).
  const clean = Number(stepped.toFixed(4));
  return Math.min(bounds.max, Math.max(bounds.min, clean));
}

function pick<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return allowed.includes(value as T) ? (value as T) : fallback;
}

function color(value: unknown): string | null {
  return typeof value === 'string' && HEX_COLOR.test(value) ? value.toLowerCase() : null;
}

/** A complete, valid settings object from anything (a saved entry, a partial update). */
export function normalizePreview(raw: unknown): PreviewSettings {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Partial<
    Record<keyof PreviewSettings, unknown>
  >;
  const d = DEFAULT_PREVIEW;
  return {
    sample: pick(
      r.sample,
      SAMPLE_KINDS.map((k) => k.value),
      d.sample,
    ),
    customText:
      typeof r.customText === 'string' ? r.customText.slice(0, CUSTOM_TEXT_MAX) : d.customText,
    language: isPangramLanguage(r.language) ? r.language : d.language,
    sizePx: clampSetting(r.sizePx, SIZE, d.sizePx),
    lineHeight: clampSetting(r.lineHeight, LINE_HEIGHT, d.lineHeight),
    trackingEm: clampSetting(r.trackingEm, TRACKING, d.trackingEm),
    align: pick(r.align, ALIGNMENTS, d.align),
    textColor: color(r.textColor),
    backgroundColor: color(r.backgroundColor),
    surface: pick(r.surface, SURFACES, d.surface),
    autoSize: pick(r.autoSize, AUTO_SIZES, d.autoSize),
  };
}

/** Height of one preview row: the sample line plus the name label and padding. */
export function previewRowHeight(settings: Pick<PreviewSettings, 'sizePx' | 'lineHeight'>): number {
  return Math.ceil(settings.sizePx * settings.lineHeight) + 30;
}

function readSaved(storage: Storage | undefined): Record<string, PreviewSettings> {
  const views: Record<string, PreviewSettings> = {};
  try {
    const parsed: unknown = JSON.parse(storage?.getItem(PREVIEW_STORAGE_KEY) ?? 'null');
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      for (const [view, value] of Object.entries(parsed)) views[view] = normalizePreview(value);
    }
  } catch {
    // A damaged entry falls back to the defaults.
  }
  return views;
}

function browserStorage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

/**
 * Preview settings (sample text, size, spacing, colors, surface, auto size) for each view. Every row
 * of a view's list reads them, so a change applies to all rows at once; they are saved per
 * view as a view preference.
 */
export const usePreviewStore = defineStore('preview', () => {
  const storage = browserStorage();
  const views = ref<Record<string, PreviewSettings>>(readSaved(storage));

  /** The settings of `view`; the defaults until something is changed. */
  function settingsFor(view: string): PreviewSettings {
    return views.value[view] ?? { ...DEFAULT_PREVIEW };
  }

  /** Changes some settings of `view`; invalid values fall back as in `normalizePreview`. */
  function update(view: string, patch: Partial<PreviewSettings>): void {
    views.value = { ...views.value, [view]: normalizePreview({ ...settingsFor(view), ...patch }) };
  }

  /** Puts `view` back to the defaults. */
  function reset(view: string): void {
    const rest = { ...views.value };
    delete rest[view];
    views.value = rest;
  }

  watch(views, (value) => {
    try {
      storage?.setItem(PREVIEW_STORAGE_KEY, JSON.stringify(value));
    } catch {
      // Storage full or blocked: the settings still work for this session.
    }
  });

  return { views, settingsFor, update, reset };
});
