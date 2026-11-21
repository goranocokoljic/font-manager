<script setup lang="ts">
// The Folders view of the Duplicates module (E56.12, board 14 "Folders"): the folder
// pairs of E56.6, largest saving first, as the API orders them. Each pair is a card with
// both folders: how many of its font files are in the other folder, what only it has,
// "Contained" or the share, and "Keep this side", which puts the pair into the draft plan
// and says in one line what that covers: the identical copies of the other side, nothing
// else. A side the API never removes (a source's folder, a system font folder, the
// library folder) says why and offers only to be kept. A pair opens to its files: what
// both have, the copies that are not identical, and what only one side has.
//
// The list is virtualized with fixed-height cards (list/virtual.ts). Keys: Tab moves
// through a card's buttons; ↑/↓, Page Up/Down, Home and End move from card to card;
// Esc closes an open pair.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { FolderPair, FolderPairFile, FolderPairSide, PairSide } from '../api/types';
import { errorMessage, formatNumber, t } from '../i18n';
import { fileSizeText } from '../list/columns';
import { poolSize, scrollToShow, windowRange } from '../list/virtual';
import {
  PAIR_GROUPS,
  pairShare,
  sharedFiles,
  useDuplicatesStore,
  type PairGroup,
} from '../stores/duplicates';
import UiButton from './ui/UiButton.vue';
import UiEmptyState from './ui/UiEmptyState.vue';
import UiIcon from './ui/UiIcon.vue';
import UiIconButton from './ui/UiIconButton.vue';

const props = defineProps<{ trash: string }>();

const store = useDuplicatesStore();

/** One card's slot: the card and the gap below it. */
const ROW_PX = 288;
const OVERSCAN = 2;
const viewport = ref<HTMLElement | null>(null);
const list = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const height = ref(800);
let observer: ResizeObserver | undefined;

/** The list's own scroll position: the viewport's, less the heading above the list. */
function measure(): void {
  const el = viewport.value;
  if (!el) return;
  scrollTop.value = Math.max(0, el.scrollTop - (list.value?.offsetTop ?? 0));
  if (el.clientHeight > 0) height.value = el.clientHeight;
}

const count = computed(() => store.pairTotal ?? 0);
const range = computed(() =>
  windowRange(scrollTop.value, height.value, ROW_PX, OVERSCAN, count.value),
);
const pool = computed(() => poolSize(height.value, ROW_PX, OVERSCAN));
const rows = computed(() => {
  const out: { index: number; slot: number; pair: FolderPair | undefined }[] = [];
  for (let i = range.value.start; i < range.value.end; i++)
    out.push({ index: i, slot: i % pool.value, pair: store.pairAt(i) });
  return out;
});
watch(range, (r) => {
  if (r.end > r.start) {
    void store.ensurePair(r.start);
    void store.ensurePair(r.end - 1);
  }
});

/** "1.4 GB", "612 MB": decimal units, as the file managers show them. */
const sizeText = (bytes: number) =>
  bytes >= 1_000_000_000
    ? formatNumber(bytes / 1_000_000_000, {
        style: 'unit',
        unit: 'gigabyte',
        maximumFractionDigits: 1,
      })
    : fileSizeText(bytes);

const nameOf = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;
const sideOf = (pair: FolderPair, side: PairSide) => (side === 'a' ? pair.a : pair.b);
const otherOf = (pair: FolderPair, side: PairSide) => (side === 'a' ? pair.b : pair.a);
const key = (pair: FolderPair) => `${pair.a.path}|${pair.b.path}`;

/** The card's title: "A is contained in B", or "A and B". */
function title(pair: FolderPair): string {
  if (pair.a.contained && !pair.b.contained)
    return t('duplicatesModule.folders.containedIn', {
      a: nameOf(pair.a.path),
      b: nameOf(pair.b.path),
    });
  if (pair.b.contained && !pair.a.contained)
    return t('duplicatesModule.folders.containedIn', {
      a: nameOf(pair.b.path),
      b: nameOf(pair.a.path),
    });
  return t('duplicatesModule.folders.and', { a: nameOf(pair.a.path), b: nameOf(pair.b.path) });
}
const contained = (pair: FolderPair) => pair.a.contained || pair.b.contained;
const share = (pair: FolderPair) =>
  t('duplicatesModule.folders.share', {
    share: formatNumber(pairShare(pair), { style: 'percent', maximumFractionDigits: 0 }),
  });
