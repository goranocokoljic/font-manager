<script setup lang="ts">
// The Collections group of the sidebar: every manual and smart collection as a
// tree, with its face count. Click one to show it in the list. Make, rename,
// nest (In, Out), reorder (Up, Down) and delete them with the row buttons or by dragging a
// collection onto another (top or bottom edge: before or after it; middle:
// inside it). Drop fonts from the list on a collection to add them; the
// palette has an "Add selected fonts to …" command per collection, and
// "Export license inventory…" for the open one (a row button too). The row
// buttons show on the open collection and under keyboard focus. "New smart" and a
// smart collection's "Edit rules" open the rule editor (E14.1).
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { CollectionId } from '../api/types';
import { errorMessage, t } from '../i18n';
import { isFontDrag, pickOf, readFontDrop } from '../list/drag';
import { useCollectionsStore, type CollectionRow } from '../stores/collections';
import { useCommandsStore } from '../stores/commands';
import { useFontListStore } from '../stores/fontList';
import { useQuickCollectionStore } from '../stores/quickCollection';
import { useRuleEditorStore } from '../stores/ruleEditor';

/** The `DataTransfer` type a collection drag carries its id in. */
const COLLECTION_MIME = 'application/x-fm-collection';

type Zone = 'before' | 'inside' | 'after';

const store = useCollectionsStore();
// While the Quick Collection review is open, no collection is the current one.
const quick = useQuickCollectionStore();
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

function countLabel(n: number): string {
  return t('collections.fontCount', { count: n });
}
</script>

<template>
  <section class="collections" aria-labelledby="fm-collections-heading">
    <header class="head">
      <h2 id="fm-collections-heading" class="heading">{{ t('collections.heading') }}</h2>
      <span class="head-actions">
        <button
          type="button"
          class="add"
          :title="t('collections.newTitle')"
          @click="startEdit(null, null)"
        >
          {{ t('collections.new') }}
        </button>
        <button
          type="button"
          class="add"
          :aria-label="t('collections.newSmartLabel')"
          :title="t('collections.newSmartTitle')"
          @click="ruleEditor.openNew()"
        >
          {{ t('collections.newSmart') }}
        </button>
      </span>
    </header>
    <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
    <ul class="list" aria-labelledby="fm-collections-heading">
      <li class="item">
        <div class="row">
          <button
            type="button"
            class="name"
            :aria-current="!quick.open && store.openId == null ? 'true' : undefined"
            @click="store.show(null)"
          >
            {{ t('collections.allFonts') }}
          </button>
        </div>
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
          :class="{
            open: store.openId === row.collection.id,
            [`drop-${target?.zone}`]: target?.id === row.collection.id,
          }"
          :style="{ paddingInlineStart: `${8 + row.depth * 14}px` }"
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
          <div v-else class="row">
            <button
              type="button"
              class="name"
              :aria-current="!quick.open && store.openId === row.collection.id ? 'true' : undefined"
              @click="store.show(row.collection.id)"
            >
              {{ row.collection.name }}
            </button>
            <span v-if="row.collection.rules" class="badge">{{ t('collections.smart') }}</span>
            <span class="count" :aria-label="countLabel(row.collection.faceCount)">{{
              row.collection.faceCount
            }}</span>
          </div>
          <div class="actions">
            <button
              type="button"
              :aria-label="t('collections.newInsideLabel', { name: row.collection.name })"
              @click="startEdit(null, row.collection.id)"
            >
              {{ t('collections.newInside') }}
            </button>
            <button
              v-if="row.collection.rules"
              type="button"
              :aria-label="t('collections.editRulesLabel', { name: row.collection.name })"
              @click="ruleEditor.openEdit(row.collection.id)"
            >
              {{ t('collections.editRules') }}
            </button>
            <button
              type="button"
              :aria-label="t('collections.renameActionLabel', { name: row.collection.name })"
              @click="startEdit(row.collection.id, row.collection.parent, row.collection.name)"
            >
              {{ t('common.rename') }}
            </button>
            <button
              type="button"
              :aria-label="t('collections.upLabel', { name: row.collection.name })"
              :disabled="row.position === 0"
              @click="shift(row, -1)"
            >
              {{ t('collections.up') }}
            </button>
            <button
              type="button"
              :aria-label="t('collections.downLabel', { name: row.collection.name })"
              :disabled="row.position === row.siblings - 1"
              @click="shift(row, 1)"
            >
              {{ t('collections.down') }}
            </button>
            <button
              v-if="row.position > 0"
              type="button"
              :aria-label="t('collections.inLabel', { name: row.collection.name })"
              :title="t('collections.inTitle')"
              @click="moveIn(row)"
            >
              {{ t('collections.in') }}
            </button>
            <button
              v-if="row.collection.parent != null"
              type="button"
              :aria-label="t('collections.outLabel', { name: row.collection.name })"
              @click="moveOut(row)"
            >
              {{ t('collections.out') }}
            </button>
            <button
              type="button"
              :aria-label="t('collections.exportLabel', { name: row.collection.name })"
              :title="t('collections.exportTitle')"
              @click="store.exportLicenseInventory(row.collection.id)"
            >
              {{ t('collections.exportLicenseInventory') }}
            </button>
            <button
              type="button"
              :aria-label="t('collections.deleteLabel', { name: row.collection.name })"
              :title="t('collections.deleteTitle')"
              @click="store.remove(row.collection.id)"
            >
              {{ t('common.delete') }}
            </button>
          </div>
        </li>
        <li
          v-if="editing && editing.id == null && editing.parent === row.collection.id"
          class="item"
          :style="{ paddingInlineStart: `${22 + row.depth * 14}px` }"
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
  </section>
</template>

<style scoped>
.collections {
  padding: 8px;
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
}
.head-actions {
  display: flex;
  gap: 4px;
}
.heading {
  margin: 0;
  padding: 8px 0;
  color: var(--fm-muted);
  font-size: 11.5px;
  font-weight: 600;
}
.add,
.actions button {
  padding: 2px 6px;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 11.5px;
}
.list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.item {
  padding: 4px 8px;
  border-radius: 6px;
}
.item:hover,
.item:focus-within {
  background: var(--fm-bg);
}
.open {
  background: var(--fm-bg);
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
.row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.name {
  flex: 1;
  min-width: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: start;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.name[aria-current='true'] {
  color: var(--fm-accent);
  font-weight: 600;
}
.badge {
  padding: 0 4px;
  border: 1px solid var(--fm-border);
  border-radius: 4px;
  color: var(--fm-muted);
  font-size: 10.5px;
}
.count {
  color: var(--fm-muted);
  font-variant-numeric: tabular-nums;
}
.actions {
  display: none;
  flex-wrap: wrap;
  gap: 4px;
  padding-top: 4px;
}
/* Shown for the open collection and under keyboard focus, never on hover: rows that
   grow under the pointer would move a drag's target. */
.open .actions,
.item:focus-within .actions {
  display: flex;
}
.edit input {
  box-sizing: border-box;
  width: 100%;
  padding: 2px 6px;
  border: 1px solid var(--fm-accent);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.error {
  margin: 4px 8px;
  color: var(--fm-danger);
}
</style>
