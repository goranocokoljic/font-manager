<script setup lang="ts">
// An online catalog in the center (E22.2): its families from `list_catalog_families`
// with the font list's preview controls (sample text, size, …) and its List, Grid and
// Waterfall modes, virtualized the same way: only the lines on screen (plus `overscan`)
// exist, keyed by slot. The search box filters through the core; "Refresh" fetches the
// list again. Offline, or when a refresh fails, the cached list stays with a note. A row's
// "Download..." opens the download panel above the list (E22.3). "Check for updates" asks the
// core which downloaded families have a newer version (E22.4); a Refresh asks too. A row's
// "Install" downloads and activates the family in one call; its progress line is above the
// list too (E22.7).
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { CatalogFamily } from '../api/types';
import { errorMessage, formatDate, t } from '../i18n';
import { CARD, gridColumns, gridRange } from '../list/grid';
import { poolSize } from '../list/virtual';
import { waterfallRowPx } from '../list/waterfall';
import { sampleTextFor } from '../preview/sampleText';
import { CATALOG_VIEW, useCatalogsStore } from '../stores/catalogs';
import { previewRowHeight, usePreviewStore } from '../stores/preview';
import { useSettingsStore } from '../stores/settings';
import { useViewsStore, type ViewMode } from '../stores/views';
import CatalogDownload from './CatalogDownload.vue';
import CatalogFamilyItem from './CatalogFamilyItem.vue';
import CatalogInstall from './CatalogInstall.vue';
import PreviewControls from './PreviewControls.vue';

const props = withDefaults(defineProps<{ overscan?: number }>(), { overscan: 2 });

const store = useCatalogsStore();
const previews = usePreviewStore();
const settings = useSettingsStore();
const views = useViewsStore();

type CatalogMode = 'list' | 'grid' | 'waterfall';
// The label of each is `home.modes.<mode>`.
const MODES: readonly CatalogMode[] = ['list', 'grid', 'waterfall'];
const mode = computed<CatalogMode>(() => {
  const m: ViewMode = views.modeFor(CATALOG_VIEW);
  return m === 'grid' || m === 'waterfall' ? m : 'list';
});

const catalog = computed(() => store.current);
const preview = computed(() => previews.settingsFor(CATALOG_VIEW));
const textOf = (f: CatalogFamily) =>
  sampleTextFor(preview.value, f.name, (code) => previews.pangramText(code));

const dateOf = (at: string | null | undefined, options?: Intl.DateTimeFormatOptions) => {
  const ms = at ? Date.parse(at) : NaN;
  return Number.isFinite(ms) ? formatDate(ms, options) : null;
};
const refreshedAt = computed(() => dateOf(catalog.value?.refreshedAt));
const offlineNote = computed(() => {
  const date = dateOf(catalog.value?.refreshedAt, { dateStyle: 'medium' });
  return store.offline && date ? t('catalogs.offline', { date }) : null;
});
/** What the last update check found, as one line; `null` before the first one. */
const updatesNote = computed(() => {
  const found = store.updates;
  if (!found) return null;
  const count = found.filter((u) => u.status === 'updateAvailable').length;
  const gone = found.filter((u) => u.status === 'noLongerListed').map((u) => u.family);
  const parts = [t('catalogs.updates.found', { count })];
  if (gone.length) parts.push(t('catalogs.updates.noLongerListed', { names: gone.join(', ') }));
  return parts.join(' ');
});
/** A failed refresh that is not "offline" (or with no list to fall back on). */
const refreshFailure = computed(() =>
  store.refreshError && !offlineNote.value ? errorMessage(store.refreshError) : null,
);

// ---- the virtualized body ----

/** Viewport size before the first measurement (and where there is no layout). */
const FALLBACK = { width: 800, height: 600 };
const viewport = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const height = ref(FALLBACK.height);
const width = ref(FALLBACK.width);
let observer: ResizeObserver | undefined;

function measure(): void {
  const el = viewport.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  if (el.clientHeight > 0) height.value = el.clientHeight;
  if (el.clientWidth > 0) width.value = el.clientWidth;
}
watch(viewport, (el, before) => {
  if (before) observer?.unobserve(before);
  if (!el) return;
  measure();
  if (typeof ResizeObserver !== 'undefined') {
    observer ??= new ResizeObserver(measure);
    observer.observe(el);
  }
});
onMounted(measure);
onBeforeUnmount(() => observer?.disconnect());

