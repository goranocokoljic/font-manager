<script setup lang="ts">
// The Adobe Fonts entry of the online catalogs (E22.6): a note and a link, nothing
// else. Adobe Fonts are encrypted and served by Creative Cloud, so the app does not
// look for Creative Cloud, list its fonts or call the network; the link opens the
// site in the default browser.
import { ref } from 'vue';
import { useClient } from '../api/client';
import { errorMessage, t } from '../i18n';

const props = withDefaults(defineProps<{ level?: 1 | 2 | 3 }>(), { level: 2 });

const ADOBE_FONTS_URL = 'https://fonts.adobe.com';
const heading = `h${props.level}`;
const error = ref<unknown>(null);

async function openSite(): Promise<void> {
  error.value = null;
  try {
    await useClient().openUrl(ADOBE_FONTS_URL);
  } catch (e) {
    error.value = e;
  }
}
</script>

<template>
  <section class="catalog-note" aria-labelledby="adobe-fonts-title" data-testid="adobe-fonts-note">
    <component :is="heading" id="adobe-fonts-title" class="name">
      {{ t('catalogs.adobeFonts.name') }}
    </component>
    <p class="text">{{ t('catalogs.adobeFonts.note') }}</p>
    <a
      class="link"
      :href="ADOBE_FONTS_URL"
      :title="t('catalogs.adobeFonts.openTitle')"
      @click.prevent="openSite"
      @auxclick.prevent="openSite"
    >
      {{ t('catalogs.adobeFonts.open') }}
    </a>
    <p v-if="error" role="alert" class="error">{{ errorMessage(error) }}</p>
  </section>
</template>

<style scoped>
.catalog-note {
  max-width: 60ch;
  padding: 16px;
}
.name {
  margin: 0 0 8px;
  font-size: 15px;
  font-weight: 600;
}
.text {
  margin: 0 0 12px;
  line-height: 1.5;
}
.link {
  color: var(--fm-accent);
}
.link:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.error {
  color: var(--fm-danger);
}
</style>
