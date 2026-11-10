<script setup lang="ts">
// The family page (E42.1): the frame every section of E42 lives in. Only the panel under the
// section switch changes with the section; the hero, the axis row and the rail stay mounted,
// so what is chosen there carries across sections. E42.2 to E42.9 fill the parts; a language
// chosen in the rail (E42.9) previews the whole page in its pangram. They read the selection
// store, so the page keeps the face it shows selected: the `?face=` of the route, else the
// family's first.
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import FamilyCharacters from '../components/FamilyCharacters.vue';
import FamilyFeatures from '../components/FamilyFeatures.vue';
import FamilyInfo from '../components/FamilyInfo.vue';
import FamilyPairings from '../components/FamilyPairings.vue';
import FamilyCollectionMenu from '../components/FamilyCollectionMenu.vue';
import FamilyRail from '../components/FamilyRail.vue';
import FamilyStyles from '../components/FamilyStyles.vue';
import FamilyText from '../components/FamilyText.vue';
import InspectorAxes from '../components/InspectorAxes.vue';
import InspectorInfo from '../components/InspectorInfo.vue';
import TaggingPanel from '../components/TaggingPanel.vue';
import CompareSheet from '../components/CompareSheet.vue';
import CompareToggle from '../components/CompareToggle.vue';
import UiButton from '../components/ui/UiButton.vue';
import UiEmptyState from '../components/ui/UiEmptyState.vue';
import UiIcon from '../components/ui/UiIcon.vue';
import UiIconButton from '../components/ui/UiIconButton.vue';
import UiSegmentedControl from '../components/ui/UiSegmentedControl.vue';
import { errorMessage, inlineKey, t } from '../i18n';
import { ACTIONS, yieldsToFocus } from '../keyboard/keymap';
import { FmFade } from '../motion';
import { useHeldFace } from '../preview/useHeldFace';
import { sampleText } from '../preview/sampleText';
import { useSpecimen } from '../preview/useSpecimen';
import SpecimenText from '../components/SpecimenText';
import MissingGlyphs from '../components/MissingGlyphs.vue';
import { useWeightScrub } from '../preview/useWeightScrub';
import { variationSettings } from '../preview/variations';
import { useAxesStore } from '../stores/axes';
import type { CollectionId, PairingSuggestion } from '../api/types';
import { useCatalogsStore } from '../stores/catalogs';
import { useCollectionsStore } from '../stores/collections';
import { useCompareStore } from '../stores/compare';
import { useActions, useCommandsStore } from '../stores/commands';
import { FAMILY_SECTIONS, parseFamilyId, useFamilyPageStore } from '../stores/familyPage';
import type { PairingCard } from '../stores/familyPairings';
import { useFamilyRailStore } from '../stores/familyRail';
import { useFeaturesStore } from '../stores/features';
import { usePalettesStore } from '../stores/palettes';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import { useSearchStore } from '../stores/search';
import { parseFaceId, useSelectionStore } from '../stores/selection';
import { useSimilarStore } from '../stores/similar';
import { useTaggingStore } from '../stores/tagging';
import { useFamilyNavigation } from './useFamilyNavigation';
import { useLiveLibrary } from './useLiveLibrary';

const route = useRoute();
const router = useRouter();
const page = useFamilyPageStore();
const commands = useCommandsStore();
const compare = useCompareStore();
const preview = usePreviewStore();
const search = useSearchStore();
const selection = useSelectionStore();
const similar = useSimilarStore();
const tagging = useTaggingStore();
const { back, step } = useFamilyNavigation();

useLiveLibrary('home');

watch(
  () => route.params.id,
  (id) => {
    if (route.name === 'family') void page.show(parseFamilyId(id));
  },
  { immediate: true },
);

watch(
  () => route.query.face,
  (face) => {
    if (route.name === 'family') page.chosenFace = parseFaceId(face);
  },
  { immediate: true },
);
watch(
  () => page.face?.id ?? null,
  (face) => void selection.select(face),
  { immediate: true },
);