const perLine = computed(() => (mode.value === 'grid' ? gridColumns(width.value) : 1));
const linePx = computed(() => {
  if (mode.value === 'grid') return CARD.linePx;
  if (mode.value === 'waterfall') return waterfallRowPx(preview.value.lineHeight);
  return previewRowHeight(preview.value, settings.density);
});
const lines = computed(() => Math.ceil(store.total / perLine.value));
const range = computed(() =>
  gridRange(
    scrollTop.value,
    height.value,
    linePx.value,
    props.overscan,
    store.total,
    perLine.value,
  ),
);
const pool = computed(() => poolSize(height.value, linePx.value, props.overscan) * perLine.value);
watch(range, (r) => store.want(r.start, r.end), { immediate: true });

const visible = computed(() => {
  const out: {
    index: number;
    slot: number;
    family: CatalogFamily;
    style: Record<string, string | undefined>;
  }[] = [];
  const cardPx = width.value / perLine.value;
  for (let index = range.value.start; index < range.value.end; index++) {
    const family = store.families[index];
    if (!family) continue;
    const line = Math.floor(index / perLine.value);
    const column = index % perLine.value;
    out.push({
      index,
      slot: index % pool.value,
      family,
      style: {
        height: `${linePx.value}px`,
        width: perLine.value > 1 ? `${cardPx}px` : undefined,
        // A logical side, so the grid's columns run from the right in a right-to-left UI.
        insetInlineStart: `${column * cardPx}px`,
        transform: `translateY(${line * linePx.value}px)`,
      },
    });
  }
  return out;
});

// A new search or a new mode starts at the top.
watch([() => store.query, mode], () => {
  if (viewport.value) viewport.value.scrollTop = 0;
  scrollTop.value = 0;
});

/** A closed download panel gives the focus back to the list. */
function downloadClosed(): void {
  viewport.value?.focus();
}

const surfaceClass = computed(() =>
  preview.value.surface === 'auto' ? undefined : `surface-${preview.value.surface}`,
);
</script>

