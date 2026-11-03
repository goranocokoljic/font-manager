<script setup lang="ts">
// The Columns menu of the detail list: show or hide each column and move it
// up or down, by keyboard as well as by pointer (header drag is the shortcut;
// this is its single-pointer, keyboard-operable alternative).
import { computed, nextTick, ref, useId } from 'vue';
import UiButton from './ui/UiButton.vue';
import { columnById, type ColumnId } from '../list/columns';
import { t } from '../i18n';
import { useViewsStore } from '../stores/views';

const props = defineProps<{ view: string }>();

const views = useViewsStore();
const uid = useId();
const open = ref(false);
const button = ref<InstanceType<typeof UiButton> | null>(null);
const panel = ref<HTMLElement | null>(null);

const config = computed(() => views.tableFor(props.view));
const items = computed(() =>
  config.value.order.map((id, i) => ({
    ...columnById(id),
    index: i,
    shown: !config.value.hidden.includes(id),
  })),
);

async function toggle(): Promise<void> {
  open.value = !open.value;
  if (open.value) {
    await nextTick();
    panel.value?.querySelector<HTMLElement>('input, button')?.focus();
  }
}

function close(): void {
  open.value = false;
  (button.value?.$el as HTMLElement | undefined)?.focus();
}

/** Moves a column, keeping focus on the same button so repeated presses keep moving it. */
async function move(id: ColumnId, to: number, e: Event): Promise<void> {
  const label = (e.currentTarget as HTMLElement).getAttribute('aria-label');
  views.moveColumn(props.view, id, to);
  await nextTick();
  const again = panel.value?.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`);
  (again && !again.disabled
    ? again
    : panel.value?.querySelector<HTMLElement>(`#${uid}-${id}`)
  )?.focus();
}

function onFocusOut(e: FocusEvent): void {
  const next = e.relatedTarget as Node | null;
  if (next && (panel.value?.contains(next) || button.value?.$el.contains(next))) return;
  open.value = false;
}
</script>

<template>
  <div class="menu" @keydown.esc.stop="close">
    <UiButton ref="button" :aria-expanded="open" :aria-controls="`${uid}-panel`" @click="toggle">
      {{ t('table.columnsMenu.button') }}
    </UiButton>
    <div
      v-if="open"
      :id="`${uid}-panel`"
      ref="panel"
      class="panel"
      role="group"
      :aria-label="t('table.columnsMenu.label')"
      @focusout="onFocusOut"
    >
      <ul>
        <li v-for="item in items" :key="item.id">
          <input
            :id="`${uid}-${item.id}`"
            type="checkbox"
            :checked="item.shown"
            :disabled="!item.hideable"
            @change="views.toggleColumn(view, item.id)"
          />
          <label :for="`${uid}-${item.id}`">{{ item.label }}</label>
          <button
            type="button"
            class="move"
            :aria-label="t('table.columnsMenu.moveUp', { column: item.label })"
            :disabled="item.index === 0"
            @click="move(item.id, item.index - 1, $event)"
          >
            ↑
          </button>
          <button
            type="button"
            class="move"
            :aria-label="t('table.columnsMenu.moveDown', { column: item.label })"
            :disabled="item.index === items.length - 1"
            @click="move(item.id, item.index + 1, $event)"
          >
            ↓
          </button>
        </li>
      </ul>
      <button type="button" class="reset" @click="views.resetColumns(view)">
        {{ t('table.columnsMenu.reset') }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.menu {
  position: relative;
}
.move,
.reset {
  padding: 4px 8px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  background-color: transparent;
  color: inherit;
  font: inherit;
}
.panel {
  position: absolute;
  z-index: var(--fm-z-popover);
  top: calc(100% + 4px);
  inset-inline-end: 0;
  width: 240px;
  padding: 8px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bg);
  box-shadow: var(--fm-shadow-2);
}
ul {
  margin: 0 0 8px;
  padding: 0;
  list-style: none;
}
li {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 0;
}
label {
  flex: 1;
}
.move {
  padding: 0 6px;
}
.move:disabled {
  opacity: 0.4;
}
.reset {
  width: 100%;
}
</style>
