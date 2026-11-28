<script setup lang="ts">
// The Duplicates module (E56.11, board 14): a full view of its own, like Studio. A
// 232 px navigation on the surface colour ("Library" to go back, then Overview,
// Folders, Sets, Plan, History, Hidden copies and Remembered decisions with their counts, and at its foot
// the languages the user works in, the link to edit them under Settings › Languages (E58.2)
// and the link to the rules, E56.14); the section is in the
// URL (`#/duplicates?section=sets`). The overview leads with the identical copies as
// one decision, then the levels still to decide, then the top folder pairs. Every
// number comes from the API; nothing moves until a plan is applied. The Plan, History,
// Hidden copies and Remembered decisions are E56.15's; `#/duplicates?family=<id>` opens the
// queue on the sets of one family (the "N copies" mark of a wall card or the family page).
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import type { DuplicateLevel, FolderPair } from '../api/types';
import DuplicatesFolders from '../components/DuplicatesFolders.vue';
import DuplicatesHistory from '../components/DuplicatesHistory.vue';
import DuplicatesKept from '../components/DuplicatesKept.vue';
import DuplicatesPlan from '../components/DuplicatesPlan.vue';
import DuplicatesQueue from '../components/DuplicatesQueue.vue';
import DuplicatesRules from '../components/DuplicatesRules.vue';
import UiButton from '../components/ui/UiButton.vue';
import UiEmptyState from '../components/ui/UiEmptyState.vue';
import UiIcon from '../components/ui/UiIcon.vue';
import { errorMessage, formatNumber, t, type MessageKey } from '../i18n';
import { fileSizeText } from '../list/columns';
import { fadeIn } from '../motion/fadeIn';
import { useAppStore } from '../stores/app';
import { useCleanupStore } from '../stores/cleanup';
import { useCommandsStore } from '../stores/commands';
import { useDuplicateSummaryStore } from '../stores/duplicateSummary';
import { useDuplicatesStore } from '../stores/duplicates';
import { useSettingsStore } from '../stores/settings';

const store = useDuplicatesStore();
const cleanup = useCleanupStore();
const summary = useDuplicateSummaryStore();
const settings = useSettingsStore();
const app = useAppStore();
const commands = useCommandsStore();
const route = useRoute();
const router = useRouter();

const SECTIONS = ['overview', 'folders', 'sets', 'plan', 'history', 'hidden', 'decisions'] as const;
type Section = (typeof SECTIONS)[number] | 'rules';
const section = computed<Section>(() => {
  const s = route.query.section;
  return s === 'rules' || SECTIONS.includes(s as never) ? (s as Section) : 'overview';
});

const main = ref<HTMLElement | null>(null);
async function open(s: Section, focusNav = false): Promise<void> {
  await router.replace({ query: { ...route.query, section: s } });
  await nextTick();
  if (focusNav) document.getElementById(`dup-nav-${s}`)?.focus();
}
watch(section, async () => {
  await nextTick();
  fadeIn(main.value, 'fast');
});

function back(): void {
  void router.push({ name: 'home' });
}
function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' && !commands.open && !e.defaultPrevented) {
    e.preventDefault();
    back();
  }
}

/** The name of this system's trash, as the board says it. */
const trash = computed(() =>
  app.info?.platform === 'windows'
    ? t('duplicatesModule.trash.windows')
    : t('duplicatesModule.trash.unix'),
);

/** "1.4 GB", "612 MB": decimal units, as the file managers show them. */
const sizeText = (bytes: number) =>
  bytes >= 1_000_000_000
    ? formatNumber(bytes / 1_000_000_000, {
        style: 'unit',
        unit: 'gigabyte',
        maximumFractionDigits: 1,
      })
    : fileSizeText(bytes);

/** Where the languages are edited (E58.2): Settings › Languages. */
const LANGUAGE_SETTINGS = { name: 'settings', query: { section: 'languages' } } as const;

const o = computed(() => store.overview);
const myLanguages = computed(() => settings.settings?.myLanguages ?? []);
const languagePresets = computed(() =>
  myLanguages.value
    .map((code) => store.presets.find((p) => p.code === code))
    .filter((p) => p != null),
);
const languageNames = computed(() => languagePresets.value.map((p) => p.name).join(', '));

const counts = computed<Record<Section, number | null>>(() => ({
  overview: null,
  folders: store.pairTotal,
  sets: store.total,
  plan: store.planned,
  history: cleanup.sessions?.length ?? store.sessions,
  hidden: null,
  decisions: null,
  rules: null,
}));

