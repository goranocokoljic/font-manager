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
import { errorMessage, formatDate, t, type MessageKey } from '../i18n';
import { CARD, gridColumns, gridRange } from '../list/grid';
import { poolSize } from '../list/virtual';
import { waterfallRowPx } from '../list/waterfall';
import { sampleTextFor } from '../preview/sampleText';
import { CATALOG_VIEW, useCatalogsStore } from '../stores/catalogs';
import { listPreview, previewRowHeight, usePreviewStore } from '../stores/preview';
import { useViewsStore, VIEW_MODES, type ViewMode } from '../stores/views';
import AppToolbar from './AppToolbar.vue';
import CatalogDownload from './CatalogDownload.vue';
import CatalogFamilyItem from './CatalogFamilyItem.vue';
import CatalogInstall from './CatalogInstall.vue';
import PreviewControls from './PreviewControls.vue';
import SearchBox from './SearchBox.vue';
import UiButton from './ui/UiButton.vue';
import UiIconButton from './ui/UiIconButton.vue';
import type { SegmentOption } from './ui/UiSegmentedControl.vue';

const props = withDefaults(defineProps<{ overscan?: number }>(), { overscan: 2 });

const store = useCatalogsStore();
const previews = usePreviewStore();
const views = useViewsStore();

type CatalogMode = 'list' | 'grid' | 'waterfall';
/** The library's modes a catalog cannot show, and why (E51.4: off, never hidden). */
const MODE_OFF: Partial<Record<ViewMode, MessageKey>> = {
  table: 'catalogs.modeOff.table',
  layout: 'catalogs.modeOff.layout',
};
// The library's switch, in its order (E51.3); the label of each is `home.modes.<mode>`.
const modeOptions = computed<SegmentOption<ViewMode>[]>(() =>
  VIEW_MODES.map((m) => {
    const off = MODE_OFF[m];
    return { value: m, label: t(`home.modes.${m}`), icon: m, reason: off ? t(off) : undefined };
  }),
);
const mode = computed<CatalogMode>(() => {
  const m: ViewMode = views.modeFor(CATALOG_VIEW);
  return m === 'grid' || m === 'waterfall' ? m : 'list';
});

const catalog = computed(() => store.current);
/** The view's settings; the list and the cards set Paragraph at the body size (E51.4). */
const preview = computed(() => {
  const settings = previews.settingsFor(CATALOG_VIEW);
  return mode.value === 'waterfall' ? settings : listPreview(settings);
});
const textOf = (f: CatalogFamily) =>
  sampleTextFor(preview.value, f.name, (code) => previews.pangramText(code));

const dateOf = (at: string | null | undefined, options?: Intl.DateTimeFormatOptions) => {
  const ms = at ? Date.parse(at) : NaN;
  return Number.isFinite(ms) ? formatDate(ms, options) : null;
};
const refreshedAt = computed(() => dateOf(catalog.value?.refreshedAt));
const refreshedLabel = computed(() =>
  refreshedAt.value
    ? t('catalogs.refreshedAt', { date: refreshedAt.value })
    : t('catalogs.neverRefreshed'),
);
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
  return previewRowHeight(preview.value);
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
    <!-- The library's bar (E54.5): the same view switch, search and size slider at the same
         places. Table and Layout stay in the switch, off, with the reason. -->
    <AppToolbar
      :title="catalog.name"
      :title-id="`catalog-title-${catalog.id}`"
      :view="CATALOG_VIEW"
      :mode="mode"
      :modes="modeOptions"
      @update:mode="views.setMode(CATALOG_VIEW, $event)"
    >
      <template #search>
        <SearchBox
          data-testid="catalog-search"
          :model-value="store.query"
          :label="t('catalogs.searchLabel', { name: catalog.name })"
          :placeholder="t('catalogs.searchPlaceholder')"
          :disabled="!catalog.enabled"
          @update:model-value="store.setQuery($event)"
          @leave="viewport?.focus()"
        />
      </template>
      <div class="fill status">
        <p class="count" role="status">
          <template v-if="catalog.enabled && store.listed">{{
            t('home.families', { count: store.total })
          }}</template>
        </p>
        <!-- The date gives way first: it shortens, the full text in its tooltip, so it never
             pushes a button out of the bar in a long language. -->
        <span class="refreshed" data-testid="catalog-refreshed" :title="refreshedLabel">{{
          refreshedLabel
        }}</span>
        <UiIconButton
          icon="rescan"
          data-testid="catalog-refresh"
          :label="store.refreshing ? t('catalogs.refreshing') : t('catalogs.refresh')"
          :title="t('catalogs.refreshTitle', { name: catalog.name })"
          :disabled="!catalog.enabled || store.refreshing"
          @click="store.refresh()"
        />
        <UiIconButton
          icon="updateCheck"
          data-testid="catalog-check-updates"
          :label="store.checking ? t('catalogs.updates.checking') : t('catalogs.updates.check')"
          :title="t('catalogs.updates.checkTitle')"
          :disabled="!catalog.enabled || store.checking || store.refreshing"
          @click="store.checkUpdates()"
        />
      </div>
    </AppToolbar>
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
    <PreviewControls :view="CATALOG_VIEW" :mode="mode" />

    <div v-if="!catalog.enabled" class="empty" data-testid="catalog-off">
      <p>{{ t('catalogs.off', { name: catalog.name }) }}</p>
      <UiButton
        :aria-label="t('catalogs.turnOnLabel', { name: catalog.name })"
        aria-describedby="catalog-off-connects"
        :disabled="store.busy === catalog.id"
        @click="store.setEnabled(catalog.id, true)"
      >
        {{ t('catalogs.turnOn') }}
      </UiButton>
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
/* The catalog's row of the bar: the count, the date, Refresh and Check for updates. As tall
   as the library's filter row, so a bar wrapped to two rows is the same height in both (the
   28 px buttons stand 2 px into the bar's padding and gap). */
.status {
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
  height: 24px;
}
.count,
.refreshed {
  min-width: 0;
  margin: 0;
  overflow: hidden;
  color: var(--fm-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.count {
  flex: 0 1 auto;
}
/* The date gives way first (a far larger shrink weight); the count only once it is gone. */
.refreshed {
  flex: 0 1000 auto;
  margin-inline-start: auto;
  font-size: var(--fm-text-xs);
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
  background-color: var(--fm-bg);
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
  font-size: var(--fm-text-xs);
}
</style>
