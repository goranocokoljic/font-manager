<script setup lang="ts">
// The Foundries group of the sidebar (E50.12, board 9-split.png): folded at first and
// remembered, the heading counts the foundries; open, the ten with the most families, each
// with its family count, and "All N foundries…" for a searchable list of every one. A click
// filters the wall by the foundry (a chip says "Foundry · Google") and marks the row; a
// second click clears it. Right-click (or the context-menu key) offers "Merge into…": the
// API's override of E50.11, with an undo toast. The names and counts are the API's.
import { computed, nextTick, onMounted, ref } from 'vue';
import type { FoundryInfo } from '../api/types';
import { errorMessage, formatNumber, t } from '../i18n';
import { useFoundriesStore } from '../stores/foundries';
import { useSearchStore } from '../stores/search';
import FoundryPicker from './FoundryPicker.vue';
import SidebarHead from './SidebarHead.vue';
import UiPopover from './ui/UiPopover.vue';
import { useFolded } from './useFolded';

const store = useFoundriesStore();
const search = useSearchStore();
const { folded, toggle } = useFolded('foundries', true);
onMounted(() => void store.refresh());

const section = ref<HTMLElement | null>(null);

/** The short list; a chosen foundry outside it is added, so its row shows the filter. */
const rows = computed<FoundryInfo[]>(() => {
  const shown = store.largest;
  const chosen = search.foundry;
  if (chosen === null || shown.some((f) => search.isFoundry(f.name))) return shown;
  const extra = store.find(chosen);
  return extra ? [...shown, extra] : shown;
});

const familiesLabel = (n: number) => t('home.families', { count: n });

function rowOf(name: string): HTMLElement | null {
  const key = name.toLowerCase();
  return (
    [...(section.value?.querySelectorAll<HTMLElement>('[data-foundry]') ?? [])].find(
      (el) => el.dataset.foundry?.toLowerCase() === key,
    ) ?? null
  );
}

// "All N foundries…": every foundry, the unknown one too, to search and filter by.
const allButton = ref<HTMLButtonElement | null>(null);
const browsing = ref(false);
function closeBrowse(): void {
  browsing.value = false;
  allButton.value?.focus();
}
function browseChoose(name: string): void {
  search.toggleFoundry(name);
  closeBrowse();
}

// The right-click menu of a row, then the list of foundries to merge it into.
const menuFor = ref<string | null>(null);
const picking = ref(false);
const menuAnchor = ref<HTMLElement | null>(null);
const menuEl = ref<HTMLElement | null>(null);
const targets = computed(() =>
  store.named.filter((f) => f.name.toLowerCase() !== menuFor.value?.toLowerCase()),
);

function openMenu(f: FoundryInfo, e: MouseEvent): void {
  // The unknown foundry is no name to merge (the API refuses it).
  if (f.unknown) return;
  menuAnchor.value = e.currentTarget as HTMLElement;
  picking.value = false;
  menuFor.value = f.name;
}
/**
 * A right press (or Ctrl+press on macOS) opens the menu, which takes the focus. On macOS the
 * menu event comes during the press, before the press focuses the row: without this, the row
 * would take the focus back from the menu.
 */
function keepFocusForMenu(e: MouseEvent): void {
  if (e.button === 2 || e.ctrlKey) e.preventDefault();
}
/** The menu shows: its item takes the focus. */
function focusMenu(): void {
  menuEl.value?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
}
/** Closes the menu or the merge list; the focus goes back to the row. */
function closeMenu(): void {
  menuFor.value = null;
  picking.value = false;
  menuAnchor.value?.focus();
}
function onMenuKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' || e.key === 'Tab') {
    e.preventDefault();
    e.stopPropagation();
    closeMenu();
  } else if (!e.ctrlKey && !e.metaKey && !e.altKey) e.stopPropagation();
}
function startMerge(): void {
  picking.value = true;
}
async function mergeInto(into: string): Promise<void> {
  const from = menuFor.value;
  menuFor.value = null;
  picking.value = false;
  if (!from) return;
  const anchor = menuAnchor.value;
  anchor?.focus();
  if (!(await store.merge(from, into))) return;
  // The merged row is gone: the focus goes to the one it became, else to "All N foundries…".
  await nextTick();
  (rowOf(into) ?? allButton.value)?.focus();
}
</script>

