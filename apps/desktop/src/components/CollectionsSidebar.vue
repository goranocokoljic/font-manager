<script setup lang="ts">
import SidebarHead from './SidebarHead.vue';
import UiIconButton from './ui/UiIconButton.vue';
import UiIcon from './ui/UiIcon.vue';
import UiPopover from './ui/UiPopover.vue';
// The Collections group of the sidebar: every manual and smart collection as a
// tree, with its colour dot or, for a smart one, the star, and its face count.
// Click one to show it in the list. Make, rename,
// nest (In, Out), reorder (Up, Down) and delete them with the row buttons or by dragging a
// collection onto another (top or bottom edge: before or after it; middle:
// inside it). Drop fonts from the list on a collection to add them; the
// palette has an "Add selected fonts to …" command per collection, and
// "Export license inventory…" for the open one (a row button too). The row
// buttons show on the open collection and under keyboard focus. "New smart" and a
// smart collection's "Edit rules" open the rule editor (E14.1).
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { COLLECTION_COLORS, collectionColorVar } from '../api/collectionColors';
import type { CollectionId } from '../api/types';
import { errorMessage, formatNumber, t } from '../i18n';
import { useFolded } from './useFolded';
import { isFontDrag, pickOf, readFontDrop } from '../list/drag';
import { useCatalogsStore } from '../stores/catalogs';
import { useCollectionsStore, type CollectionRow } from '../stores/collections';
import { useCommandsStore } from '../stores/commands';
import { useFontListStore } from '../stores/fontList';
import { useQuickCollectionStore } from '../stores/quickCollection';
import { useRuleEditorStore } from '../stores/ruleEditor';

/** The `DataTransfer` type a collection drag carries its id in. */
const COLLECTION_MIME = 'application/x-fm-collection';

type Zone = 'before' | 'inside' | 'after';

const store = useCollectionsStore();
// While a review or a catalog is in the center, no collection is the current one.
const quick = useQuickCollectionStore();
const catalogs = useCatalogsStore();
const listShown = computed(() => !quick.open && catalogs.open === null);
const fontList = useFontListStore();
const commands = useCommandsStore();
const ruleEditor = useRuleEditorStore();
onMounted(() => void store.refresh());

/** A name being typed: for a new collection (under `parent`) or a rename (`id`). */
const editing = ref<{ id: CollectionId | null; parent: CollectionId | null; name: string } | null>(
  null,
);
const input = ref<HTMLInputElement[] | HTMLInputElement | null>(null);
const dragged = ref<CollectionId | null>(null);
const target = ref<{ id: CollectionId; zone: Zone } | null>(null);

async function startEdit(id: CollectionId | null, parent: CollectionId | null, name = '') {
  editing.value = { id, parent, name };
  await nextTick();
  const el = Array.isArray(input.value) ? input.value[0] : input.value;
  el?.focus();
  el?.select();
}

async function commit(): Promise<void> {
  const e = editing.value;
  if (!e) return;
  const ok = e.id == null ? await store.create(e.name, e.parent) : await store.rename(e.id, e.name);
  if (ok) editing.value = null;
}

function rowOf(id: CollectionId): CollectionRow | undefined {
  return store.rows.find((r) => r.collection.id === id);
}

/** Moves a collection one place up or down among its siblings. */
function shift(row: CollectionRow, by: number): void {
  void store.move(row.collection.id, row.collection.parent, row.position + by);
}

/** The number of collections directly inside `id`. */
function childCount(id: CollectionId): number {
  return store.rows.filter((r) => r.collection.parent === id).length;
}

/** Nests a collection inside the sibling above it, as its last child (the keyboard's nest). */
function moveIn(row: CollectionRow): void {
  const above = store.rows.find(
    (r) => r.collection.parent === row.collection.parent && r.position === row.position - 1,
  );
  if (!above) return;
  void store.move(row.collection.id, above.collection.id, childCount(above.collection.id));
}

