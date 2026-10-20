// Copying with the inline "Copied" / "Could not copy" feedback of CopyValue.
import { onBeforeUnmount, ref } from 'vue';
import { copyText } from '../api/clipboard';

/**
 * Copy with inline feedback: `status` is "copied" or "failed" for 1.5 s after
 * `copy`, which takes the text or a promise of it (a value fm-core builds).
 */
export function useCopy() {
  const status = ref<'copied' | 'failed' | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy(text: string | Promise<string>): Promise<void> {
    try {
      await copyText(await text);
      status.value = 'copied';
    } catch {
      status.value = 'failed';
    }
    clearTimeout(timer);
    timer = setTimeout(() => (status.value = null), 1500);
  }

  onBeforeUnmount(() => clearTimeout(timer));
  return { status, copy };
}
