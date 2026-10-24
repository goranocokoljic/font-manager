<script setup lang="ts">
// The smart-collection rule editor (E14.1): "New smart collection" and "Edit rules".
// Each row picks what it tests, then a value from a menu the API filled; rows sit in
// an "all of" or "any of" group, with one level of nested groups. The count is the
// API's answer for the draft rule; Save sends the rule and shows fm-core's message
// when it refuses one. Every control is a native one in tab order; Add and Remove
// move focus to the row they made or to the next control, and Escape cancels. Like
// CompareSheet, the dialog keeps focus and plain keys to itself while it is open:
// Tab cycles inside it, and the list's shortcuts behind it never see a key.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import type { RuleOp } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useRuleEditorStore, type DraftGroup, type DraftItem } from '../stores/ruleEditor';
import SmartRuleRow from './SmartRuleRow.vue';

const editor = useRuleEditorStore();
const panel = ref<HTMLElement | null>(null);
let returnFocus: HTMLElement | null = null;

const OPS: { value: RuleOp; readonly label: string }[] = [
  {
    value: 'and',
    get label() {
      return t('rules.allOf');
    },
  },
  {
    value: 'or',
    get label() {
      return t('rules.anyOf');
    },
  },
];

const title = computed(() =>
  editor.editing ? t('rules.editTitle', { name: editor.editing.name }) : t('rules.newTitle'),
);

/**
 * The rows' names for their labels: "Rule 2", "Group 1, rule 1"; `inline` is the
 * same name inside a sentence ("Remove group 1").
 */
const labels = computed(() => {
  const out = new Map<number, { label: string; inline: string }>();
  let rule = 0;
  let group = 0;
  for (const item of editor.draft.items) {
    if (item.type === 'row') {
      const n = ++rule;
      out.set(item.key, {
        label: t('rules.ruleLabel', { n }),
        inline: t('rules.ruleInline', { n }),
      });
    } else {
      const g = ++group;
      out.set(item.key, {
        label: t('rules.groupLabel', { n: g }),
        inline: t('rules.groupInline', { n: g }),
      });
      item.rows.forEach((r, i) =>
        out.set(r.key, {
          label: t('rules.groupRuleLabel', { group: g, n: i + 1 }),
          inline: t('rules.groupRuleInline', { group: g, n: i + 1 }),
        }),
      );
    }
  }
  return out;
});
const labelOf = (item: DraftItem) => labels.value.get(item.key)?.label ?? t('rules.rule');
const inlineOf = (item: DraftItem) => labels.value.get(item.key)?.inline ?? t('rules.ruleLower');

async function focusKey(key: number): Promise<void> {
  await nextTick();
  panel.value?.querySelector<HTMLElement>(`[data-focus="${key}"]`)?.focus();
}

function addRow(group?: DraftGroup): void {
  void focusKey(editor.addRow(group).key);
}

function addGroup(): void {
  const g = editor.addGroup();
  void focusKey(g.rows[0]!.key);
}

function remove(item: DraftItem, after: string): void {
  editor.remove(item);
  void nextTick(() => panel.value?.querySelector<HTMLElement>(after)?.focus());
}

function listen(on: boolean): void {
  if (on) {
    window.addEventListener('keydown', onOutsideKey, true);
    document.addEventListener('focusin', onFocusIn);
  } else {
    window.removeEventListener('keydown', onOutsideKey, true);
    document.removeEventListener('focusin', onFocusIn);
  }
}
onBeforeUnmount(() => listen(false));

watch(
  () => editor.open,
  async (open) => {
    listen(open);
    if (open) {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      await nextTick();
      panel.value?.querySelector<HTMLElement>('input, select')?.focus();
    } else {
      const target = returnFocus;
      returnFocus = null;
      await nextTick();
      if (target?.isConnected) target.focus();
    }
  },
  { immediate: true },
);

/** Tab cycles inside the dialog; Escape closes it; plain keys stay out of the list's shortcuts. */
function onKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    editor.close();
    return;
  }
  if (!e.ctrlKey && !e.metaKey) e.stopPropagation();
  if (e.key !== 'Tab' || !panel.value) return;
  const stops = [
    ...panel.value.querySelectorAll<HTMLElement>('input, select, button, [tabindex="0"]'),
  ].filter((el) => !el.hasAttribute('disabled'));
  const first = stops[0];
  const last = stops[stops.length - 1];
  if (!first || !last) return;
  if (document.activeElement === panel.value) {
    e.preventDefault();
    (e.shiftKey ? last : first).focus();
  } else if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

/**
 * A key whose target is outside the dialog (focus fell to the page) is handled as if
 * it were in the dialog: the list's shortcuts behind it never see it.
 */
function onOutsideKey(e: KeyboardEvent): void {
  if (!panel.value || (e.target instanceof Node && panel.value.contains(e.target))) return;
  // The command palette may open over the editor; it owns its keys.
  if (e.target instanceof Element && e.target.closest('[role="dialog"]')) return;
  if (e.key === 'Tab') {
    e.preventDefault();
    panel.value.focus();
  }
  onKeyDown(e);
}