/** The fonts both folders have in another file: they stay in the queue of sets. */
const otherLevels = (pair: FolderPair) => {
  const side = pair.a.fontFiles <= pair.b.fontFiles ? pair.a : pair.b;
  return side.shared.sameVersion + side.shared.otherVersion;
};
const fill = (side: FolderPairSide) =>
  side.fontFiles ? Math.round((sharedFiles(side) / side.fontFiles) * 100) : 0;

const PROTECTED_KEY = {
  sourceRoot: 'duplicatesModule.folders.protected.sourceRoot',
  systemFolder: 'duplicatesModule.folders.protected.systemFolder',
  libraryFolder: 'duplicatesModule.folders.protected.libraryFolder',
} as const;

/** Whether "Keep this side" is offered: only when the other side may be removed. */
const canKeep = (pair: FolderPair, side: PairSide) => otherOf(pair, side).protected === null;
/** Whether the draft keeps this side of `pair`. */
const kept = (pair: FolderPair, side: PairSide) =>
  store.draftPair(pair)?.keep === sideOf(pair, side).path;

/** The one line that says what the pair in the plan covers, and nothing else. */
function covers(pair: FolderPair): string | null {
  const d = store.draftPair(pair);
  if (!d) return null;
  return t('duplicatesModule.folders.covers', {
    count: d.identical,
    size: sizeText(d.identicalSize),
    folder: d.remove,
    trash: props.trash,
  });
}

const full = ref(new Set<string>());
const fullKey = (pair: FolderPair, side: PairSide) => `${key(pair)}|${side}`;
function toggleFull(pair: FolderPair, side: PairSide): void {
  const next = new Set(full.value);
  const k = fullKey(pair, side);
  if (next.has(k)) next.delete(k);
  else next.add(k);
  full.value = next;
}

const busy = ref(false);
async function keep(pair: FolderPair, side: PairSide, from?: HTMLElement): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  try {
    if (kept(pair, side)) await store.takeBackPair(pair.a.path, pair.b.path);
    else await store.keepSide(pair, side);
  } finally {
    busy.value = false;
  }
  // The button stays where it was: give it the focus back after the store refresh.
  await nextTick();
  from?.focus();
}
async function takeBack(pair: FolderPair, card: HTMLElement | null): Promise<void> {
  await store.takeBackPair(pair.a.path, pair.b.path);
  await nextTick();
  card?.querySelector<HTMLElement>('[data-keep]')?.focus();
}

// Moving from card to card with the keys.
const cardEl = (index: number) =>
  list.value?.querySelector<HTMLElement>(`[data-index="${index}"]`) ?? null;
async function focusCard(index: number): Promise<void> {
  const el = viewport.value;
  const to = Math.max(0, Math.min(index, count.value - 1));
  if (el) {
    const top = list.value?.offsetTop ?? 0;
    el.scrollTop = scrollToShow(to, ROW_PX, Math.max(0, el.scrollTop - top), el.clientHeight) + top;
    measure();
  }
  await store.ensurePair(to);
  await nextTick();
  cardEl(to)?.focus();
}
function onListKey(e: KeyboardEvent): void {
  const card = (e.target as HTMLElement).closest<HTMLElement>('[data-index]');
  if (!card) return;
  const index = Number(card.dataset.index);
  const page = Math.max(1, Math.floor(height.value / ROW_PX));
  const to =
    e.key === 'ArrowDown'
      ? index + 1
      : e.key === 'ArrowUp'
        ? index - 1
        : e.key === 'PageDown'
          ? index + page
          : e.key === 'PageUp'
            ? index - page
            : e.key === 'Home'
              ? 0
              : e.key === 'End'
                ? count.value - 1
                : null;
  if (to === null) return;
  e.preventDefault();
  void focusCard(to);
}