/** Moves a nested collection out, right after its parent. */
function moveOut(row: CollectionRow): void {
  const parent = row.collection.parent == null ? undefined : rowOf(row.collection.parent);
  if (!parent) return;
  void store.move(row.collection.id, parent.collection.parent, parent.position + 1);
}

function zoneOf(e: DragEvent): Zone {
  const el = e.currentTarget as HTMLElement;
  const box = el.getBoundingClientRect();
  const y = box.height ? (e.clientY - box.top) / box.height : 0.5;
  return y < 0.3 ? 'before' : y > 0.7 ? 'after' : 'inside';
}

function onDragStart(row: CollectionRow, e: DragEvent): void {
  if (!e.dataTransfer) return;
  dragged.value = row.collection.id;
  e.dataTransfer.effectAllowed = 'move';
  e.dataTransfer.setData(COLLECTION_MIME, String(row.collection.id));
}

function onDragOver(row: CollectionRow, e: DragEvent): void {
  if (isFontDrag(e)) {
    if (row.collection.rules) return; // a smart collection fills itself
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
    target.value = { id: row.collection.id, zone: 'inside' };
  } else if (dragged.value != null && dragged.value !== row.collection.id) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    target.value = { id: row.collection.id, zone: zoneOf(e) };
  }
}

function onDrop(row: CollectionRow, e: DragEvent): void {
  e.preventDefault();
  const zone = target.value?.zone ?? 'inside';
  target.value = null;
  const fonts = readFontDrop(e);
  if (fonts) {
    void store.add(row.collection.id, fonts);
    return;
  }
  const id = dragged.value;
  dragged.value = null;
  const src = id == null ? undefined : rowOf(id);
  if (!src || id === row.collection.id) return;
  if (zone === 'inside') {
    void store.move(id!, row.collection.id, childCount(row.collection.id));
    return;
  }
  // The API places it among the siblings without itself.
  const same = src.collection.parent === row.collection.parent;
  const at = row.position - (same && src.position < row.position ? 1 : 0);
  void store.move(id!, row.collection.parent, zone === 'before' ? at : at + 1);
}

function onDragEnd(): void {
  dragged.value = null;
  target.value = null;
}

/** The selected rows of the list, or the cursor row when nothing is selected. */
const listPick = computed(() => {
  const selected = fontList.selection.selected;
  const cursor = fontList.cursor;
  return pickOf(selected.size ? selected : cursor ? [cursor] : []);
});

// One palette command per manual collection, kept in step with the list.
let undo: (() => void)[] = [];
function registerCommands(): void {
  undo.splice(0).forEach((u) => u());
  undo = [
    commands.register({
      id: 'collections.new',
      get title() {
        return t('collections.commands.new');
      },
      get group() {
        return t('collections.commands.group');
      },
      run: () => startEdit(null, null),
    }),
    commands.register({
      id: 'collections.newSmart',
      get title() {
        return t('collections.commands.newSmart');
      },
      get group() {
        return t('collections.commands.group');
      },
      run: () => ruleEditor.openNew(),
    }),
    // The open collection's families, or the whole library under "All fonts".
    commands.register({
      id: 'collections.exportLicenseInventory',
      get title() {
        return t('collections.commands.exportLicenseInventory');
      },
      get group() {
        return t('collections.commands.group');
      },
      run: () => store.exportLicenseInventory(store.openId),
    }),
    ...store.collections
      .filter((c) => !c.rules)
      .map((c) =>
        commands.register({
          id: `collections.add.${c.id}`,
          get title() {
            return t('collections.commands.addSelected', { name: c.name });
          },
          get group() {
            return t('collections.commands.group');
          },
          enabled: () => listPick.value.faces.length + listPick.value.families.length > 0,
          run: () => store.add(c.id, listPick.value),
        }),
      ),
  ];
}
watch(() => store.collections, registerCommands, { immediate: true });
onBeforeUnmount(() => undo.splice(0).forEach((u) => u()));

/**
 * Picks a colour from the menu; the colour it already has takes it off again. Nothing opens
 * after it, so the focus goes back to the row's button.
 */