/** A style in the list: the page and its panels show that one. */
function chooseStyle(face: number): void {
  void router.replace({ query: { face: String(face) } });
}

/** A pairing card opens the Studio: the body face is the panel's, the headline face its block's. */
function openPairing(card: PairingCard): void {
  void router.push({
    name: 'studio',
    query: { face: String(card.body), headline: String(card.headline) },
  });
}

/** The rail's "Open in Pairing Studio": this face sets the headline, the top pairing the body. */
function openStudio(top: PairingSuggestion | null): void {
  const face = page.face?.id;
  if (face === undefined) return;
  void router.push({
    name: 'studio',
    query: top ? { face: String(top.face.id), headline: String(face) } : { face: String(face) },
  });
}

/** "Used in": the library shows that collection. */
const collections = useCollectionsStore();
// Adding the family to a collection, here or elsewhere, changes what "Used in" lists.
watch(
  () => collections.collections,
  () => {
    if (page.id !== null) void page.show(page.id);
  },
);
async function openCollection(id: CollectionId): Promise<void> {
  collections.show(id);
  await back();
}

/** No candidate in the library: the library shows the first online catalog to browse. */
const catalogs = useCatalogsStore();
async function browseCatalogs(): Promise<void> {
  await catalogs.load();
  const first = catalogs.catalogs[0];
  if (first) catalogs.show(first.id);
  await back();
}

// The webview's own transition moves the specimen; without one the page slides in. A deep
// link and a reload just appear.
const slideIn = page.takePushed() && typeof document.startViewTransition !== 'function';

const faceId = computed(() => page.face?.id ?? null);
const rail = useFamilyRailStore();
watch(faceId, (id) => void rail.load(id), { immediate: true });
const heldFamily = useHeldFace(faceId);
const axesSet = useAxesStore();
const features = useFeaturesStore();
const palettes = usePalettesStore();
/** What the axis row, the Features section and the palette picker set shows on the hero at once. */
const shown = computed(() =>
  selection.detail && selection.detail.face.id === page.face?.id ? selection.detail : null,
);
const heroVariation = computed(() =>
  page.face ? variationSettings(shown.value?.axes ?? [], axesSet.values(page.face.id) ?? []) : '',
);
const specimenStyle = computed(() => {
  const face = page.face;
  if (!face || !heldFamily.value) return undefined;
  return {
    fontFamily: `'${heldFamily.value}'`,
    fontVariationSettings: heroVariation.value,
    fontFeatureSettings: features.settings(face.id),
    fontPalette: palettes.fontPaletteOf(ALL_FONTS_VIEW, face),
  };
});
/** The page's preview text: the rail's language when one is chosen (E42.9), else the user's. */
const specimen = computed(() => {
  if (!page.face) return '';
  return (
    rail.textOf(search.languages) ??
    sampleText(preview.settingsFor(ALL_FONTS_VIEW), page.face, preview.pangramText)
  );
});

/** The hero's text with the characters the face lacks as tofu (E43.4). */
const heroSpecimen = useSpecimen(
  () => page.face?.id,
  () => specimen.value,
);

const scrubAxis = computed(() => {
  const axis = shown.value?.axes.find((a) => a.tag === 'wght');
  return axis && (axis.max ?? 0) > (axis.min ?? 0) ? axis : null;
});
const scrub = useWeightScrub(faceId, scrubAxis);