<template>
  <section ref="section" class="side-section" aria-labelledby="fm-foundries-heading">
    <SidebarHead
      id="fm-foundries-heading"
      :folded="folded"
      controls="fm-foundries-list"
      :count="folded && store.named.length ? formatNumber(store.named.length) : undefined"
      :count-label="t('foundries.count', { count: store.named.length })"
      @toggle="toggle"
    >
      {{ t('foundries.heading') }}
    </SidebarHead>
    <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
    <ul
      v-show="!folded"
      id="fm-foundries-list"
      class="side-list"
      aria-labelledby="fm-foundries-heading"
    >
      <li v-if="!store.list.length" class="side-quiet">{{ t('foundries.empty') }}</li>
      <li v-for="f in rows" :key="f.name">
        <button
          type="button"
          class="side-row foundry"
          :data-foundry="f.name"
          :aria-pressed="search.isFoundry(f.name)"
          :aria-haspopup="f.unknown ? undefined : 'menu'"
          :aria-expanded="f.unknown ? undefined : menuFor === f.name"
          @click="search.toggleFoundry(f.name)"
          @mousedown="keepFocusForMenu"
          @contextmenu.prevent="openMenu(f, $event)"
        >
          <span class="side-lead" aria-hidden="true" />
          <span class="side-label">{{ f.name }}</span>
          <span class="side-count" aria-hidden="true">{{ formatNumber(f.families) }}</span>
          <span class="sr-only">{{ familiesLabel(f.families) }}</span>
        </button>
      </li>
      <li v-if="store.list.length">
        <button
          ref="allButton"
          type="button"
          class="side-row all"
          :aria-expanded="browsing"
          data-testid="all-foundries"
          @click="browsing = !browsing"
        >
          <span class="side-lead" aria-hidden="true" />
          <span class="side-label">{{
            t('foundries.all', { count: store.named.length || store.list.length })
          }}</span>
        </button>
      </li>
    </ul>
    <FoundryPicker
      :open="browsing"
      :anchor="allButton"
      :label="t('foundries.searchLabel')"
      :foundries="store.list"
      :chosen="search.foundry"
      @choose="browseChoose"
      @close="closeBrowse"
    />
    <UiPopover
      :open="menuFor !== null && !picking"
      :anchor="menuAnchor"
      @close="closeMenu"
      @placed="focusMenu"
    >
      <div
        v-if="menuFor !== null"
        ref="menuEl"
        class="menu"
        role="menu"
        :aria-label="t('foundries.menuLabel', { name: menuFor })"
        @keydown="onMenuKey"
      >
        <button role="menuitem" type="button" :disabled="!targets.length" @click="startMerge">
          {{ t('foundries.merge') }}
        </button>
      </div>
    </UiPopover>
    <FoundryPicker
      :open="menuFor !== null && picking"
      :anchor="menuAnchor"
      :label="t('foundries.mergeLabel', { name: menuFor ?? '' })"
      :foundries="targets"
      @choose="mergeInto"
      @close="closeMenu"
    />
  </section>
</template>

<style scoped>
.all .side-label {
  color: var(--fm-muted);
}
.menu {
  display: flex;
  flex-direction: column;
  min-width: 160px;
}
.menu button {
  padding: var(--fm-space-1) var(--fm-space-2);
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: inherit;
  font: inherit;
  text-align: start;
  white-space: nowrap;
}
.menu button:hover:not(:disabled),
.menu button:focus-visible {
  background-color: var(--fm-hover);
}
.menu button:disabled {
  color: var(--fm-muted);
}
.error {
  margin: 4px 8px;
  color: var(--fm-danger);
}
</style>
