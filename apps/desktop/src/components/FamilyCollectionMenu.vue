<script setup lang="ts">
// "Add to collection" on the family page: the manual collections in a menu under the icon
// button. A smart collection works its members out from a rule, so it takes no fonts by hand.
// As with UiSelect the button keeps the focus and handles the keys: a combobox over a listbox.
import { computed, ref, useId } from 'vue';
import type { Collection, FamilyId } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useCollectionsStore } from '../stores/collections';
import { useToastStore } from '../stores/toast';
import UiIconButton from './ui/UiIconButton.vue';
import UiPopover from './ui/UiPopover.vue';

const props = defineProps<{ family: FamilyId | null }>();

const collections = useCollectionsStore();
const toast = useToastStore();
const open = ref(false);
const active = ref(0);
const id = useId();
const button = ref<InstanceType<typeof UiIconButton> | null>(null);
const anchor = computed(() => (button.value?.$el as HTMLElement | undefined) ?? null);
const manual = computed(() => collections.collections.filter((c) => c.rules === null));

async function toggle(): Promise<void> {
  if (open.value) {
    open.value = false;
    return;
  }
  await collections.refresh();
  active.value = 0;
  open.value = true;
}

async function add(collection: Collection): Promise<void> {
  open.value = false;
  anchor.value?.focus();
  if (props.family === null) return;
  await collections.add(collection.id, { faces: [], families: [props.family] });
  toast.show(
    collections.error
      ? errorMessage(collections.error)
      : t('family.addedToCollection', { name: collection.name }),
  );
}

/** While the menu is open it owns these keys: the page's arrows and Escape must not act. */
function onKey(e: KeyboardEvent): void {
  if (!open.value) return;
  const count = manual.value.length;
  if (e.key === 'Tab') {
    open.value = false;
    return;
  }
  if (e.key === 'Escape') open.value = false;
  else if (e.key === 'ArrowDown' && count) active.value = (active.value + 1) % count;
  else if (e.key === 'ArrowUp' && count) active.value = (active.value - 1 + count) % count;
  else if (e.key === 'Enter' || e.key === ' ') {
    const picked = manual.value[active.value];
    if (picked) void add(picked);
    else open.value = false;
  } else if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
  e.preventDefault();
  e.stopPropagation();
}
</script>

<template>
  <UiIconButton
    ref="button"
    icon="folderPlus"
    :label="t('family.addToCollection')"
    role="combobox"
    aria-haspopup="listbox"
    :aria-expanded="open"
    :aria-controls="open ? id : undefined"
    :aria-activedescendant="open && manual.length ? `${id}-${active}` : undefined"
    :disabled="family === null"
    @click="toggle"
    @keydown="onKey"
  />
  <UiPopover :open="open" :anchor="anchor" @close="open = false">
    <div :id="id">
      <div v-if="manual.length" role="listbox" :aria-label="t('family.addToCollection')">
        <button
          v-for="(c, i) in manual"
          :id="`${id}-${i}`"
          :key="c.id"
          type="button"
          role="option"
          :aria-selected="i === active"
          class="item"
          :class="{ active: i === active }"
          tabindex="-1"
          @click="add(c)"
        >
          {{ c.name }}
        </button>
      </div>
      <p v-else class="none">{{ t('family.noCollections') }}</p>
    </div>
  </UiPopover>
</template>

<style scoped>
.item {
  display: block;
  width: 100%;
  height: 28px;
  padding: 0 var(--fm-space-2);
  border: 0;
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  text-align: start;
  white-space: nowrap;
}
.item.active {
  background-color: var(--fm-hover);
  box-shadow: inset 2px 0 0 var(--fm-accent);
}
.none {
  margin: 0;
  padding: var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
</style>
