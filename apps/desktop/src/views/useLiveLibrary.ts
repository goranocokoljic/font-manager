// What the library view and a family page both need listening. The two hand over to each
// other without stopping it: a stop and a new load would make the list forget its faces, and
// with them a selected style row (E42.1).
import { onBeforeUnmount, onMounted } from 'vue';
import { useRoute, type RouteRecordName } from 'vue-router';
import { useActivationStore } from '../stores/activation';
import { useLibraryStore } from '../stores/library';
import { useProblemsStore } from '../stores/problems';
import { useSelectionStore } from '../stores/selection';

/** Starts it all on mount; stops it on unmount unless the next route is `partner`. */
export function useLiveLibrary(partner: RouteRecordName): void {
  const route = useRoute();
  const library = useLibraryStore();
  const activation = useActivationStore();
  const selection = useSelectionStore();
  const problems = useProblemsStore();

  onMounted(() => {
    void library.load();
    void activation.load();
    void selection.start();
    void problems.load();
  });
  onBeforeUnmount(() => {
    if (route.name === partner) return;
    void library.stop();
    void activation.stop();
    void selection.stop();
    void problems.stop();
  });
}
