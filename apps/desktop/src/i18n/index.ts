// The UI's message catalogs (`src/locales/*.json`) and the one way to read them.
// Every user-visible string goes through `t`; numbers and dates through `Intl`.
// `src/locales/README.md` explains how to add a locale.
import { ref } from 'vue';
import type { ApiError, ErrorCode, Language } from '../api/types';
import de from '../locales/de.json';
import en from '../locales/en.json';

/** The forms of a counted message, chosen by `Intl.PluralRules`; `zero` wins for 0. */
export interface PluralForms {
  zero?: string;
  one?: string;
  two?: string;
  few?: string;
  many?: string;
  other: string;
}

/** A message catalog: nested groups of messages. */
export interface Catalog {
  [key: string]: string | PluralForms | Catalog;
}

type Keys<T, P extends string = ''> = {
  [K in keyof T & string]: T[K] extends string
    ? `${P}${K}`
    : T[K] extends { other: string }
      ? `${P}${K}`
      : Keys<T[K], `${P}${K}.`>;
}[keyof T & string];

/** Every message in the English catalog, as a dotted path (`settings.title`). */
// TEMP-PARTS: `| string` until the parts are merged into en.json.
export type MessageKey = Keys<typeof en> | (string & {});

/** The value a message's `{name}` placeholder takes; numbers are formatted. */
export type MessageParams = Record<string, string | number>;

/** The catalogs that ship, by locale tag. English is complete by definition. */
export const CATALOGS: Readonly<Record<string, Catalog>> = { en, de };

// TEMP-PARTS (removed before merge): parts/<group>.<locale>.json are merged in.
for (const [path, part] of Object.entries(
  import.meta.glob<Catalog>('../locales/parts/*.json', { eager: true, import: 'default' }),
)) {
  const tag = path.split('.').at(-2) ?? '';
  const target = (CATALOGS as Record<string, Catalog>)[tag];
  if (target) Object.assign(target, part);
}

/** The locale every missing message falls back to. */
export const FALLBACK_LOCALE = 'en';

/** The locales the Language setting offers, in catalog order. */
export const LOCALES: readonly string[] = Object.keys(CATALOGS);

const current = ref<string>(FALLBACK_LOCALE);

/** The locale the UI shows now. */
export function locale(): string {
  return current.value;
}

/** Shows the UI in `tag` (a key of `CATALOGS`); an unknown tag shows English. */
export function setLocale(tag: string): void {
  current.value = tag in CATALOGS ? tag : FALLBACK_LOCALE;
}

/**
 * The locale for the Language setting: an explicit choice when a catalog has it;
 * for "system" (or nothing), the first of the OS's preferred languages that has a
 * catalog (`de-AT` finds `de`), else English.
 */
export function resolveLocale(
  language: Language | null | undefined,
  preferred: readonly string[],
): string {
  if (language && language !== 'system') {
    return language in CATALOGS ? language : FALLBACK_LOCALE;
  }
  for (const tag of preferred) {
    const lower = tag.toLowerCase();
    if (lower in CATALOGS) return lower;
    const primary = lower.split('-')[0];
    if (primary in CATALOGS) return primary;
  }
  return FALLBACK_LOCALE;
}

function isPlural(value: unknown): value is PluralForms {
  return typeof value === 'object' && value !== null && typeof (value as PluralForms).other === 'string';
}

function lookup(catalog: Catalog | undefined, key: string): string | PluralForms | undefined {
  let node: string | PluralForms | Catalog | undefined = catalog;
  for (const part of key.split('.')) {
    if (typeof node !== 'object' || node === null || isPlural(node)) return undefined;
    node = (node as Catalog)[part];
  }
  return typeof node === 'string' || isPlural(node) ? node : undefined;
}

/** Whether `key` names a message in `tag`'s catalog (not its fallback). */
export function hasMessage(key: string, tag = current.value): boolean {
  return lookup(CATALOGS[tag], key) !== undefined;
}

/** Formats a number for the UI's locale. */
export function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
  return new Intl.NumberFormat(current.value, options).format(value);
}

/** Formats a date (or epoch milliseconds) for the UI's locale. */
export function formatDate(
  value: Date | number,
  options: Intl.DateTimeFormatOptions = { dateStyle: 'medium', timeStyle: 'short' },
): string {
  return new Intl.DateTimeFormat(current.value, options).format(value);
}

function pick(forms: PluralForms, count: number, tag: string): string {
  if (count === 0 && forms.zero !== undefined) return forms.zero;
  const category = new Intl.PluralRules(tag).select(count) as keyof PluralForms;
  return forms[category] ?? forms.other;
}

/**
 * The message `key` in the UI's locale (English when the locale lacks it; the key
 * itself when no catalog has it), with `{name}` placeholders filled from `params`.
 * A counted message picks its plural form from `params.count`. Reading `t` in a
 * template or a computed makes it follow `setLocale`.
 */
export function t(key: MessageKey, params?: MessageParams): string {
  const tag = current.value;
  let used = tag;
  let message = lookup(CATALOGS[tag], key);
  if (message === undefined) {
    used = FALLBACK_LOCALE;
    message = lookup(CATALOGS[FALLBACK_LOCALE], key);
  }
  if (message === undefined) return key;
  const text = isPlural(message) ? pick(message, Number(params?.count ?? 0), used) : message;
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = params[name];
    if (value === undefined) return whole;
    return typeof value === 'number' ? new Intl.NumberFormat(used).format(value) : value;
  });
}

/**
 * The text to show for a failed request: the catalog's message for its `ErrorCode`,
 * where the core's English message fills `{message}` (the English catalog shows just
 * that). Anything that is not an `ApiError` shows the generic internal message.
 */
export function errorMessage(error: unknown): string {
  const e = error as Partial<ApiError> | null | undefined;
  const message = typeof e?.message === 'string' ? e.message : '';
  const code: ErrorCode = typeof e?.code === 'string' ? e.code : 'internal';
  const key = `errors.${code}` as MessageKey;
  const text = t(key, { message });
  // An unknown code (a newer core) has no message: show the core's own words.
  return text === key || text === '' ? message || t('errors.internal', { message }) : text;
}
