import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';
import type { Density, LanguagePreset } from '../api/types';
import { DEFAULT_DENSITY, DENSITY_ROWS } from '../list/density';
import {
  FALLBACK_LANGUAGE,
  isLanguageCode,
  pickLanguage,
  SAMPLE_KINDS,
  type SampleKind,
} from '../preview/sampleText';
import { useSearchStore } from './search';

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

export interface PreviewSettings {
  sample: SampleKind;
  customText: string;
  /** Language preset code of the pangram (`sr-Cyrl`). */
  language: string;
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
}

export const DEFAULT_PREVIEW: Readonly<PreviewSettings> = Object.freeze({
  sample: 'pangram',
  customText: '',
  language: FALLBACK_LANGUAGE,
  sizePx: 28,
  lineHeight: 1.2,
  trackingEm: 0,
  align: 'start',
  textColor: null,
  backgroundColor: null,
  surface: 'auto',
});

const ALIGNMENTS: readonly Alignment[] = ['start', 'center', 'end'];
const SURFACES: readonly Surface[] = ['auto', 'light', 'dark'];
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
    language: isLanguageCode(r.language) ? r.language : d.language,
    sizePx: clampSetting(r.sizePx, SIZE, d.sizePx),
    lineHeight: clampSetting(r.lineHeight, LINE_HEIGHT, d.lineHeight),
    trackingEm: clampSetting(r.trackingEm, TRACKING, d.trackingEm),
    align: pick(r.align, ALIGNMENTS, d.align),
    textColor: color(r.textColor),
    backgroundColor: color(r.backgroundColor),
    surface: pick(r.surface, SURFACES, d.surface),
  };
}

/**
 * Height of one preview row: the sample line at the size the user chose, plus the
 * name label and padding of the row density.
 */
export function previewRowHeight(
  settings: Pick<PreviewSettings, 'sizePx' | 'lineHeight'>,
  density: Density = DEFAULT_DENSITY,
): number {
  return Math.ceil(settings.sizePx * settings.lineHeight) + DENSITY_ROWS[density].listChromePx;
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

/** The UI/OS languages, most wanted first; empty where the webview gives none. */
function uiLanguages(): readonly string[] {
  const nav = globalThis.navigator;
  if (!nav) return [];
  return nav.languages?.length ? nav.languages : nav.language ? [nav.language] : [];
}

function browserStorage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

/**
 * Preview settings (sample text, size, spacing, colors, surface) for each view. Every row
 * of a view's list reads them, so a change applies to all rows at once; they are saved per
 * view as a view preference. The pangrams come from `list_languages`.
 */
export const usePreviewStore = defineStore('preview', () => {
  const storage = browserStorage();
  const views = ref<Record<string, PreviewSettings>>(readSaved(storage));
  const search = useSearchStore();

  /** The languages the API returns with a pangram, in its order. */
  const pangramLanguages = computed<LanguagePreset[]>(() =>
    search.languages.filter((l) => !!l.pangram),
  );
  const pangrams = computed(
    () => new Map(pangramLanguages.value.map((l) => [l.code, l.pangram ?? ''])),
  );
  /** The pangram language that follows the UI/OS language, else English. */
  const defaultLanguage = computed(() => pickLanguage(uiLanguages(), [...pangrams.value.keys()]));

  /** Loads the languages and their pangrams (once). */
  async function loadPangrams(): Promise<void> {
    await search.loadLanguages();
  }

  /** The pangram of `language`; English for a code without one; empty while loading. */
  function pangramText(language: string): string {
    return pangrams.value.get(language) ?? pangrams.value.get(FALLBACK_LANGUAGE) ?? '';
  }

  /**
   * The settings of `view`; the defaults until something is changed. A saved language
   * without a pangram (once the languages are known) shows the default language.
   */
  function settingsFor(view: string): PreviewSettings {
    const saved = views.value[view];
    if (!saved) return { ...DEFAULT_PREVIEW, language: defaultLanguage.value };
    if (pangrams.value.size && !pangrams.value.has(saved.language)) {
      return { ...saved, language: defaultLanguage.value };
    }
    return saved;
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

  return {
    views,
    pangramLanguages,
    defaultLanguage,
    loadPangrams,
    pangramText,
    settingsFor,
    update,
    reset,
  };
});
