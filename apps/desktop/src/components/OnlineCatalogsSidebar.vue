<script setup lang="ts">
// The sidebar's "Catalogs" group: each catalog of `list_catalogs` (E22.2) with its family
// count, and the Adobe Fonts note (E22.6). Pressing an entry shows it in the center. A
// catalog that is off says so and has a "Turn on" icon that says it connects to the internet.
import { onMounted } from 'vue';
import { formatNumber, t } from '../i18n';
import SidebarHead from './SidebarHead.vue';
import UiIcon from './ui/UiIcon.vue';
import UiIconButton from './ui/UiIconButton.vue';
import { useFolded } from './useFolded';
import { useCatalogsStore } from '../stores/catalogs';

const store = useCatalogsStore();
const { folded, toggle } = useFolded('catalogs');
onMounted(() => void store.load());
</script>

<template>
  <section class="side-section" aria-labelledby="online-catalogs-title">
    <SidebarHead
      id="online-catalogs-title"
      :folded="folded"
      controls="online-catalogs-body"
      @toggle="toggle"
    >
      {{ t('home.sidebarGroups.online') }}
    </SidebarHead>
    <div v-show="!folded" id="online-catalogs-body">
      <div
        v-for="c in store.catalogs"
        :key="c.id"
        class="side-row"
        :class="{ current: store.open === c.id }"
      >
        <span class="side-lead"><UiIcon name="globe" /></span>
        <button
          type="button"
          class="side-label"
          :data-testid="`catalog-${c.id}`"
          :aria-current="store.open === c.id ? 'true' : undefined"
          @click="store.show(c.id)"
        >
          <bdi>{{ c.name }}</bdi>
        </button>
        <span class="side-count" :data-testid="`catalog-count-${c.id}`">{{
          c.enabled ? formatNumber(c.familyCount) : t('catalogs.offShort')
        }}</span>
        <span v-if="!c.enabled" class="side-actions">
          <UiIconButton
            icon="power"
            :data-testid="`catalog-turn-on-${c.id}`"
            :label="t('catalogs.turnOnLabel', { name: c.name })"
            :title="`${t('catalogs.turnOn')} — ${t('catalogs.connects')}`"
            :aria-describedby="`catalog-connects-${c.id}`"
            :disabled="store.busy === c.id"
            @click="store.setEnabled(c.id, true)"
          />
          <span :id="`catalog-connects-${c.id}`" class="sr-only">{{ t('catalogs.connects') }}</span>
        </span>
      </div>
      <div class="side-row" :class="{ current: store.open === 'adobeFonts' }">
        <span class="side-lead"><UiIcon name="globe" /></span>
        <button
          type="button"
          class="side-label"
          data-testid="catalog-adobe-fonts"
          :aria-current="store.open === 'adobeFonts' ? 'true' : undefined"
          @click="store.show('adobeFonts')"
        >
          {{ t('catalogs.adobeFonts.name') }}
        </button>
      </div>
    </div>
  </section>
</template>
