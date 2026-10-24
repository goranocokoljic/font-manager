<script setup lang="ts">
// The sidebar's "Online catalogs" group: each catalog of `list_catalogs` (E22.2) and the
// Adobe Fonts note (E22.6). Pressing an entry shows it in the center. A catalog that is
// off has a "Turn on" button that says it connects to the internet.
import { onMounted } from 'vue';
import { t } from '../i18n';
import { useCatalogsStore } from '../stores/catalogs';

const store = useCatalogsStore();
onMounted(() => void store.load());
</script>

<template>
  <section class="catalogs" aria-labelledby="online-catalogs-title">
    <h2 id="online-catalogs-title" class="group">{{ t('home.sidebarGroups.online') }}</h2>
    <div v-for="c in store.catalogs" :key="c.id" class="row">
      <button
        type="button"
        class="entry"
        :data-testid="`catalog-${c.id}`"
        :aria-current="store.open === c.id ? 'true' : undefined"
        @click="store.show(c.id)"
      >
        <bdi>{{ c.name }}</bdi>
      </button>
      <template v-if="!c.enabled">
        <button
          type="button"
          class="turn-on"
          :data-testid="`catalog-turn-on-${c.id}`"
          :aria-label="t('catalogs.turnOnLabel', { name: c.name })"
          :aria-describedby="`catalog-connects-${c.id}`"
          :disabled="store.busy === c.id"
          @click="store.setEnabled(c.id, true)"
        >
          {{ t('catalogs.turnOn') }}
        </button>
        <span :id="`catalog-connects-${c.id}`" class="connects">{{ t('catalogs.connects') }}</span>
      </template>
    </div>
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
.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  column-gap: 4px;
}
.row .entry {
  flex: 1;
  width: auto;
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
.turn-on {
  padding: 1px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: inherit;
  font: inherit;
  font-size: 11.5px;
}
.connects {
  flex-basis: 100%;
  padding: 0 8px 4px;
  color: var(--fm-muted);
  font-size: 11px;
}
</style>