// An open pair.
const openedFrom = ref<number | null>(null);
const detail = ref<HTMLElement | null>(null);
async function openPair(pair: FolderPair, index: number, group?: PairGroup): Promise<void> {
  openedFrom.value = index;
  await store.showPair(pair);
  await nextTick();
  const target = group
    ? detail.value?.querySelector<HTMLElement>(`#pair-files-${group}`)
    : detail.value?.querySelector<HTMLElement>('h2');
  target?.focus();
}
async function closePair(): Promise<void> {
  const index = openedFrom.value ?? 0;
  await store.showPair(null);
  await nextTick();
  measure();
  await focusCard(index);
}
function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' && store.openPair && !e.defaultPrevented) {
    e.preventDefault();
    void closePair();
  }
}

const groupTitle = (group: PairGroup, pair: FolderPair) =>
  t(`duplicatesModule.folders.files.${group}`, {
    a: nameOf(pair.a.path),
    b: nameOf(pair.b.path),
    count: store.pairFiles[group]?.total ?? 0,
  });
const levelText = (f: FolderPairFile) =>
  f.level ? t(`duplicatesModule.queue.level.${f.level}`) : '';

onMounted(async () => {
  observer = new ResizeObserver(measure);
  if (viewport.value) observer.observe(viewport.value);
  measure();
  await store.reloadPairs();
});
onBeforeUnmount(() => {
  observer?.disconnect();
  void store.showPair(null);
});
</script>

