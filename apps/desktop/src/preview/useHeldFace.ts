// Holds one face registered with the webview while a component shows it, and
// gives its CSS family name once it has loaded (null while loading, on failure
// or with no face). Releases the face on change and on unmount.
import { type Ref, onBeforeUnmount, ref, watch } from 'vue';
import type { FaceId } from '../api/types';
import { useFaceLoader } from './faceLoader';

export function useHeldFace(face: Ref<FaceId | null>): Ref<string | null> {
  const loader = useFaceLoader();
  const family = ref<string | null>(null);
  let held: FaceId | null = null;
  function hold(id: FaceId | null): void {
    if (held !== null) loader.release(held);
    held = id;
    family.value = null;
    if (id === null) return;
    loader.acquire(id).then(
      (name) => {
        if (held === id) family.value = name;
      },
      () => undefined,
    );
  }
  watch(face, hold, { immediate: true });
  onBeforeUnmount(() => hold(null));
  return family;
}