const license = computed(() => {
  const open = page.summary?.openLicense;
  if (open) return t(`info.openLicenses.${open}`);
  const declared = page.summary?.declaredLicense;
  return declared && declared !== 'unknown' ? t(`chips.licenses.${declared}`) : null;
});
/** "Variable, 4 axes · 18 named styles", or "6 styles" for a static face. */
const counts = computed(() => {
  const axes = shown.value?.axes.length ?? 0;
  if (axes)
    return t('family.variableCounts', {
      axes: t('family.axisCount', { count: axes }),
      styles: t('family.namedStyleCount', { count: shown.value?.instances.length ?? 0 }),
    });
  return page.summary ? t('family.styleCount', { count: page.summary.styleCount }) : null;
});
/** Foundry and year, then the counts and the licence; a part the font does not state is left out. */
const metaParts = computed(() => {
  const made = [page.summary?.foundry, page.detail?.year].filter((p) => p != null).join(' · ');
  return [made, counts.value, license.value].filter((p): p is string => !!p);
});

const sections = computed(() =>
  FAMILY_SECTIONS.map((value) => ({ value, label: t(`family.sections.${value}`) })),
);
const activeLabel = computed(() =>
  page.summary?.activation === 'active' ? t('family.deactivate') : t('family.activate'),
);

function tag(): void {
  if (page.id !== null) void tagging.openFor({ faces: [], families: [page.id] });
}

/** The results replace the list in the library view, so the page goes back to show them. */
function findSimilar(): void {
  if (!page.face) return;
  void similar.openFor(page.face);
  void back();
}

useActions([
  {
    id: 'family.back',
    get title() {
      return t('family.backAction');
    },
    get group() {
      return t('palette.groups.view');
    },
    run: back,
  },
  {
    id: ACTIONS.activateSession,
    get title() {
      return t('home.actions.activateSession');
    },
    get group() {
      return t('palette.groups.activation');
    },
    enabled: () => page.summary !== null,
    run: () => page.toggleActivation('session'),
  },
  {
    id: ACTIONS.activatePermanent,
    get title() {
      return t('home.actions.activatePermanent');
    },
    get group() {
      return t('palette.groups.activation');
    },
    enabled: () => page.summary !== null,
    run: () => page.toggleActivation('permanent'),
  },
  {
    id: ACTIONS.tag,
    get title() {
      return t('home.actions.tag');
    },
    get group() {
      return t('palette.groups.tags');
    },
    enabled: () => page.id !== null && !tagging.open,
    run: tag,
  },
  {
    id: ACTIONS.similar,
    get title() {
      return t('home.actions.findSimilar');
    },
    get group() {
      return t('palette.groups.search');
    },
    enabled: () => page.face !== null,
    run: findSimilar,
  },
]);

/** Esc and Alt+← go back, ← and → change the family, unless a field or a dialog has the key. */
function onKey(e: KeyboardEvent): void {
  if (e.defaultPrevented || e.isComposing || commands.open) return;
  if (document.querySelector('[aria-modal="true"]')) return;
  if (e.ctrlKey || e.metaKey || e.shiftKey) return;
  // The notes save when their field loses the focus: Escape leaves the field and stays here.
  const typing = yieldsToFocus(e, { key: e.key });
  if (e.altKey) {
    if (e.key === 'ArrowLeft' && !e.repeat && !typing) {
      e.preventDefault();
      void back();
    }
    return;
  }
  const el = e.target instanceof Element ? e.target : null;
  if (e.key === 'Escape') {
    if (!e.repeat) {
      e.preventDefault();
      if (typing && el instanceof HTMLElement) el.blur();
      else void back();
    }
    return;
  }
  if (typing || el?.closest('input, [role="slider"], [data-keeps-arrows]')) return;
  const key = inlineKey(e.key);
  if (key === 'ArrowLeft') void step(-1);
  else if (key === 'ArrowRight') void step(1);
  else return;
  e.preventDefault();
}

// The page takes the focus, so Escape and the arrows work at once.
const title = ref<HTMLHeadingElement | null>(null);
onMounted(() => {
  title.value?.focus();
  window.addEventListener('keydown', onKey);
  void search.loadLanguages();
  void collections.refresh();
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  // Nothing on screen needs the family any more, so library changes stop reading it.
  void page.show(null);
  void rail.load(null);
});
</script>

