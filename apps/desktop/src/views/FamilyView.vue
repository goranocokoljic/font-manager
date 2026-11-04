<script setup lang="ts">
// The family page (E42.1): the frame every section of E42 lives in. Only the panel under the
// section switch changes with the section; the hero, the axis row and the rail stay mounted,
// so what is chosen there carries across sections. E42.2 to E42.9 fill the parts.
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRoute } from 'vue-router';
import FamilyCollectionMenu from '../components/FamilyCollectionMenu.vue';
import TaggingPanel from '../components/TaggingPanel.vue';
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
import { useActions, useCommandsStore } from '../stores/commands';
import { FAMILY_SECTIONS, parseFamilyId, useFamilyPageStore } from '../stores/familyPage';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import { useSearchStore } from '../stores/search';
import { useSimilarStore } from '../stores/similar';
import { useTaggingStore } from '../stores/tagging';
import { useFamilyNavigation } from './useFamilyNavigation';
import { useLiveLibrary } from './useLiveLibrary';

const route = useRoute();
const page = useFamilyPageStore();
const commands = useCommandsStore();
const preview = usePreviewStore();
const search = useSearchStore();
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

// The webview's own transition moves the specimen; without one the page slides in. A deep
// link and a reload just appear.
const slideIn = page.takePushed() && typeof document.startViewTransition !== 'function';

const faceId = computed(() => page.face?.id ?? null);
const heldFamily = useHeldFace(faceId);
const specimen = computed(() =>
  page.face ? sampleText(preview.settingsFor(ALL_FONTS_VIEW), page.face, preview.pangramText) : '',
);

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
  if (e.altKey) {
    if (e.key === 'ArrowLeft' && !e.repeat) {
      e.preventDefault();
      void back();
    }
    return;
  }
  if (e.key === 'Escape') {
    if (!e.repeat) {
      e.preventDefault();
      void back();
    }
    return;
  }
  const el = e.target instanceof Element ? e.target : null;
  if (yieldsToFocus(e, { key: e.key }) || el?.closest('input, [role="slider"]')) return;
  const key = inlineKey(e.key);
  if (key === 'ArrowLeft') step(page.previous);
  else if (key === 'ArrowRight') step(page.next);
  else return;
  e.preventDefault();
}

// The page takes the focus, so Escape and the arrows work at once.
const title = ref<HTMLHeadingElement | null>(null);
onMounted(() => {
  title.value?.focus();
  window.addEventListener('keydown', onKey);
  void search.loadLanguages();
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  // Nothing on screen needs the family any more, so library changes stop reading it.
  void page.show(null);
});
</script>

<template>
  <div class="family-page" :class="{ 'slide-in': slideIn }">
    <header class="bar">
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
          @click="step(page.previous)"
        />
        <UiIconButton
          class="key"
          icon="arrowRight"
          :label="t('family.next')"
          aria-keyshortcuts="ArrowRight"
          :disabled="page.next === null"
          @click="step(page.next)"
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
          <h1 ref="title" class="name" tabindex="-1">{{ page.summary?.name }}</h1>
          <p
            class="specimen"
            data-testid="family-specimen"
            :class="{ loading: !heldFamily }"
            :style="heldFamily ? { fontFamily: `'${heldFamily}'` } : undefined"
          >
            {{ specimen }}
          </p>
        </section>
        <section class="axes" data-testid="family-axes" :aria-label="t('family.axes')">
          {{ page.summary?.isVariable ? t('family.axesVariable') : t('family.axesStatic') }}
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
                  <ul v-if="page.section === 'styles'" class="styles">
                    <li v-for="f in page.faces" :key="f.id">{{ f.style }}</li>
                  </ul>
                  <p v-else class="quiet">{{ t('family.sectionLater') }}</p>
                </section>
              </FmFade>
            </div>
          </div>
          <aside class="rail" data-testid="family-rail" :aria-label="t('family.rail')">
            <dl class="facts">
              <template v-if="page.detail?.designer">
                <dt>{{ t('family.designer') }}</dt>
                <dd>{{ page.detail.designer }}</dd>
              </template>
              <template v-if="page.summary?.foundry">
                <dt>{{ t('family.foundry') }}</dt>
                <dd>{{ page.summary.foundry }}</dd>
              </template>
              <template v-if="page.summary">
                <dt>{{ t('family.stylesFact') }}</dt>
                <dd class="count">{{ page.summary.styleCount }}</dd>
              </template>
            </dl>
          </aside>
        </div>
      </template>
    </main>
    <TaggingPanel />
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
.specimen {
  overflow: hidden;
  margin: var(--fm-space-3) 0;
  font-size: clamp(48px, 9vw, 132px);
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
  view-transition-name: fm-family-specimen;
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
.styles {
  margin: 0;
  padding: 0;
  list-style: none;
}
.styles li {
  padding: var(--fm-space-2) 0;
  border-bottom: 1px solid var(--fm-border);
}
.quiet {
  margin: 0;
  color: var(--fm-muted);
}
.facts {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: var(--fm-space-1) var(--fm-space-3);
  margin: 0;
  font-size: var(--fm-text-sm);
}
.facts dt {
  color: var(--fm-muted);
}
.facts dd {
  margin: 0;
}
.count {
  font-variant-numeric: tabular-nums;
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
/* "Back to the wall": the reverse, on the exit curve at 0.8× the time. */
html.fm-family-closing::view-transition-group(*),
html.fm-family-closing::view-transition-old(*),
html.fm-family-closing::view-transition-new(*) {
  animation-duration: calc(var(--fm-motion-page) * 0.8);
  animation-timing-function: ease-in;
}
</style>
