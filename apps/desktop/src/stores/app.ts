import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, AppInfo } from '../api/types';

/** What the running app is, as fm-api reports it. */
export const useAppStore = defineStore('app', () => {
  const info = ref<AppInfo | null>(null);
  const error = ref<ApiError | null>(null);

  async function load(): Promise<void> {
    try {
      info.value = await useClient().getAppInfo();
      error.value = null;
    } catch (e) {
      error.value = e as ApiError;
    }
  }

  return { info, error, load };
});
