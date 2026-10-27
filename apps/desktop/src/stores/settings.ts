import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient, type FullSettings } from '../api/client';
import type {
  ApiError,
  AppFolders,
  Density,
  EndpointInfo,
  McpStatus,
  SettingsUpdate,
} from '../api/types';
import { DEFAULT_DENSITY } from '../list/density';

/**
 * The user's settings and the app folders, as fm-api reports them. Every change is
 * sent at once and the answer replaces the local copy, so what the window shows is
 * what was saved. The sources live in the sources store.
 */
export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<FullSettings | null>(null);
  const folders = ref<AppFolders | null>(null);
  /** The local API endpoint (Settings → Local API); read when that section opens. */
  const endpoint = ref<EndpointInfo | null>(null);
  /** The MCP server (Settings → AI assistants); read when that section opens. */
  const mcp = ref<McpStatus | null>(null);
  /** The last request that failed, for the Settings window to show. */
  const error = ref<ApiError | null>(null);

  // Changes run one after another, so a slow answer never overwrites a newer one.
  let queue: Promise<unknown> = Promise.resolve();
  function enqueue<T>(work: () => Promise<T>): Promise<T> {
    const next = queue.then(work, work);
    queue = next.catch(() => undefined);
    return next;
  }

  async function attempt(work: () => Promise<void>): Promise<boolean> {
    try {
      await work();
      error.value = null;
      return true;
    } catch (e) {
      error.value = e as ApiError;
      return false;
    }
  }

  /** Reads the settings (at app start, so the appearance applies everywhere). */
  function load(): Promise<boolean> {
    return attempt(async () => {
      settings.value = await useClient().getSettings();
    });
  }

  /** Reads what only the Settings window shows: the app folders. */
  function loadWindow(): Promise<boolean> {
    return attempt(async () => {
      folders.value = await useClient().getAppFolders();
    });
  }

  /** Reads the local API address and token. */
  function loadEndpoint(): Promise<boolean> {
    return attempt(async () => {
      endpoint.value = await useClient().endpointInfo();
    });
  }

  /** Makes a new local API token; programs with the old one are refused from now on. */
  function regenerateEndpointToken(): Promise<boolean> {
    return enqueue(() =>
      attempt(async () => {
        endpoint.value = await useClient().regenerateEndpointToken();
      }),
    );
  }

  /** Reads whether the MCP server is on, its URL and the client snippets. */
  function loadMcp(): Promise<boolean> {
    return attempt(async () => {
      mcp.value = await useClient().mcpStatus();
    });
  }

  /** Turns the MCP server on or off; false (and `error` set) when the API refused. */
  function setMcpEnabled(enabled: boolean): Promise<boolean> {
    return enqueue(() =>
      attempt(async () => {
        mcp.value = await useClient().setMcpEnabled({ enabled });
      }),
    );
  }

  /** Allows or refuses one MCP tool that changes something; false (and `error` set) when refused. */
  function setMcpGrant(tool: string, granted: boolean): Promise<boolean> {
    return enqueue(() =>
      attempt(async () => {
        mcp.value = await useClient().setMcpGrant({ tool, granted });
      }),
    );
  }

  /** Changes some settings; false (and `error` set) when the API refused. */
  function update(patch: SettingsUpdate): Promise<boolean> {
    return enqueue(() =>
      attempt(async () => {
        settings.value = await useClient().updateSettings(patch);
      }),
    );
  }

  /** What the Activate button does; session until the settings are read. */
  const activationMode = computed(() => settings.value?.activationMode ?? 'session');
  /** What activation does on a name clash; keep the existing font until read. */
  const onConflict = computed(() => settings.value?.onConflict ?? 'keepExisting');
  /** The row density of the font list and table; comfortable until read. */
  const density = computed<Density>(() => settings.value?.density ?? DEFAULT_DENSITY);

  /** Saves the row density (the toolbar, the palette and Settings all go through here). */
  function setDensity(value: Density): Promise<boolean> {
    return update({ density: value });
  }

  return {
    settings,
    folders,
    endpoint,
    mcp,
    error,
    activationMode,
    onConflict,
    density,
    load,
    loadWindow,
    loadEndpoint,
    regenerateEndpointToken,
    loadMcp,
    setMcpEnabled,
    setMcpGrant,
    update,
    setDensity,
  };
});