function pickColor(key: string): void {
  const row = menuRow.value;
  closeMenu(true);
  if (row) void store.setColor(row.collection, row.collection.color === key ? null : key);
}

/** The row whose "more" menu is open, and the button it hangs from. */
const menu = ref<{ id: CollectionId } | null>(null);
const menuAnchor = ref<HTMLElement | null>(null);
const menuEl = ref<HTMLElement | null>(null);
const menuRow = computed(() => (menu.value ? rowOf(menu.value.id) : undefined));

async function openMenu(row: CollectionRow, e: Event): Promise<void> {
  const el = e.currentTarget as HTMLElement;
  if (menu.value?.id === row.collection.id) return closeMenu();
  menuAnchor.value = el;
  menu.value = { id: row.collection.id };
  await nextTick();
  await nextTick();
  menuEl.value?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')?.focus();
}
function closeMenu(refocus = false): void {
  menu.value = null;
  if (refocus === true) menuAnchor.value?.focus();
}
/** Run a menu action, then close the menu. */
function run(action: (row: CollectionRow) => void): void {
  const row = menuRow.value;
  closeMenu();
  if (row) action(row);
}
function onMenuKey(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    closeMenu(true);
    return;
  }
  const items = [
    ...(menuEl.value?.querySelectorAll<HTMLElement>(
      '[role="menuitem"]:not(:disabled), [role="menuitemradio"]',
    ) ?? []),
  ];
  const at = items.indexOf(document.activeElement as HTMLElement);
  const go = (i: number) => {
    e.preventDefault();
    items[(i + items.length) % items.length]?.focus();
  };
  // The swatches are a row: left and right move along it, in the reading direction.
  const swatch = (e.target as HTMLElement).getAttribute('role') === 'menuitemradio';
  const rtl = swatch && getComputedStyle(e.target as HTMLElement).direction === 'rtl';
  const next = swatch && e.key === (rtl ? 'ArrowLeft' : 'ArrowRight');
  const previous = swatch && e.key === (rtl ? 'ArrowRight' : 'ArrowLeft');
  if (e.key === 'ArrowDown' || next) go(at + 1);
  else if (e.key === 'ArrowUp' || previous) go(at - 1);
  else if (e.key === 'Home') go(0);
  else if (e.key === 'End') go(items.length - 1);
  else if (e.key === 'Tab') closeMenu();
}

function countLabel(n: number): string {
  return t('collections.fontCount', { count: n });
}

const { folded, toggle } = useFolded('collections');
</script>

