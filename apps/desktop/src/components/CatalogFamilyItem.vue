<script setup lang="ts">
// One family of an online catalog in the catalog browser (E22.2): its name in the UI
// font and the sample in the family itself, live text. The preview file is registered
// by the shared face loader from the app's font scheme; the core fetches it into the
// cache folder the first time a row shows it, only while the catalog is on. As a list
// row, a grid card or a waterfall block (the six sizes).
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { useClient } from '../api/client';
import type { CatalogFamily, CatalogId } from '../api/types';
import { t } from '../i18n';
import { cardText } from '../list/grid';
import { styleCount } from '../list/rows';
import { WATERFALL_SIZES } from '../list/waterfall';
import { catalogFamily, useFaceLoader } from '../preview/faceLoader';
import type { PreviewSettings } from '../stores/preview';

const props = defineProps<{
  catalog: CatalogId;
  family: CatalogFamily;
  text: string;
  preview: PreviewSettings;
  mode: 'list' | 'grid' | 'waterfall';
}>();

type LoadState = 'loading' | 'ready' | 'failed';

const loader = useFaceLoader();
const state = ref<LoadState>('loading');
let held: string | null = null;

function release(): void {
  if (held !== null) loader.releaseSource(held);
  held = null;
}

watch(
  () => [props.catalog, props.family.name] as const,
  ([catalog, name]) => {
    const family = catalogFamily(catalog, name);
    if (held === family) return;
    release();
    held = family;
    state.value = 'loading';
    loader
      .acquireSource(family, () => useClient().catalogFontUrl(catalog, name))
      .then(
        () => {
          if (held === family) state.value = 'ready';
        },
        () => {
          if (held === family) state.value = 'failed';
        },
      );
  },
  { immediate: true },
);
onBeforeUnmount(release);

const fontFamily = computed(
  () =>
    `"${catalogFamily(props.catalog, props.family.name)}", var(--fm-preview-fallback, system-ui), sans-serif`,
);
const sampleStyle = computed(() => ({
  fontFamily: fontFamily.value,
  fontSize: `${props.preview.sizePx}px`,
  lineHeight: String(props.preview.lineHeight),
  letterSpacing: `${props.preview.trackingEm}em`,
  textAlign: props.preview.align,
  color: props.preview.textColor ?? undefined,
}));
const sizeStyle = (sizePx: number) => ({ ...sampleStyle.value, fontSize: `${sizePx}px` });
const styles = computed(() => styleCount(props.family.styles.length));
</script>

<template>
  <div
    class="item"
    :class="mode"
    :data-family="family.name"
    :data-state="state"
    data-testid="catalog-family"
  >
    <span class="label">
      <bdi class="name">{{ family.name }}</bdi>
      <span v-if="mode !== 'grid'" class="detail">
        <span v-if="family.category">{{ family.category }}</span>
        <span>{{ styles }}</span>
        <span v-if="family.axes?.length" class="badge">{{ t('list.variable') }}</span>
      </span>
      <span v-if="state === 'failed'" class="note">{{ t('list.previewUnavailable') }}</span>
    </span>
    <template v-if="mode === 'waterfall'">
      <span v-for="size in WATERFALL_SIZES" :key="size" class="line">
        <span class="size">{{ t('catalogs.sizePx', { size }) }}</span>
        <span class="sample" dir="auto" :style="sizeStyle(size)">{{ text }}</span>
      </span>
    </template>
    <span
      v-else
      class="sample"
      dir="auto"
      :style="mode === 'grid' ? sizeStyle(Math.min(preview.sizePx, 40)) : sampleStyle"
      >{{ mode === 'grid' ? cardText(text) : text }}</span
    >
  </div>
</template>

<style scoped>
.item {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
  box-sizing: border-box;
  height: 100%;
  padding: 0 calc(2 * var(--fm-gap, 8px));
  overflow: hidden;
}
.item.grid {
  margin: 4px;
  height: calc(100% - 8px);
  border: 1px solid var(--fm-border);
  border-radius: 8px;
}
.item.waterfall {
  justify-content: flex-start;
  padding-block: 8px;
}
.label {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.name {
  overflow: hidden;
  color: var(--fm-fg);
  white-space: nowrap;
  text-overflow: ellipsis;
}
.detail {
  display: flex;
  gap: 8px;
}
.badge {
  color: var(--fm-accent);
}
.note {
  color: var(--fm-warning);
}
.line {
  display: flex;
  flex-direction: column;
  padding-bottom: 8px;
}
.size {
  color: var(--fm-muted);
  font-size: 10.5px;
  line-height: 14px;
}
.sample {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
