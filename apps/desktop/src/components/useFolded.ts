// Which sidebar groups the user folded. A view preference, so it may live in localStorage.
import { ref, type Ref } from 'vue';

const KEY = 'fm.sidebar.folded.v1';

function read(): Record<string, boolean> {
  try {
    const parsed: unknown = JSON.parse(globalThis.localStorage?.getItem(KEY) ?? 'null');
    return parsed && typeof parsed === 'object' ? (parsed as Record<string, boolean>) : {};
  } catch {
    return {};
  }
}

/** A remembered folded flag for the group `id`. */
export function useFolded(id: string): { folded: Ref<boolean>; toggle: () => void } {
  const folded = ref(read()[id] === true);
  function toggle(): void {
    folded.value = !folded.value;
    try {
      globalThis.localStorage?.setItem(KEY, JSON.stringify({ ...read(), [id]: folded.value }));
    } catch {
      // storage unavailable: the fold still works for this session
    }
  }
  return { folded, toggle };
}