<template>
  <div
    ref="viewport"
    class="folders"
    data-testid="duplicates-folders"
    @scroll.passive="measure"
    @keydown="onKey"
  >
    <!-- One pair, opened -->
    <div v-if="store.openPair" ref="detail" class="page" data-testid="folder-pair-detail">
      <UiButton class="back" @click="closePair">
        <UiIcon name="chevronBack" />{{ t('duplicatesModule.folders.backToPairs') }}
      </UiButton>
      <h2 class="heading" tabindex="-1">{{ title(store.openPair) }}</h2>
      <p class="lead">
        <bdi>{{ store.openPair.a.path }}</bdi>
        <span class="muted"> · </span>
        <bdi>{{ store.openPair.b.path }}</bdi>
      </p>
      <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
      <section
        v-for="g in PAIR_GROUPS"
        :key="g"
        class="group"
        :aria-labelledby="`pair-files-${g}`"
        :data-group="g"
      >
        <h3 :id="`pair-files-${g}`" class="section-label" tabindex="-1">
          {{ groupTitle(g, store.openPair) }}
        </h3>
        <ul v-if="store.pairFiles[g]?.items.length" class="files">
          <li v-for="f in store.pairFiles[g]!.items" :key="f.path" class="file">
            <code class="path wrap"
              ><bdi>{{ f.path }}</bdi></code
            >
            <span v-if="f.copy" class="muted small">
              {{ levelText(f) }} ·
              <code class="path wrap"
                ><bdi>{{ f.copy }}</bdi></code
              >
            </span>
            <span class="muted small">{{ fileSizeText(f.size) }}</span>
          </li>
        </ul>
        <p v-else-if="store.pairFiles[g]" class="muted small">
          {{ t('duplicatesModule.folders.files.none') }}
        </p>
        <UiButton
          v-if="(store.pairFiles[g]?.items.length ?? 0) < (store.pairFiles[g]?.total ?? 0)"
          class="outlined"
          @click="store.morePairFiles(g)"
        >
          {{ t('duplicatesModule.folders.files.more') }}
        </UiButton>
      </section>
    </div>

    <!-- The pairs -->
    <div v-else class="page">
      <h2 class="heading">{{ t('duplicatesModule.folders.heading') }}</h2>
      <p class="lead">{{ t('duplicatesModule.folders.lead') }}</p>
      <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
      <ul
        v-if="count > 0"
        ref="list"
        class="pairs"
        :aria-label="t('duplicatesModule.folders.listLabel')"
        :style="{ height: `${count * ROW_PX}px` }"
        @keydown="onListKey"
      >
        <li
          v-for="r in rows"
          :key="r.slot"
          class="slot"
          :style="{ transform: `translateY(${r.index * ROW_PX}px)` }"
        >
          <article
            v-if="r.pair"
            class="card"
            tabindex="-1"
            :data-index="r.index"
            :aria-labelledby="`pair-title-${r.index}`"
            data-testid="folder-pair"
          >
            <header class="card-head">
              <h3 :id="`pair-title-${r.index}`" class="card-title">{{ title(r.pair) }}</h3>
              <span class="chip" data-testid="pair-share">
                {{ contained(r.pair) ? t('duplicatesModule.folders.contained') : share(r.pair) }}
              </span>
              <span class="muted small size">
                {{ t('duplicatesModule.folders.frees', { size: sizeText(r.pair.frees) }) }}
              </span>
              <UiButton
                class="outlined"
                :aria-label="t('duplicatesModule.folders.openName', { name: title(r.pair) })"
                @click="openPair(r.pair, r.index)"
              >
                {{ t('duplicatesModule.folders.open') }}
              </UiButton>
            </header>
            <div class="sides">
              <div
                v-for="s in ['a', 'b'] as const"
                :key="s"
                class="side"
                :data-side="s"
                :data-protected="sideOf(r.pair, s).protected ?? undefined"
              >
                <div class="path-row">
                  <code
                    class="path"
                    :class="{ wrap: full.has(fullKey(r.pair, s)) }"
                    data-testid="pair-path"
                    ><bdi>{{ sideOf(r.pair, s).path }}</bdi></code
                  >
                  <UiIconButton
                    size="sm"
                    :icon="full.has(fullKey(r.pair, s)) ? 'chevronUp' : 'chevronDown'"
                    :label="
                      full.has(fullKey(r.pair, s))
                        ? t('duplicatesModule.folders.shortPath')
                        : t('duplicatesModule.folders.fullPath')
                    "
                    :aria-pressed="full.has(fullKey(r.pair, s))"
                    @click="toggleFull(r.pair, s)"
                  />
                  <UiIconButton
                    size="sm"
                    icon="folderOpen"
                    :label="t('duplicatesModule.folders.show')"
                    @click="store.showPairFolder(r.pair, s)"
                  />
                </div>
                <div class="bar" aria-hidden="true">
                  <span :style="{ inlineSize: `${fill(sideOf(r.pair, s))}%` }" />
                </div>
                <p class="counts small">
                  {{
                    t('duplicatesModule.folders.inOther', {
                      shared: formatNumber(sharedFiles(sideOf(r.pair, s))),
                      count: sideOf(r.pair, s).fontFiles,
                    })
                  }}
                  <a
                    v-if="sideOf(r.pair, s).onlyHere > 0"
                    href="#"
                    class="link"
                    @click.prevent="openPair(r.pair, r.index, s === 'a' ? 'onlyA' : 'onlyB')"
                  >
                    {{
                      t('duplicatesModule.folders.onlyHere', {
                        count: sideOf(r.pair, s).onlyHere,
                      })
                    }}
                  </a>
                </p>
                <div class="side-action">
                  <UiButton
                    v-if="canKeep(r.pair, s)"
                    class="outlined keep"
                    data-keep
                    :aria-pressed="kept(r.pair, s)"
                    :aria-label="
                      t('duplicatesModule.folders.keepName', { name: sideOf(r.pair, s).path })
                    "
                    @click="keep(r.pair, s, $event.currentTarget as HTMLElement)"
                  >
                    {{ t('duplicatesModule.folders.keep') }}
                  </UiButton>
                  <span
                    v-if="sideOf(r.pair, s).protected"
                    class="muted small protected"
                    data-testid="pair-protected"
                  >
                    <UiIcon name="lock" />
                    {{ t(PROTECTED_KEY[sideOf(r.pair, s).protected!]) }}
                  </span>
                </div>
              </div>
            </div>
            <p v-if="covers(r.pair)" class="covers small" data-testid="pair-covers">
              <span>{{ covers(r.pair) }}</span>
              <UiButton
                :aria-label="t('duplicatesModule.folders.takeBackName', { name: title(r.pair) })"
                @click="takeBack(r.pair, ($event.currentTarget as HTMLElement).closest('article'))"
              >
                {{ t('duplicatesModule.plan.takeBack') }}
              </UiButton>
            </p>
            <p v-else-if="otherLevels(r.pair) > 0" class="muted small note">
              {{ t('duplicatesModule.folders.otherLevels', { count: otherLevels(r.pair) }) }}
            </p>
          </article>
          <div v-else class="card placeholder" aria-hidden="true" />
        </li>
      </ul>
      <UiEmptyState
        v-else-if="store.pairTotal !== null"
        :title="t('duplicatesModule.folders.empty')"
      />
    </div>
  </div>