<template>
  <div class="family-page" :class="{ 'slide-in': slideIn }">
    <header class="bar" data-top-bar>
      <UiButton class="back" aria-keyshortcuts="Escape Alt+ArrowLeft" @click="back">
        <UiIcon name="chevronBack" class="fm-directional" />
        {{ t('family.library') }}
      </UiButton>
      <p v-if="page.index >= 0" class="position" data-testid="family-position">
        {{ t('family.position', { position: page.index + 1, total: page.total }) }}
        <span aria-hidden="true">·</span>
        <UiIconButton
          class="key"
          icon="arrowLeft"
          :label="t('family.previous')"
          aria-keyshortcuts="ArrowLeft"
          :disabled="page.previous === null"
          @click="step(-1)"
        />
        <UiIconButton
          class="key"
          icon="arrowRight"
          :label="t('family.next')"
          aria-keyshortcuts="ArrowRight"
          :disabled="page.next === null"
          @click="step(1)"
        />
        {{ t('family.nextHint') }}
      </p>
      <span class="actions">
        <UiIconButton
          :icon="page.favorite ? 'heartFilled' : 'heart'"
          :label="t('family.favorite')"
          :aria-pressed="page.favorite"
          :disabled="!page.detail"
          @click="page.toggleFavorite"
        />
        <FamilyCollectionMenu :family="page.detail ? page.id : null" />
        <UiIconButton
          icon="tag"
          :label="t('family.tag')"
          aria-keyshortcuts="T"
          :disabled="!page.detail"
          @click="tag"
        />
        <CompareToggle
          v-if="page.face"
          data-testid="family-compare"
          scope="family"
          fresh
          :face="page.face"
        />
        <UiIconButton
          icon="similar"
          :label="t('family.findSimilar')"
          aria-keyshortcuts="S"
          :disabled="!page.face"
          @click="findSimilar"
        />
        <UiButton class="activate" :disabled="!page.summary" @click="page.toggleActivation()">
          <UiIcon name="power" />
          {{ activeLabel }}
        </UiButton>
      </span>
    </header>

    <main class="scroll">
      <p v-if="page.error" role="alert" class="error">{{ errorMessage(page.error) }}</p>
      <UiEmptyState
        v-else-if="page.id === null"
        :title="t('family.notFound')"
        :body="t('family.notFoundBody')"
      >
        <button type="button" data-testid="empty-action" @click="back">
          {{ t('family.library') }}
        </button>
      </UiEmptyState>
      <template v-else>
        <section class="hero" data-testid="family-hero" :aria-label="t('family.hero')">
          <div class="title-line">
            <h1 ref="title" class="name" tabindex="-1">{{ page.summary?.name }}</h1>
            <p v-if="page.summary" class="meta" data-testid="family-meta">
              <span v-for="part in metaParts" :key="part" class="meta-part">{{ part }}</span>
              <span class="state" :data-state="page.summary.activation">
                <span class="dot" aria-hidden="true" />
                {{ t(`columns.state.${page.summary.activation}`) }}
              </span>
            </p>
          </div>
          <p
            class="specimen"
            data-testid="family-specimen"
            :class="{ loading: !heldFamily, scrubbable: scrubAxis }"
            :style="specimenStyle"
            @pointerdown="scrub.onPointerDown"
            @pointermove="scrub.onPointerMove"
            @pointerup="scrub.onPointerEnd"
            @pointercancel="scrub.onPointerEnd"
          >
            <SpecimenText :text="specimen" :missing="heroSpecimen.tofu" />
          </p>
          <MissingGlyphs
            v-if="heroSpecimen.tofu.length"
            class="hero-missing"
            :missing="heroSpecimen.tofu"
          />
        </section>
        <section class="axes" data-testid="family-axes" :aria-label="t('family.axes')">
          <InspectorAxes v-if="page.face?.isVariable" />
          <template v-else>{{ t('family.axesStatic') }}</template>
        </section>
        <UiSegmentedControl
          v-model="page.section"
          class="switch"
          :options="sections"
          :label="t('family.sectionSwitch')"
          wrap
        />
        <div class="columns">
          <div class="main" data-testid="family-main">
            <div class="panels">
              <FmFade speed="fast">
                <section
                  :key="page.section"
                  class="panel"
                  data-testid="family-panel"
                  :data-section="page.section"
                  :aria-label="t(`family.sections.${page.section}`)"
                >
                  <FamilyStyles
                    v-if="page.section === 'styles'"
                    :detail="shown"
                    :text="specimen"
                    @choose="chooseStyle"
                  />
                  <FamilyText
                    v-else-if="page.section === 'text'"
                    :detail="shown"
                    :text="specimen"
                    :face-style="specimenStyle"
                  />
                  <FamilyCharacters
                    v-else-if="page.section === 'characters'"
                    :detail="shown"
                    :face-style="specimenStyle"
                  />
                  <FamilyFeatures
                    v-else-if="page.section === 'features'"
                    :detail="shown"
                    :text="specimen"
                  />
                  <FamilyPairings
                    v-else-if="page.section === 'pairings'"
                    :face="faceId"
                    :text="specimen"
                    @open="openPairing"
                    @browse="browseCatalogs"
                  />
                  <template v-else-if="page.section === 'info'">
                    <FamilyInfo
                      :detail="shown"
                      :summary="page.summary"
                      :year="page.detail?.year ?? null"
                      :designer="page.detail?.designer ?? null"
                      :face-style="specimenStyle"
                    />
                    <!-- The user's own part of the face: palettes, tags, rating, notes. -->
                    <InspectorInfo part="yours" class="yours" />
                  </template>
                  <p v-else class="quiet">{{ t('family.sectionLater') }}</p>
                </section>
              </FmFade>
            </div>
          </div>
          <aside class="rail" data-testid="family-rail" :aria-label="t('family.rail')">
            <FamilyRail
              :face="faceId"
              :detail="shown"
              :family="page.detail"
              :summary="page.summary"
              :presets="search.languages"
              :collections="collections.collections"
              :text="specimen"
              :variation="heroVariation"
              :features="specimenStyle?.fontFeatureSettings"
              @studio="openStudio"
              @collection="openCollection"
            />
          </aside>
        </div>
      </template>
    </main>
    <TaggingPanel />
    <!-- The family page's Compare opens the sheet here too (E48.5). -->
    <CompareSheet v-if="compare.open" />
  </div>
