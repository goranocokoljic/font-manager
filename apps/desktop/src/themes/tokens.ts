// How a named theme (E47.1, `listThemes()`) becomes CSS: each of its tokens is the
// `--fm-*` custom property of the same name in styles/theme.css.
import type { Theme, ThemeDefinition, ThemeTokens } from '../api/types';

/** The light or dark token set of a theme. */
export type Scheme = 'light' | 'dark';

/** The `--fm-*` property of each theme token. */
export const TOKEN_PROPERTIES: Record<keyof ThemeTokens, string> = {
  accent: '--fm-accent',
  onAccent: '--fm-on-accent',
  bg: '--fm-bg',
  surface: '--fm-surface',
  surface2: '--fm-surface-2',
  border: '--fm-border',
};

/** The custom properties of one token set, e.g. `{ '--fm-accent': '#c56eb2', … }`. */
export function tokenStyle(tokens: ThemeTokens): Record<string, string> {
  return Object.fromEntries(
    (Object.keys(TOKEN_PROPERTIES) as (keyof ThemeTokens)[]).map((k) => [
      TOKEN_PROPERTIES[k],
      tokens[k],
    ]),
  );
}

/** Which token set applies: the Light or Dark setting, or the OS for System. */
export function resolveScheme(theme: Theme, osDark: boolean): Scheme {
  if (theme === 'light' || theme === 'dark') return theme;
  return osDark ? 'dark' : 'light';
}

/** The theme with `id`, else the first (the default, Paper); undefined for no themes. */
export function findTheme(
  themes: readonly ThemeDefinition[],
  id: string | undefined,
): ThemeDefinition | undefined {
  return themes.find((t) => t.id === id) ?? themes[0];
}

/** Writes a token set onto `el` (`:root`) as inline custom properties. */
export function writeTokens(el: HTMLElement, tokens: ThemeTokens): void {
  for (const [prop, value] of Object.entries(tokenStyle(tokens))) el.style.setProperty(prop, value);
}

/** Takes the inline tokens off `el` again, so styles/theme.css applies. */
export function clearTokens(el: HTMLElement): void {
  for (const prop of Object.values(TOKEN_PROPERTIES)) el.style.removeProperty(prop);
}
