<script setup lang="ts">
// One family of an online catalog in the catalog browser (E22.2): its name in the UI
// font and the sample in the family itself, live text. The preview file is registered
// by the shared face loader from the app's font scheme; the core fetches it into the
// cache folder the first time a row shows it, only while the catalog is on. As a list
// row, a grid card or a waterfall block (the six sizes). Its two actions are icons (E50.6). The
// activation dot (E50.5) is the Install: always off (a catalog family is not in the library),
// a click downloads the family into the download folder and activates it in one call (E22.7).
// The download arrow opens the browser's download panel for the family (E22.3). While the
// family downloads or installs, a progress ring takes the arrow's 28 px slot. A family the
// last update check found a newer version of shows an "Update available" badge (E22.4).
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { useClient } from '../api/client';
import type { CatalogFamily, CatalogId } from '../api/types';
import { t } from '../i18n';
import { cardText } from '../list/grid';
import { styleCount } from '../list/rows';
import { WATERFALL_SIZES } from '../list/waterfall';
import { catalogFamily, useFaceLoader } from '../preview/faceLoader';
import { useCatalogsStore } from '../stores/catalogs';
import type { PreviewSettings } from '../stores/preview';
import ActivationDot from './ActivationDot.vue';
import UiBadge from './ui/UiBadge.vue';
import UiIconButton from './ui/UiIconButton.vue';

const props = defineProps<{
  catalog: CatalogId;
  family: CatalogFamily;
  text: string;
  preview: PreviewSettings;
  mode: 'list' | 'grid' | 'waterfall';
}>();

type LoadState = 'loading' | 'ready' | 'failed';

const catalogs = useCatalogsStore();
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
const update = computed(() => catalogs.updateOf(props.catalog, props.family.name));

/** The running download or install of this family: its ring takes the Download slot. */
const progress = computed(() => catalogs.progressOf(props.catalog, props.family.name));
const progressLabel = computed(() => {
  const name = props.family.name;
  return progress.value?.phase === 'scanning'
    ? t('catalogs.download.scanning', { name })
    : t('catalogs.download.downloading', { name });
});
/** A known count fills the arc; an unknown one (the download itself) shows a still quarter. */
const arcStyle = computed(() => {
  const p = progress.value;
  const share = p && p.total ? Math.min(1, (p.done ?? 0) / p.total) : 0.25;
  return { strokeDasharray: `${share * 100} 100` };
});
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
      <ActivationDot
        state="off"
        catalog
        :name="family.name"
        :disabled="catalogs.downloading"
        @activate="catalogs.askInstall(family)"
      />
      <bdi class="name">{{ family.name }}</bdi>
      <span v-if="mode !== 'grid'" class="detail">
        <span v-if="family.category">{{ family.category }}</span>
        <span>{{ styles }}</span>
        <UiBadge v-if="family.axes?.length">{{ t('list.variable') }}</UiBadge>
      </span>
      <UiBadge
        v-if="update"
        tone="accent"
        data-testid="update-available"
        :title="
          t('catalogs.updates.badgeTitle', {
            catalog: catalogs.current?.name ?? '',
            version: update.availableVersion ?? '',
          })
        "
        >{{ t('catalogs.updates.badge') }}</UiBadge
      >
      <span v-if="state === 'failed'" class="note">{{ t('list.previewUnavailable') }}</span>
      <span class="slot">
        <span
          v-if="progress"
          class="ring"
          role="progressbar"
          data-testid="catalog-progress"
          :aria-label="progressLabel"
          :aria-valuemin="progress.total !== null ? 0 : undefined"
          :aria-valuemax="progress.total ?? undefined"
          :aria-valuenow="progress.done ?? undefined"
          :title="progressLabel"
        >
          <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
            <circle class="track" cx="8" cy="8" r="6" />
            <circle class="arc" cx="8" cy="8" r="6" pathLength="100" :style="arcStyle" />
          </svg>
        </span>
        <UiIconButton
          v-else
          data-testid="catalog-download-open"
          icon="download"
          :label="t('catalogs.download.actionLabel', { name: family.name })"
          :disabled="catalogs.downloading"
          @click="catalogs.askDownload(family)"
        />
      </span>
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
  border-radius: var(--fm-radius-md);
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
  font-size: var(--fm-text-xs);
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
.note {
  color: var(--fm-warning);
}
/* The Download slot (E50.6): the 28 px icon button, or the progress ring in its place. */
.slot {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  margin-inline-start: auto;
}
.ring {
  display: inline-flex;
}
.ring svg {
  display: block;
}
.ring circle {
  fill: none;
  stroke-width: 2;
}
.ring .track {
  stroke: color-mix(in srgb, var(--fm-fg) 15%, transparent);
}
.ring .arc {
  stroke: var(--fm-accent);
  stroke-linecap: round;
  transform: rotate(-90deg);
  transform-origin: 50% 50%;
}
.line {
  display: flex;
  flex-direction: column;
  padding-bottom: 8px;
}
.size {
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  line-height: 14px;
}
.sample {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