</template>

<style scoped>
.folders {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
}
.page {
  max-width: 1040px;
  padding-block: var(--fm-space-6);
  padding-inline: var(--fm-space-8);
}
.back {
  margin-bottom: var(--fm-space-3);
}
.heading {
  margin: 0 0 var(--fm-space-2);
  font-size: var(--fm-text-xl);
}
.lead {
  max-width: 720px;
  margin: 0 0 var(--fm-space-5);
  color: var(--fm-muted);
  overflow-wrap: anywhere;
}
.muted {
  color: var(--fm-muted);
}
.small {
  font-size: var(--fm-text-sm);
}
.error {
  color: var(--fm-danger);
}
.pairs {
  position: relative;
  margin: 0;
  padding: 0;
  list-style: none;
}
.slot {
  position: absolute;
  inset-block-start: 0;
  inset-inline: 0;
  box-sizing: border-box;
  height: 288px;
  padding-bottom: var(--fm-space-3);
}
.card {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
  height: 100%;
  /* A path shown in full can be longer than the card's fixed slot: it scrolls, never overlaps. */
  overflow-y: auto;
  padding: var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bg);
}
.card:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.placeholder {
  background-color: var(--fm-surface);
}
.card-head {
  display: flex;
  align-items: center;
  gap: var(--fm-space-3);
  min-width: 0;
}
.card-title {
  overflow: hidden;
  margin: 0;
  font-size: var(--fm-text-md, inherit);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chip {
  flex: none;
  padding-block: 2px;
  padding-inline: var(--fm-space-2);
  border-radius: 999px;
  background-color: var(--fm-surface);
  font-size: var(--fm-text-xs);
}
.size {
  flex: none;
  margin-inline-start: auto;
}
.sides {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--fm-space-3);
}
.side {
  display: grid;
  gap: var(--fm-space-2);
  min-width: 0;
  padding: var(--fm-space-3);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
}
.path-row {
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
  min-width: 0;
}
.path {
  overflow: hidden;
  min-width: 0;
  flex: 1;
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.path.wrap {
  overflow: visible;
  overflow-wrap: anywhere;
  white-space: normal;
}
.bar {
  height: 4px;
  overflow: hidden;
  border-radius: 2px;
  background-color: var(--fm-border);
}
.bar > span {
  display: block;
  height: 100%;
  background-color: var(--fm-fg);
}
.counts {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-2);
  margin: 0;
  color: var(--fm-muted);
}
.link {
  color: var(--fm-fg);
  text-decoration: underline;
  text-underline-offset: 2px;
}
.side-action {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2);
  min-height: 32px;
}
.protected {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-1);
}
.outlined {
  height: 32px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  color: var(--fm-fg);
}
.keep[aria-pressed='true'] {
  border-color: var(--fm-accent);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
}
.covers,
.note {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  margin: 0;
}
.covers > span {
  overflow-wrap: anywhere;
}
.group {
  margin-bottom: var(--fm-space-5);
}
.section-label {
  margin: 0 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.files {
  margin: 0 0 var(--fm-space-2);
  padding: 0;
  list-style: none;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
}
.files > li + li {
  border-top: 1px solid var(--fm-border);
}
.file {
  display: grid;
  gap: 2px;
  padding-block: var(--fm-space-2);
  padding-inline: var(--fm-space-3);
}
</style>