// Overview.
const levels = computed(() => {
  const v = o.value;
  if (!v) return [];
  const row = (level: DuplicateLevel | 'keepBoth', sets: number, files: number | null) => ({
    level,
    title: t(`duplicatesModule.overview.${level}` as MessageKey),
    body: t(`duplicatesModule.overview.${level}Body` as MessageKey),
    fonts: t('duplicatesModule.overview.fonts', { count: sets }),
    value:
      files === null
        ? t('duplicatesModule.overview.protected')
        : t('duplicatesModule.overview.fileCount', { count: files }),
    sets,
  });
  return [
    row('otherVersion', v.otherVersion.sets, v.otherVersion.files),
    row('sameVersion', v.sameVersion.sets, v.sameVersion.files),
    row('keepBoth', v.keepBoth, null),
  ].filter((r) => r.sets > 0);
});
async function openLevel(level: DuplicateLevel | 'keepBoth'): Promise<void> {
  await store.setFilter(
    level === 'keepBoth'
      ? { level: null, recommendation: 'keepBoth' }
      : { level, recommendation: null },
  );
  await open('sets');
}
async function reviewIdentical(): Promise<void> {
  await store.includeIdentical(true);
  await open('plan');
}
const pairLine = (p: FolderPair) => {
  const [small, big] = p.a.fontFiles <= p.b.fontFiles ? [p.a, p.b] : [p.b, p.a];
  return {
    path: small.path,
    other: big.path,
    relation: small.contained
      ? t('duplicatesModule.overview.contained')
      : t('duplicatesModule.overview.shares'),
    shared: t('duplicatesModule.overview.shared', {
      shared: formatNumber(
        small.shared.identical + small.shared.sameVersion + small.shared.otherVersion,
      ),
      count: small.fontFiles,
    }),
    size: sizeText(small.sharedSize),
  };
};

/** The sets of the family in `?family=`, from the summary; null without one. */
const familySets = computed(() => {
  const id = Number(route.query.family);
  return Number.isInteger(id) && route.query.family != null ? (summary.of(id)?.sets ?? []) : null;
});
async function showFamily(): Promise<void> {
  const ids = familySets.value;
  if (ids === null) return;
  await store.setFilter({ ids, level: null, recommendation: null });
  if (section.value !== 'sets') await open('sets');
}
watch(familySets, (now, before) => {
  if (now && JSON.stringify(now) !== JSON.stringify(before)) void showFamily();
});

onMounted(async () => {
  document.getElementById(`dup-nav-${section.value}`)?.focus();
  fadeIn(main.value, 'move');
  await Promise.all([
    settings.settings ? undefined : settings.load(),
    app.info ? undefined : app.load(),
    store.load(),
    cleanup.start(),
    summary.start(),
    // Fresh each time the module opens: the library may have changed since.
    familySets.value === null ? store.reload(store.cursor) : showFamily(),
  ]);
});
</script>