/** Focus that moves to the page behind the dialog is taken back to it. */
function onFocusIn(e: FocusEvent): void {
  const t = e.target;
  if (!panel.value || !(t instanceof Element) || panel.value.contains(t)) return;
  if (t.closest('[role="dialog"]')) return;
  panel.value.focus();
}
</script>

<template>
  <div v-if="editor.open" class="backdrop" @mousedown.self="editor.close">
    <div
      ref="panel"
      class="panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="fm-rules-title"
      tabindex="-1"
      @keydown="onKeyDown"
    >
      <h2 id="fm-rules-title" class="title">{{ title }}</h2>
      <form class="body" @submit.prevent="editor.save">
        <label v-if="!editor.editing" class="field">
          <span>{{ t('rules.name') }}</span>
          <input v-model="editor.name" type="text" autocomplete="off" spellcheck="false" />
        </label>
        <label class="field">
          <span>{{ t('rules.fontsMatch') }}</span>
          <select v-model="editor.draft.op" :aria-label="t('rules.opLabel')">
            <option v-for="op in OPS" :key="op.value" :value="op.value">{{ op.label }}</option>
          </select>
          <span>{{ t('rules.theseRules') }}</span>
        </label>

        <ul class="rules" :aria-label="t('rules.rules')">
          <li v-for="item in editor.draft.items" :key="item.key" class="item">
            <SmartRuleRow
              v-if="item.type === 'row'"
              :row="item"
              :label="labelOf(item)"
              :inline="inlineOf(item)"
              @remove="remove(item, '.add-rule')"
            />
            <fieldset v-else class="group" :data-group="labelOf(item)">
              <legend>
                <span>{{ t('rules.groupFontsMatch', { label: labelOf(item) }) }}</span>
                <select
                  v-model="item.op"
                  :aria-label="t('rules.groupOpLabel', { label: labelOf(item) })"
                >
                  <option v-for="op in OPS" :key="op.value" :value="op.value">
                    {{ op.label }}
                  </option>
                </select>
              </legend>
              <ul class="rules" :aria-label="t('rules.groupRules', { label: labelOf(item) })">
                <li v-for="row in item.rows" :key="row.key">
                  <SmartRuleRow
                    :row="row"
                    :label="labelOf(row)"
                    :inline="inlineOf(row)"
                    @remove="remove(row, `[data-add='${item.key}']`)"
                  />
                </li>
              </ul>
              <div class="group-actions">
                <button
                  type="button"
                  class="button"
                  :data-add="item.key"
                  :aria-label="t('rules.addRuleTo', { label: inlineOf(item) })"
                  @click="addRow(item)"
                >
                  {{ t('rules.addRule') }}
                </button>
                <button
                  type="button"
                  class="button"
                  :aria-label="t('rules.removeLabel', { label: inlineOf(item) })"
                  @click="remove(item, '.add-rule')"
                >
                  {{ t('rules.removeGroup') }}
                </button>
              </div>
            </fieldset>
          </li>
        </ul>
        <div class="actions">
          <button type="button" class="button add-rule" @click="addRow()">
            {{ t('rules.addRule') }}
          </button>
          <button type="button" class="button" @click="addGroup">{{ t('rules.addGroup') }}</button>
        </div>

        <p class="count" role="status" aria-live="polite">
          <template v-if="editor.count != null">{{
            t('rules.matches', { count: editor.count })
          }}</template>
        </p>
        <p v-if="editor.incomplete" class="hint">
          {{ t('rules.incomplete', { count: editor.incomplete }) }}
        </p>
        <p v-if="editor.error" role="alert" class="error">{{ errorMessage(editor.error) }}</p>
        <footer class="foot">
          <button type="button" class="button" @click="editor.close">
            {{ t('common.cancel') }}
          </button>
          <button type="submit" class="button primary" :disabled="editor.busy">
            {{ t('common.save') }}
          </button>
        </footer>
      </form>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 10;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 8vh;
  background: rgb(0 0 0 / 20%);
}
.panel {
  display: flex;
  flex-direction: column;
  width: min(640px, 92vw);
  max-height: 84vh;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: var(--fm-bg);
  box-shadow: 0 8px 32px rgb(0 0 0 / 24%);
  overflow: hidden;
}
.panel:focus {
  outline: none;
}
.title {
  margin: 0;
  padding: 12px 16px 4px;
  font-size: 13px;
  font-weight: 600;
}
.body {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px 16px 0;
  overflow: auto;
}
.field {
  display: flex;
  align-items: center;
  gap: 6px;
}
.rules {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.group {
  margin: 0;
  padding: 6px 8px 8px;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
}
.group legend {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 4px;
}
.group-actions,
.actions {
  display: flex;
  gap: 6px;
  padding-top: 6px;
}
select,
input {
  padding: 2px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
select:focus-visible,
input:focus-visible,
.button:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.count {
  margin: 4px 0 0;
  font-variant-numeric: tabular-nums;
}
.hint {
  margin: 0;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
.foot {
  position: sticky;
  bottom: 0;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin: 4px -16px 0;
  padding: 8px 16px 12px;
  border-top: 1px solid var(--fm-border);
  background: var(--fm-bg);
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.button.primary {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
</style>