</template>

<style scoped>
.family-page {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: var(--fm-bg);
  color: var(--fm-fg);
}
.bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--fm-space-3);
  min-height: 48px;
  padding: 0 var(--fm-space-3);
  border-bottom: 1px solid var(--fm-border);
}
.position {
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.actions {
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
  margin-inline-start: auto;
}
/* The primary action: words on the accent take the paired token (DESIGN.md). */
.activate.activate,
.activate.activate:not(:disabled):hover {
  height: 32px;
  margin-inline-start: var(--fm-space-2);
  padding: 0 var(--fm-space-3);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font-weight: 500;
}
.scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--fm-space-4);
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
.name {
  margin: 0;
  font-size: var(--fm-text-lg);
  font-weight: 600;
}
.name:focus {
  outline: none;
}
.title-line {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--fm-space-1) var(--fm-space-3);
}
.meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-1) var(--fm-space-3);
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  font-variant-numeric: tabular-nums;
}
.state {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-1);
}
/* The state dot: activation fills it, the fill scaling 0 → 1 (E46.4, motion table "Activate"). */
.dot {
  position: relative;
  width: 7px;
  height: 7px;
  border: 1px solid var(--fm-muted);
  border-radius: 50%;
  transition: border-color var(--fm-motion-fast) var(--fm-ease);
}
.dot::before {
  position: absolute;
  inset: -1px;
  border-radius: 50%;
  background-color: var(--fm-success);
  transform: scale(0);
  transition: transform var(--fm-motion-fast) var(--fm-ease);
  content: '';
}
.state[data-state='active'] .dot,
.state[data-state='partial'] .dot {
  border-color: var(--fm-success);
}
.state[data-state='active'] .dot::before {
  transform: scale(1);
}
.specimen {
  overflow: hidden;
  margin: var(--fm-space-3) 0;
  font-size: 132px;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
  view-transition-name: fm-family-specimen;
}
.scrubbable {
  cursor: ew-resize;
  touch-action: pan-y;
  user-select: none;
}
/* "6 glyphs missing · š đ č" under the hero (E43.4). */
.hero-missing {
  display: block;
  margin-block: calc(-1 * var(--fm-space-2)) var(--fm-space-3);
  font-size: var(--fm-text-sm);
}
/* Never a fallback font: nothing shows until the face has loaded. */
.specimen.loading {
  visibility: hidden;
}
.axes {
  min-height: 40px;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.switch {
  margin-block: var(--fm-space-3);
}
.columns {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: var(--fm-space-5);
}
/* Under 1100 px the rail wraps under the main column. */
@media (min-width: 1100px) {
  .columns {
    grid-template-columns: minmax(560px, 1fr) 320px;
  }
}
/* The old and the new panel share one cell, so they crossfade in place. */
.panels {
  display: grid;
}
.panel {
  grid-area: 1 / 1;
  min-width: 0;
}
.yours.yours {
  margin-block-start: var(--fm-space-5);
  padding: var(--fm-space-4) 0 0;
  border-block-start: 1px solid var(--fm-border);
}
.quiet {
  margin: 0;
  color: var(--fm-muted);
}
/* MOTION.md "Open a family", fallback: 24 px from the end side over a fade. */
.slide-in {
  animation: fm-family-slide var(--fm-motion-page) var(--fm-ease);
}
:global([dir='rtl']) .slide-in {
  animation-name: fm-family-slide-rtl;
}
@keyframes fm-family-slide {
  from {
    opacity: 0;
    transform: translateX(24px);
  }
}
@keyframes fm-family-slide-rtl {
  from {
    opacity: 0;
    transform: translateX(-24px);
  }
}
</style>

<!-- Not scoped: view transitions draw on pseudo-elements of the root. -->
<style>
html.fm-family-opening::view-transition-group(*),
html.fm-family-opening::view-transition-old(*),
html.fm-family-opening::view-transition-new(*) {
  animation-duration: var(--fm-motion-page);
  animation-timing-function: var(--fm-ease);
}
/* "Back to the wall": the reverse, on the exit curve at 0.8× the time (220 ms). */
html.fm-family-closing::view-transition-group(*),
html.fm-family-closing::view-transition-old(*),
html.fm-family-closing::view-transition-new(*) {
  animation-duration: var(--fm-motion-page-exit);
  animation-timing-function: var(--fm-ease-exit);
}
/* The arrows (E46.1): only the content under the top bar crossfades. It is its own snapshot,
   with the hero inside it; the rest of the page (the bar) is the root, which does not move. */
html.fm-family-stepping .family-page > .scroll {
  view-transition-name: fm-family-content;
}
html.fm-family-stepping .family-page .specimen {
  view-transition-name: none;
}
html.fm-family-stepping::view-transition-group(root),
html.fm-family-stepping::view-transition-old(root),
html.fm-family-stepping::view-transition-new(root) {
  animation: none;
}
html.fm-family-stepping::view-transition-group(fm-family-content),
html.fm-family-stepping::view-transition-old(fm-family-content),
html.fm-family-stepping::view-transition-new(fm-family-content) {
  animation-duration: var(--fm-motion-move);
  animation-timing-function: var(--fm-ease);
}
</style>
