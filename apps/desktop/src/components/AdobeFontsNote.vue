<script setup lang="ts">
// The Adobe Fonts entry of the online catalogs (E22.6): a note and a link, nothing
// else. Adobe Fonts are encrypted and served by Creative Cloud, so the app does not
// look for Creative Cloud, list its fonts or call the network; the link opens the
// site in the default browser. In the center (`bar`, E54.5) its title sits in the app's top
// bar, at the library's height and title style, with no control that would have nothing to do.
import { ref } from 'vue';
import { useClient } from '../api/client';
import { errorMessage, t } from '../i18n';
import AppToolbar from './AppToolbar.vue';

const props = withDefaults(defineProps<{ level?: 1 | 2 | 3; bar?: boolean }>(), {
  level: 2,
  bar: false,
});

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
  <section
    class="catalog-note"
    :class="{ bar }"
    aria-labelledby="adobe-fonts-title"
    data-testid="adobe-fonts-note"
  >
    <AppToolbar
      v-if="bar"
      bare
      title-id="adobe-fonts-title"
      :title="t('catalogs.adobeFonts.name')"
    />
    <component :is="heading" v-else id="adobe-fonts-title" class="name">
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
/* In the center: the bar spans the column, the note keeps its reading width under it. */
.catalog-note.bar {
  display: flex;
  flex: 1;
  flex-direction: column;
  max-width: none;
  padding: 0;
}
.catalog-note.bar > :not(header) {
  max-width: 60ch;
  margin-inline: 16px;
}
.catalog-note.bar > .text {
  margin-block-start: 16px;
}
.catalog-note.bar > .link {
  align-self: flex-start;
}
.name {
  margin: 0 0 8px;
  font-size: var(--fm-text-lg);
  font-weight: 600;
}
.text {
  margin: 0 0 12px;
  line-height: 1.5;
}
.link {
  color: var(--fm-fg);
  text-decoration: underline;
}
.link:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.error {
  color: var(--fm-danger);
}
</style>
