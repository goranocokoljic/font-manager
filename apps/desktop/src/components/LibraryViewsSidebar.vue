<script setup lang="ts">
// The rows at the top of the sidebar: All fonts, Recently added, Activated, Favorites. Each
// shows its fonts in the list; the name of the button is the view's, the count describes it.
import { computed, onMounted } from 'vue';
import { formatNumber, t } from '../i18n';
import { useCatalogsStore } from '../stores/catalogs';
import { LIBRARY_VIEWS, useLibraryViewsStore } from '../stores/libraryViews';
import { useQuickCollectionStore } from '../stores/quickCollection';
import UiIcon from './ui/UiIcon.vue';

const store = useLibraryViewsStore();
const quick = useQuickCollectionStore();
const catalogs = useCatalogsStore();
onMounted(() => void store.refresh());

/** While the center shows a review or a catalog, no view is the current one. */
const listShown = computed(() => !quick.open && catalogs.open === null);
</script>

<template>
  <section class="side-section views" :aria-label="t('libraryViews.heading')">
    <ul class="side-list">
      <li v-for="view in LIBRARY_VIEWS" :key="view.id">
        <button
          type="button"
          class="side-row"
          :data-view="view.id"
          :aria-current="listShown && store.current === view.id ? 'true' : undefined"
          :aria-describedby="store.counts[view.id] == null ? undefined : `fm-view-${view.id}`"
          @click="store.show(view.id)"
        >
          <span class="side-lead"><UiIcon :name="view.icon" /></span>
          <span class="side-label">{{ t(`libraryViews.${view.id}`) }}</span>
          <span
            v-if="store.counts[view.id] != null"
            :id="`fm-view-${view.id}`"
            class="side-count"
            aria-hidden="true"
            :aria-label="t('home.families', { count: store.counts[view.id]! })"
            >{{ formatNumber(store.counts[view.id]!) }}</span
          >
        </button>
      </li>
    </ul>
  </section>
</template>