<template>
  <div class="duplicates" @keydown="onKey">
    <nav class="nav" :aria-label="t('duplicatesModule.navLabel')">
      <UiButton
        class="back"
        :title="t('duplicatesModule.backTitle')"
        aria-keyshortcuts="Escape"
        @click="back"
      >
        <UiIcon name="chevronBack" />{{ t('duplicatesModule.back') }}
      </UiButton>
      <h1 class="title">{{ t('duplicatesModule.title') }}</h1>
      <ul class="items">
        <li v-for="s in SECTIONS" :key="s" :class="{ gap: s === 'plan' }">
          <button
            :id="`dup-nav-${s}`"
            type="button"
            class="item"
            :aria-current="section === s ? 'page' : undefined"
            @click="open(s)"
          >
            <span>{{ t(`duplicatesModule.nav.${s}`) }}</span>
            <span v-if="counts[s] != null" class="count" :data-testid="`dup-count-${s}`">
              {{ formatNumber(counts[s] ?? 0) }}
            </span>
          </button>
        </li>
      </ul>
      <div class="foot">
        <h2 class="foot-heading">{{ t('duplicatesModule.languages.heading') }}</h2>
        <p class="foot-languages" data-testid="dup-languages">
          {{ languageNames || t('duplicatesModule.languages.none') }}
        </p>
        <RouterLink :to="LANGUAGE_SETTINGS" class="link" data-testid="dup-change-languages">
          {{ t('duplicatesModule.languages.link') }}
        </RouterLink>
        <a
          href="#"
          class="link"
          :aria-current="section === 'rules' ? 'page' : undefined"
          data-testid="dup-open-rules"
          @click.prevent="open('rules')"
        >
          {{ t('duplicatesModule.languages.rules') }}
        </a>
      </div>
    </nav>

    <main ref="main" class="main" :class="{ wide: section === 'sets' || section === 'folders' }">
      <p
        v-if="store.error && section !== 'sets' && section !== 'folders'"
        role="alert"
        class="error"
      >
        {{ errorMessage(store.error) }}
      </p>

      <!-- Overview -->
      <div v-if="section === 'overview'" class="page" data-testid="duplicates-overview">
        <template v-if="o && o.fonts > 0">
          <h2 class="heading">{{ t('duplicatesModule.overview.heading', { count: o.fonts }) }}</h2>
          <p class="lead">
            {{ t('duplicatesModule.overview.files', { count: o.extraFiles }) }}
            {{ t('duplicatesModule.overview.inFolders', { count: o.folders }) }},
            {{ t('duplicatesModule.overview.rest', { size: sizeText(o.frees), trash }) }}
          </p>
          <p v-if="o.myLanguages === 'notSet'" class="offer" data-testid="dup-offer-languages">
            {{ t('duplicatesModule.overview.noLanguages') }}
            <RouterLink :to="LANGUAGE_SETTINGS" class="link">
              {{ t('duplicatesModule.overview.setLanguages') }}
            </RouterLink>
          </p>
          <section
            v-if="o.identical.sets > 0"
            class="identical"
            aria-labelledby="dup-identical"
            data-testid="dup-identical"
          >
            <div>
              <h3 id="dup-identical" class="identical-title">
                {{ t('duplicatesModule.overview.identical', { count: o.identical.files }) }}
                {{ t('duplicatesModule.overview.inFolders', { count: o.identical.folders }) }}
              </h3>
              <p class="muted">{{ t('duplicatesModule.overview.identicalBody') }}</p>
            </div>
            <div class="identical-action">
              <button type="button" class="primary" @click="reviewIdentical">
                {{
                  store.draft.identical
                    ? t('duplicatesModule.overview.inPlan')
                    : t('duplicatesModule.overview.review')
                }}
              </button>
              <span class="muted small">
                {{ t('duplicatesModule.overview.frees', { size: sizeText(o.identical.frees) }) }}
              </span>
            </div>
          </section>
          <template v-if="levels.length">
            <h3 class="section-label">{{ t('duplicatesModule.overview.toDecide') }}</h3>
            <ul class="rows">
              <li v-for="l in levels" :key="l.level">
                <button
                  type="button"
                  class="level"
                  :data-level="l.level"
                  @click="openLevel(l.level)"
                >
                  <span class="level-name">
                    <span>{{ l.title }}</span>
                    <span class="muted small">{{ l.fonts }}</span>
                  </span>
                  <span class="muted level-body">{{ l.body }}</span>
                  <span class="level-value">{{ l.value }}</span>
                  <UiIcon name="chevron" />
                </button>
              </li>
            </ul>
          </template>
          <template v-if="store.pairs.length">
            <div class="section-row">
              <h3 class="section-label">{{ t('duplicatesModule.overview.pairs') }}</h3>
              <a href="#" class="link small" @click.prevent="open('folders')">
                {{ t('duplicatesModule.overview.allPairs', { count: store.pairTotal ?? 0 }) }}
              </a>
            </div>
            <ul class="pairs">
              <li v-for="p in store.pairs" :key="`${p.a.path}|${p.b.path}`" class="pair">
                <span>
                  <code class="path"
                    ><bdi>{{ pairLine(p).path }}</bdi></code
                  >
                  <span class="muted small">
                    {{ pairLine(p).relation }}
                    <code class="path"
                      ><bdi>{{ pairLine(p).other }}</bdi></code
                    >
                  </span>
                </span>
                <span class="pair-value">
                  <span>{{ pairLine(p).shared }}</span>
                  <span class="muted small">{{ pairLine(p).size }}</span>
                </span>
              </li>
            </ul>
          </template>
        </template>
        <UiEmptyState
          v-else-if="o"
          :title="t('duplicatesModule.overview.none')"
          :body="t('duplicatesModule.overview.noneBody')"
        />
      </div>

      <!-- Sets: the queue -->
      <DuplicatesQueue v-else-if="section === 'sets'" :languages="languagePresets" :trash="trash" />

      <!-- Plan: what the draft holds, the plan, the apply (E56.15) -->
      <DuplicatesPlan
        v-else-if="section === 'plan'"
        :trash="trash"
        :size-text="sizeText"
        @history="open('history')"
      />

      <!-- Folders: the folder pairs and "Keep this side" (E56.12) -->
      <DuplicatesFolders v-else-if="section === 'folders'" :trash="trash" />

      <!-- History, hidden copies and remembered decisions (E56.15) -->
      <DuplicatesHistory v-else-if="section === 'history'" :trash="trash" :size-text="sizeText" />
      <DuplicatesKept v-else-if="section === 'hidden'" kind="hidden" />
      <DuplicatesKept v-else-if="section === 'decisions'" kind="decisions" />

      <!-- Languages and the keeper policy (E56.14) -->
      <DuplicatesRules v-else />
    </main>
  </div>
