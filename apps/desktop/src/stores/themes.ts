import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, ThemeDefinition } from '../api/types';
import { withViewTransition } from '../motion/withViewTransition';
import { clearTokens, findTheme, resolveScheme, writeTokens, type Scheme } from '../themes/tokens';
import { useSettingsStore } from './settings';

const DARK_QUERY = '(prefers-color-scheme: dark)';
/** The root class that times the view transition of a theme change (styles/theme.css). */
export const THEME_CHANGE_CLASS = 'fm-theme-change';

/**
 * The named themes (E47.1) and the one that applies. The choice is the `themeId` setting, so
 * it survives a restart; App.vue writes the active token set onto `:root` whenever the theme,
 * the Light/Dark setting or the OS appearance changes.
 */
export const useThemesStore = defineStore('themes', () => {
  const settings = useSettingsStore();
  const themes = ref<ThemeDefinition[]>([]);
  const error = ref<ApiError | null>(null);
  /** The theme just chosen, shown while its save is on the way. */
  const pending = ref<string | null>(null);

  const media = typeof window !== 'undefined' ? window.matchMedia?.(DARK_QUERY) : undefined;
  const osDark = ref(media?.matches ?? false);
  media?.addEventListener?.('change', (e) => (osDark.value = e.matches));

  /** The id of the theme that applies: the one being saved, else the saved one. */
  const activeId = computed(() => pending.value ?? settings.settings?.themeId);
  /** The theme that applies; undefined until the themes are read. */
  const active = computed(() => findTheme(themes.value, activeId.value));
  /** The token set that applies now: the Light/Dark setting, or the OS for System. */
  const scheme = computed<Scheme>(() =>
    resolveScheme(settings.settings?.theme ?? 'system', osDark.value),
  );

  /** Reads the themes the app ships. */
  async function load(): Promise<boolean> {
    try {
      themes.value = await useClient().listThemes();
      error.value = null;
      return true;
    } catch (e) {
      error.value = e as ApiError;
      return false;
    }
  }

  /**
   * Writes the active token set onto `root`; nothing before the themes are read. The default
   * theme (Paper, listed first) is styles/theme.css itself, so it clears the inline tokens and
   * the style sheet's own values apply (the contrast tests hold the two equal).
   */
  function apply(root: HTMLElement = document.documentElement): void {
    const theme = active.value;
    if (!theme) return;
    if (theme === themes.value[0]) clearTokens(root);
    else writeTokens(root, theme[scheme.value]);
  }

  /**
   * Applies a theme at once, in a crossfade at the move time (200 ms; a cut under reduced
   * motion), then saves it as `themeId`. False (and the saved theme back) when the save fails.
   */
  async function choose(id: string): Promise<boolean> {
    if (id === activeId.value && pending.value === null) return true;
    const root = document.documentElement;
    root.classList.add(THEME_CHANGE_CLASS);
    await withViewTransition(
      () => {
        pending.value = id;
        apply(root);
      },
      () => root.classList.remove(THEME_CHANGE_CLASS),
    );
    const ok = await settings.update({ themeId: id });
    // A later choice owns `pending` now; it clears it when its own save is done.
    if (pending.value === id) pending.value = null;
    apply(root);
    return ok;
  }

  return { themes, error, activeId, active, scheme, load, apply, choose };
});