<template>
  <section class="side-section" aria-labelledby="fm-collections-heading">
    <SidebarHead
      id="fm-collections-heading"
      :folded="folded"
      controls="fm-collections-list"
      @toggle="toggle"
    >
      {{ t('collections.heading') }}
      <template #actions>
        <UiIconButton
          icon="plus"
          :label="t('collections.new')"
          :title="t('collections.newTitle')"
          @click="startEdit(null, null)"
        />
        <UiIconButton
          icon="sparkles"
          :label="t('collections.newSmartLabel')"
          :title="t('collections.newSmartTitle')"
          @click="ruleEditor.openNew()"
        />
      </template>
    </SidebarHead>
    <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
    <ul
      v-show="!folded"
      id="fm-collections-list"
      class="side-list"
      aria-labelledby="fm-collections-heading"
    >
      <li v-if="!store.rows.length && !editing" class="side-quiet" data-testid="collections-empty">
        {{ t('collections.empty') }}
      </li>
      <li v-if="editing && editing.id == null && editing.parent == null" class="item">
        <form class="edit" @submit.prevent="commit">
          <input
            ref="input"
            v-model="editing.name"
            :aria-label="t('collections.newName')"
            @keydown.esc="editing = null"
          />
        </form>
      </li>
      <template v-for="row in store.rows" :key="row.collection.id">
        <li
          class="item"
          :class="{ [`drop-${target?.zone}`]: target?.id === row.collection.id }"
          :style="{ paddingInlineStart: `${row.depth * 14}px` }"
          :data-collection="row.collection.id"
          :data-depth="row.depth"
          :aria-label="row.collection.name"
          draggable="true"
          @dragstart="onDragStart(row, $event)"
          @dragover="onDragOver(row, $event)"
          @dragleave="target = null"
          @drop="onDrop(row, $event)"
          @dragend="onDragEnd"
        >
          <form
            v-if="editing && editing.id === row.collection.id"
            class="edit"
            @submit.prevent="commit"
          >
            <input
              ref="input"
              v-model="editing.name"
              :aria-label="t('collections.renameLabel', { name: row.collection.name })"
              @keydown.esc="editing = null"
            />
          </form>
          <div
            v-else
            class="side-row"
            :class="{
              current: listShown && store.openId === row.collection.id,
              'smart-row': row.collection.rules,
            }"
          >
            <span
              v-if="row.collection.rules"
              class="side-lead smart"
              data-testid="collection-smart"
              :title="t('collections.smart')"
              :style="
                row.collection.color ? { color: collectionColorVar(row.collection.color) } : {}
              "
            >
              <UiIcon name="star" /><span class="sr-only">{{ t('collections.smart') }}</span>
            </span>
            <span
              v-else-if="row.collection.color"
              class="side-lead dot"
              data-testid="collection-dot"
              :style="{ backgroundColor: collectionColorVar(row.collection.color) }"
              aria-hidden="true"
            ></span>
            <span v-else class="side-lead" aria-hidden="true"></span>
            <button
              type="button"
              class="side-label"
              :aria-current="listShown && store.openId === row.collection.id ? 'true' : undefined"
              @click="store.show(row.collection.id)"
            >
              {{ row.collection.name }}
            </button>
            <span class="side-count count" :aria-label="countLabel(row.collection.faceCount)">{{
              formatNumber(row.collection.faceCount)
            }}</span>
            <span class="side-actions">
              <UiIconButton
                class="more"
                icon="more"
                :label="t('collections.moreLabel', { name: row.collection.name })"
                aria-haspopup="menu"
                :aria-expanded="menu?.id === row.collection.id ? 'true' : 'false'"
                @click="openMenu(row, $event)"
              />
            </span>
          </div>
        </li>
        <li
          v-if="editing && editing.id == null && editing.parent === row.collection.id"
          class="item"
          :style="{ paddingInlineStart: `${14 + row.depth * 14}px` }"
        >
          <form class="edit" @submit.prevent="commit">
            <input
              ref="input"
              v-model="editing.name"
              :aria-label="t('collections.newNameInside', { name: row.collection.name })"
              @keydown.esc="editing = null"
            />
          </form>
        </li>
      </template>
    </ul>
    <UiPopover :open="menu !== null" :anchor="menuAnchor" @close="closeMenu">
      <div
        v-if="menuRow"
        ref="menuEl"
        class="menu"
        role="menu"
        :aria-label="t('collections.moreLabel', { name: menuRow.collection.name })"
        @keydown="onMenuKey"
      >
        <button
          role="menuitem"
          type="button"
          :aria-label="t('collections.newInsideLabel', { name: menuRow.collection.name })"
          @click="run((r) => startEdit(null, r.collection.id))"
        >
          {{ t('collections.newInside') }}
        </button>
        <button
          v-if="menuRow.collection.rules"
          role="menuitem"
          type="button"
          :aria-label="t('collections.editRulesLabel', { name: menuRow.collection.name })"
          @click="run((r) => ruleEditor.openEdit(r.collection.id))"
        >
          {{ t('collections.editRules') }}
        </button>
        <button
          role="menuitem"
          type="button"
          :aria-label="t('collections.renameActionLabel', { name: menuRow.collection.name })"
          @click="run((r) => startEdit(r.collection.id, r.collection.parent, r.collection.name))"
        >
          {{ t('common.rename') }}
        </button>
        <div
          class="swatches"
          role="group"
          :aria-label="t('collections.colorLabel', { name: menuRow.collection.name })"
        >
          <button
            v-for="key in COLLECTION_COLORS"
            :key="key"
            role="menuitemradio"
            type="button"
            class="swatch"
            :aria-checked="menuRow.collection.color === key ? 'true' : 'false'"
            :aria-label="t(`collections.color.${key}`)"
            :title="t(`collections.color.${key}`)"
            :style="{ backgroundColor: collectionColorVar(key) }"
            @click="pickColor(key)"
          ></button>
        </div>
        <button
          role="menuitem"
          type="button"
          :aria-label="t('collections.upLabel', { name: menuRow.collection.name })"
          :disabled="menuRow.position === 0"
          @click="run((r) => shift(r, -1))"
        >
          {{ t('collections.up') }}
        </button>
        <button
          role="menuitem"
          type="button"
          :aria-label="t('collections.downLabel', { name: menuRow.collection.name })"
          :disabled="menuRow.position === menuRow.siblings - 1"
          @click="run((r) => shift(r, 1))"
        >
          {{ t('collections.down') }}
        </button>
        <button
          v-if="menuRow.position > 0"
          role="menuitem"
          type="button"
          :aria-label="t('collections.inLabel', { name: menuRow.collection.name })"
          :title="t('collections.inTitle')"
          @click="run((r) => moveIn(r))"
        >
          {{ t('collections.in') }}
        </button>
        <button
          v-if="menuRow.collection.parent != null"
          role="menuitem"
          type="button"
          :aria-label="t('collections.outLabel', { name: menuRow.collection.name })"
          @click="run((r) => moveOut(r))"
        >
          {{ t('collections.out') }}
        </button>
        <button
          role="menuitem"
          type="button"
          :aria-label="t('collections.exportLabel', { name: menuRow.collection.name })"
          :title="t('collections.exportTitle')"
          @click="run((r) => store.exportLicenseInventory(r.collection.id))"
        >
          {{ t('collections.exportLicenseInventory') }}
        </button>
        <button
          role="menuitem"
          type="button"
          :aria-label="t('collections.deleteLabel', { name: menuRow.collection.name })"
          :title="t('collections.deleteTitle')"
          @click="run((r) => store.remove(r.collection.id))"
        >
          {{ t('common.delete') }}
        </button>
      </div>
    </UiPopover>
  </section>