</template>

<style scoped>
.duplicates {
  display: flex;
  height: 100vh;
  min-height: 0;
}
.nav {
  box-sizing: border-box;
  display: flex;
  flex: none;
  flex-direction: column;
  width: 232px;
  padding: var(--fm-space-4) var(--fm-space-3);
  border-inline-end: 1px solid var(--fm-border);
  background-color: var(--fm-surface);
}
.back {
  align-self: flex-start;
}
.title {
  margin: var(--fm-space-4) var(--fm-space-2) var(--fm-space-2);
  font-size: var(--fm-text-lg);
}
.items {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.items .gap {
  margin-top: var(--fm-space-4);
}
.item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  height: 32px;
  padding: 0 var(--fm-space-2);
  border: 0;
  border-radius: var(--fm-radius-md);
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  text-align: start;
}
.item:hover {
  background-color: var(--fm-hover);
}
.item[aria-current='page'] {
  background-color: var(--fm-side-active, var(--fm-hover));
  font-weight: 600;
}
.count {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  font-variant-numeric: tabular-nums;
  font-weight: 400;
}
.foot {
  margin-top: auto;
  padding: var(--fm-space-3) var(--fm-space-2) 0;
  border-top: 1px solid var(--fm-border);
}
.foot-heading,
.section-label {
  margin: 0 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.foot-languages {
  margin: 0 0 var(--fm-space-2);
}
/* Two links, one per line: edit the languages (Settings), open the rules. */
.foot .link {
  display: block;
  width: fit-content;
}
.foot .link + .link {
  margin-top: var(--fm-space-1);
}
.link {
  color: var(--fm-fg);
  text-decoration: underline;
  text-underline-offset: 2px;
}
.main {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
}
.main.wide {
  display: flex;
  overflow: hidden;
}
.page {
  max-width: 960px;
  padding: var(--fm-space-6) var(--fm-space-8);
}
.heading {
  margin: 0 0 var(--fm-space-2);
  font-size: var(--fm-text-xl);
}
.lead {
  max-width: 720px;
  margin: 0 0 var(--fm-space-5);
  color: var(--fm-muted);
}
.offer {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--fm-space-2);
  margin: 0 0 var(--fm-space-5);
}
.muted {
  color: var(--fm-muted);
}
.small {
  font-size: var(--fm-text-sm);
}
.identical {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--fm-space-5);
  margin-bottom: var(--fm-space-6);
  padding: var(--fm-space-5) var(--fm-space-5);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface);
}
.identical-title {
  margin: 0 0 var(--fm-space-1);
  font-size: var(--fm-text-lg);
}
.identical .muted {
  margin: 0;
  max-width: 520px;
}
.identical-action {
  display: flex;
  flex: none;
  flex-direction: column;
  align-items: flex-end;
  gap: var(--fm-space-1);
}
.primary {
  display: inline-flex;
  align-items: center;
  height: 36px;
  padding: 0 var(--fm-space-4);
  border: 1px solid var(--fm-accent);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font: inherit;
  font-weight: 600;
}
.rows,
.pairs {
  margin: 0 0 var(--fm-space-6);
  padding: 0;
  list-style: none;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
}
.rows > li + li,
.pairs > li + li {
  border-top: 1px solid var(--fm-border);
}
.level {
  display: grid;
  grid-template-columns: 1fr 2fr auto auto;
  align-items: center;
  gap: var(--fm-space-4);
  width: 100%;
  padding: var(--fm-space-3) var(--fm-space-4);
  border: 0;
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  text-align: start;
}
.level:hover {
  background-color: var(--fm-hover);
}
.level-name {
  display: grid;
}
.section-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
}
.pair,
.decision {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-4);
  padding: var(--fm-space-3) var(--fm-space-4);
}
.pair > span:first-child,
.decision > span:first-child {
  display: grid;
  gap: 2px;
  min-width: 0;
}
.pair-value {
  display: grid;
  flex: none;
  justify-items: end;
}
.path {
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-sm);
}
.pair-line {
  overflow-wrap: anywhere;
}
.plan-identical {
  display: flex;
  align-items: center;
  gap: var(--fm-space-3);
  margin: 0 0 var(--fm-space-5);
}
.outlined {
  height: 32px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  color: var(--fm-fg);
}
.error {
  margin: var(--fm-space-4) var(--fm-space-8) 0;
  color: var(--fm-danger);
}
</style>