<template>
  <section v-if="catalog" class="browser" :aria-labelledby="`catalog-title-${catalog.id}`">
    <header class="toolbar">
      <h1 :id="`catalog-title-${catalog.id}`" class="title">
        <bdi>{{ catalog.name }}</bdi>
      </h1>
      <input
        type="search"
        class="search"
        data-testid="catalog-search"
        :aria-label="t('catalogs.searchLabel', { name: catalog.name })"
        :placeholder="t('catalogs.searchPlaceholder')"
        autocomplete="off"
        spellcheck="false"
        :disabled="!catalog.enabled"
        :value="store.query"
        @input="store.setQuery(($event.target as HTMLInputElement).value)"
        @keydown.esc.prevent="store.setQuery('')"
      />
      <p class="count" role="status">
        <template v-if="catalog.enabled && store.listed">{{
          t('home.families', { count: store.total })
        }}</template>
      </p>
      <div class="modes" role="group" :aria-label="t('home.viewMode')">
        <button
          v-for="m in MODES"
          :key="m"
          type="button"
          class="toggle"
          :aria-pressed="mode === m"
          @click="views.setMode(CATALOG_VIEW, m)"
        >
          {{ t(`home.modes.${m}`) }}
        </button>
      </div>
      <span class="refreshed" data-testid="catalog-refreshed">{{
        refreshedAt
          ? t('catalogs.refreshedAt', { date: refreshedAt })
          : t('catalogs.neverRefreshed')
      }}</span>
      <button
        type="button"
        class="toggle"
        data-testid="catalog-refresh"
        :title="t('catalogs.refreshTitle', { name: catalog.name })"
        :disabled="!catalog.enabled || store.refreshing"
        @click="store.refresh()"
      >
        {{ store.refreshing ? t('catalogs.refreshing') : t('catalogs.refresh') }}
      </button>
      <button
        type="button"
        class="toggle"
        data-testid="catalog-check-updates"
        :title="t('catalogs.updates.checkTitle')"
        :disabled="!catalog.enabled || store.checking || store.refreshing"
        @click="store.checkUpdates()"
      >
        {{ store.checking ? t('catalogs.updates.checking') : t('catalogs.updates.check') }}
      </button>
    </header>
    <p v-if="store.checkError" class="banner error" role="alert" data-testid="catalog-updates">
      {{ errorMessage(store.checkError) }}
    </p>
    <p v-else-if="updatesNote" class="banner" role="status" data-testid="catalog-updates">
      {{ updatesNote }}
    </p>
    <p v-if="offlineNote" class="banner" role="status" data-testid="catalog-offline">
      {{ offlineNote }}
    </p>
    <p v-else-if="refreshFailure" class="banner error" role="alert">{{ refreshFailure }}</p>
    <CatalogDownload
      v-if="store.download && store.download.catalog === catalog.id"
      @close="downloadClosed"
    />
    <CatalogInstall v-if="store.install && store.install.catalog === catalog.id" />
    <PreviewControls :view="CATALOG_VIEW" />

    <div v-if="!catalog.enabled" class="empty" data-testid="catalog-off">
      <p>{{ t('catalogs.off', { name: catalog.name }) }}</p>
      <button
        type="button"
        class="toggle"
        :aria-label="t('catalogs.turnOnLabel', { name: catalog.name })"
        aria-describedby="catalog-off-connects"
        :disabled="store.busy === catalog.id"
        @click="store.setEnabled(catalog.id, true)"
      >
        {{ t('catalogs.turnOn') }}
      </button>
      <p id="catalog-off-connects" class="hint">{{ t('catalogs.connects') }}</p>
    </div>
    <div
      v-else-if="store.total > 0"
      ref="viewport"
      class="viewport"
      :class="surfaceClass"
      :style="{ backgroundColor: preview.backgroundColor ?? undefined }"
      role="list"
      tabindex="0"
      data-testid="catalog-list"
      :aria-label="t('catalogs.listLabel', { name: catalog.name })"
      @scroll="measure"
    >
      <div class="spacer" role="none" :style="{ height: `${lines * linePx}px` }">
        <div
          v-for="item in visible"
          :key="item.slot"
          class="slot"
          :class="mode"
          role="listitem"
          :aria-label="item.family.name"
          :aria-setsize="store.total"
          :aria-posinset="item.index + 1"
          :style="item.style"
        >
          <CatalogFamilyItem
            :catalog="catalog.id"
            :family="item.family"
            :text="textOf(item.family)"
            :preview="preview"
            :mode="mode"
          />
        </div>
      </div>
    </div>
    <section v-else class="empty" :aria-label="t('catalogs.listLabel', { name: catalog.name })">
      <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
      <p v-else-if="store.listed && store.query">{{ t('catalogs.noMatches') }}</p>
      <p v-else-if="store.listed && !catalog.refreshedAt">{{ t('catalogs.empty') }}</p>
    </section>
  </section>
</template>

<style scoped>
.browser {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}
/* Wraps onto a second line in a narrow center (the inspector open below ~1,280 px,
   a longer language), so Refresh and the date never fall outside the column. */
.toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  min-height: 48px;
  box-sizing: border-box;
  padding: 6px 16px;
  border-bottom: 1px solid var(--fm-border);
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.title,
.count,
.refreshed {
  white-space: nowrap;
}
.search {
  flex: 0 1 220px;
  width: 220px;
  min-width: 120px;
  padding: 4px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.count {
  flex: 1;
  margin: 0;
  color: var(--fm-muted);
}
.modes {
  display: flex;
  gap: 4px;
}
.refreshed {
  color: var(--fm-muted);
  font-size: 11.5px;
}
.toggle {
  padding: 4px 8px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.toggle:disabled {
  color: var(--fm-muted);
}
.toggle[aria-pressed='true'] {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
.banner {
  margin: 0;
  padding: 6px 16px;
  border-bottom: 1px solid var(--fm-border);
  color: var(--fm-warning);
}
.banner.error,
.error {
  color: var(--fm-danger, var(--fm-warning));
}
.viewport {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  contain: strict;
  background: var(--fm-bg);
  color: var(--fm-fg);
}
.viewport:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.spacer {
  position: relative;
}
.slot {
  position: absolute;
  top: 0;
  inset-inline: 0;
  box-sizing: border-box;
  contain: layout paint;
}
.slot.list,
.slot.waterfall {
  border-bottom: 1px solid var(--fm-border);
}
.slot.grid {
  inset-inline-end: auto;
}
.empty {
  display: grid;
  flex: 1;
  place-content: center;
  justify-items: center;
  gap: 8px;
  color: var(--fm-muted);
}
.empty p {
  margin: 0;
}
.hint {
  font-size: 11.5px;
}
</style>
