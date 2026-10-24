<script setup lang="ts">
// The sidebar's "Online catalogs" group. Pressing an entry shows it in the center;
// Adobe Fonts is the only entry so far, and it is a note (E22.6).
import { t } from '../i18n';
import { useCatalogsStore } from '../stores/catalogs';

const store = useCatalogsStore();
</script>

<template>
  <section class="catalogs" aria-labelledby="online-catalogs-title">
    <h2 id="online-catalogs-title" class="group">{{ t('home.sidebarGroups.online') }}</h2>
    <button
      type="button"
      class="entry"
      data-testid="catalog-adobe-fonts"
      :aria-current="store.open === 'adobeFonts' ? 'true' : undefined"
      @click="store.show('adobeFonts')"
    >
      {{ t('catalogs.adobeFonts.name') }}
    </button>
  </section>
</template>

<style scoped>
.catalogs {
  padding: 8px;
}
.group {
  margin: 0;
  padding: 8px;
  color: var(--fm-muted);
  font-size: 11.5px;
  font-weight: 600;
}
.entry {
  display: block;
  width: 100%;
  padding: 4px 8px;
  border: 0;
  border-radius: 6px;
  background: none;
  color: inherit;
  font: inherit;
  text-align: start;
  cursor: pointer;
}
.entry:hover,
.entry:focus-visible {
  background: var(--fm-bg);
}
.entry[aria-current='true'] {
  color: var(--fm-accent);
  font-weight: 600;
}
</style>