</template>

<style scoped>
.item {
  border-radius: var(--fm-radius-sm);
}
.drop-inside {
  outline: 2px solid var(--fm-accent);
}
.drop-before {
  box-shadow: inset 0 2px 0 var(--fm-accent);
}
.drop-after {
  box-shadow: inset 0 -2px 0 var(--fm-accent);
}
/* A smart collection fills itself: its name reads quieter than the user's own. */
.smart-row .side-label {
  color: var(--fm-muted);
}
.smart-row.current .side-label {
  color: var(--fm-fg);
}
.dot {
  /* The 8 px dot, centered in the 16 px first column. */
  width: 8px;
  height: 8px;
  margin-inline: 4px;
  border-radius: 50%;
}
.menu {
  display: flex;
  flex-direction: column;
  min-width: 180px;
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
.swatches {
  display: flex;
  gap: var(--fm-space-1);
  padding: var(--fm-space-1) var(--fm-space-2);
}
/* The fill is the colour; the ring, outside it, shows the chosen one and the focus. */
.menu button.swatch {
  flex: none;
  width: 24px;
  height: 24px;
  padding: 0;
  border-radius: 50%;
}
.menu button.swatch[aria-checked='true'] {
  outline: 2px solid var(--fm-fg);
  outline-offset: 2px;
}
.menu button.swatch:focus-visible {
  outline-offset: 2px;
}
.edit input {
  box-sizing: border-box;
  width: 100%;
  padding: 2px 6px;
  border: 1px solid var(--fm-accent);
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: inherit;
  font: inherit;
}
.edit {
  padding-block: 2px;
}
.edit input {
  box-sizing: border-box;
  width: 100%;
  padding: 2px 6px;
  border: 1px solid var(--fm-accent);
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: inherit;
  font: inherit;
}
.error {
  margin: 4px 8px;
  color: var(--fm-danger);
}
</style>
