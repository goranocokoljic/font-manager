<script setup lang="ts">
// The Duplicates module's "Languages and rules" page (E56.14). First the languages the user
// works in (a searchable picker over `listLanguages`; each change saves at once through
// `updateSettings`). Then the keeper policy (E56.4, ADR 0027): the language guard stands
// above the rules as always on — it is not a rule and cannot be moved or turned off — and
// the rules follow in order, each turned on or off, reordered by drag or from its grip by
// the keyboard. The format and preferred-folder rules open their own short lists. The path
// rule always ends the list. While the user edits, `previewKeeperPolicy` says what the
// edited policy would do; nothing is saved until Save, and "Back to the default" loads the
// default policy as the API reads an empty one.
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { useClient } from '../api/client';
import type { FontFormat, KeeperPolicy, KeeperPolicyPreview, KeeperRule } from '../api/types';
import { errorMessage, t, type MessageKey } from '../i18n';
import { useDuplicatesStore } from '../stores/duplicates';
import { useSettingsStore } from '../stores/settings';
import UiButton from './ui/UiButton.vue';
import UiIconButton from './ui/UiIconButton.vue';
import UiReorderList from './ui/UiReorderList.vue';
import UiSelect from './ui/UiSelect.vue';
import UiSwitch from './ui/UiSwitch.vue';

const store = useDuplicatesStore();
const settings = useSettingsStore();

// ---- Languages ----
const myLanguages = computed(() => settings.settings?.myLanguages ?? []);
const languagePresets = computed(() =>
  myLanguages.value
    .map((code) => store.presets.find((p) => p.code === code))
    .filter((p) => p != null),
);
const languageOptions = computed(() =>
  store.presets
    .filter((p) => !myLanguages.value.includes(p.code))
    .map((p) => ({ value: p.code, label: p.name })),
);
async function setLanguages(codes: string[]): Promise<void> {
  if (await settings.update({ myLanguages: codes })) {
    await store.load();
    void runPreview();
  }
}

// ---- The policy being edited ----
/** Every rule a user can place, in the order the ADR lists them; `path` always ends. */
const RULES = {
  myLanguages: true,
  myLanguagesLocl: true,
  newestVersion: true,
  format: true,
  variable: true,
  mostGlyphs: true,
  preferredFolder: true,
  libraryFolder: true,
  userMetadata: true,
  activeOrRequested: true,
} satisfies Record<Exclude<KeeperRule, 'path'>, true>;
type PlacedRule = keyof typeof RULES;
const PLACED = Object.keys(RULES) as PlacedRule[];
const FORMATS = {
  otf: true,
  ttf: true,
  otc: true,
  ttc: true,
  woff2: true,
  woff: true,
  type1: true,
  dfont: true,
  bitmap: true,
} satisfies Record<FontFormat, true>;

interface Row {
  rule: PlacedRule;
  on: boolean;
}
const rows = ref<Row[]>([]);
const formatOrder = ref<FontFormat[]>([]);
const folders = ref<string[]>([]);
const openList = ref<PlacedRule | null>(null);

/** The editor from a policy: its rules on, in order, then every other rule off. */
function load(policy: KeeperPolicy): void {
  const on = (policy.rules ?? []).filter((r): r is PlacedRule => r !== 'path');
  rows.value = [
    ...on.map((rule) => ({ rule, on: true })),
    ...PLACED.filter((r) => !on.includes(r)).map((rule) => ({ rule, on: false })),
  ];
  formatOrder.value = [...(policy.formatOrder ?? [])];
  folders.value = [...(policy.preferredFolders ?? [])];
}

/** The policy the editor holds, as it is previewed and saved. */
const draft = computed<Required<KeeperPolicy>>(() => ({
  rules: [...rows.value.filter((r) => r.on).map((r) => r.rule), 'path'],
  formatOrder: [...formatOrder.value],
  preferredFolders: [...folders.value],
}));
const saved = computed<Required<KeeperPolicy> | null>(() => {
  const p = settings.settings?.keeperPolicy;
  return p
    ? {
        rules: p.rules ?? [],
        formatOrder: p.formatOrder ?? [],
        preferredFolders: p.preferredFolders ?? [],
      }
    : null;
});
const dirty = computed(
  () => saved.value !== null && JSON.stringify(draft.value) !== JSON.stringify(saved.value),
);
watch(
  () => settings.settings?.keeperPolicy,
  (p) => {
    // Once: later saves load the answer themselves, and an edit in progress stays.
    if (p && rows.value.length === 0) load(p);
  },
  { immediate: true },
);

const ruleName = (rule: KeeperRule) => t(`duplicatesModule.policy.rule.${rule}` as MessageKey);
const formatName = (f: FontFormat) => t(`info.formats.${f}` as MessageKey);
const hasList = (rule: PlacedRule) => rule === 'format' || rule === 'preferredFolder';

function toggle(rule: PlacedRule): void {
  rows.value = rows.value.map((r) => (r.rule === rule ? { ...r, on: !r.on } : r));
}

const formatOptions = computed(() =>
  (Object.keys(FORMATS) as FontFormat[])
    .filter((f) => !formatOrder.value.includes(f))
    .map((f) => ({ value: f, label: formatName(f) })),
);
async function addFolder(): Promise<void> {
  const picked = await useClient().pickFolder(t('duplicatesModule.policy.folders.pick'));
  if (picked && !folders.value.includes(picked)) folders.value = [...folders.value, picked];
}

// ---- Preview, save, default ----
const preview = ref<KeeperPolicyPreview | null>(null);
const previewError = ref<unknown>(null);
let previewSeq = 0;
async function runPreview(): Promise<void> {
  if (!rows.value.length) return;
  const seq = ++previewSeq;
  try {
    const answer = await useClient().previewKeeperPolicy({ policy: draft.value });
    if (seq !== previewSeq) return;
    preview.value = answer;
    previewError.value = null;
  } catch (e) {
    if (seq === previewSeq) previewError.value = e;
  }
}
let timer: ReturnType<typeof setTimeout> | undefined;
watch(
  () => JSON.stringify(draft.value),
  () => {
    clearTimeout(timer);
    timer = setTimeout(() => void runPreview(), 120);
  },
  { immediate: true },
);
onBeforeUnmount(() => clearTimeout(timer));

const status = ref('');
const saving = ref(false);
async function save(): Promise<void> {
  saving.value = true;
  try {
    if (await settings.update({ keeperPolicy: draft.value })) {
      const p = settings.settings?.keeperPolicy;
      if (p) load(p);
      status.value = t('duplicatesModule.policy.saved');
      await Promise.all([store.load(), store.reload(0)]);
      await runPreview();
    }
  } finally {
    saving.value = false;
  }
}
async function backToDefault(): Promise<void> {
  // An empty policy is the default one as the API reads it; the preview answers it in full.
  try {
    const answer = await useClient().previewKeeperPolicy({ policy: {} });
    if (answer.policy) load(answer.policy);
    status.value = t('duplicatesModule.policy.defaultLoaded');
  } catch (e) {
    previewError.value = e;
  }
}
</script>

<template>
  <div class="page" data-testid="duplicates-rules">
    <h2 class="heading">{{ t('duplicatesModule.rules.heading') }}</h2>
    <p class="lead">{{ t('duplicatesModule.rules.body') }}</p>
    <ul
      v-if="languagePresets.length"
      class="chips"
      :aria-label="t('duplicatesModule.rules.heading')"
    >
      <li v-for="p in languagePresets" :key="p.code" class="language-chip">
        {{ p.name }}
        <UiIconButton
          icon="close"
          size="sm"
          :label="t('duplicatesModule.rules.remove', { name: p.name })"
          @click="setLanguages(myLanguages.filter((c) => c !== p.code))"
        />
      </li>
    </ul>
    <p v-else class="muted">{{ t('duplicatesModule.rules.none') }}</p>
    <UiSelect
      :model-value="null"
      :options="languageOptions"
      :placeholder="t('duplicatesModule.rules.add')"
      :aria-label="t('duplicatesModule.rules.add')"
      :fit="false"
      searchable
      @update:model-value="$event && setLanguages([...myLanguages, $event as string])"
    />

    <section class="policy" aria-labelledby="dup-policy-heading" data-testid="dup-policy">
      <h2 id="dup-policy-heading" class="heading">{{ t('duplicatesModule.policy.heading') }}</h2>
      <p class="lead">{{ t('duplicatesModule.policy.body') }}</p>

      <div class="guard" data-testid="dup-guard">
        <p class="guard-title">
          <strong>{{ t('duplicatesModule.policy.guard.title') }}</strong>
          <span class="badge">{{ t('duplicatesModule.policy.guard.on') }}</span>
        </p>
        <p class="muted">{{ t('duplicatesModule.policy.guard.body') }}</p>
        <p v-if="!myLanguages.length" class="muted small">
          {{ t('duplicatesModule.policy.guard.notSet') }}
        </p>
      </div>

      <UiReorderList
        v-model:items="rows"
        :key-of="(r) => r.rule"
        :label-of="(r) => ruleName(r.rule)"
        :label="t('duplicatesModule.policy.rulesLabel')"
        data-testid="dup-rules"
      >
        <template #default="{ item }">
          <span class="rule-name" :class="{ off: !item.on }">{{ ruleName(item.rule) }}</span>
          <UiButton
            v-if="hasList(item.rule)"
            class="outlined"
            :aria-expanded="openList === item.rule"
            :aria-controls="`dup-list-${item.rule}`"
            @click="openList = openList === item.rule ? null : item.rule"
          >
            {{
              item.rule === 'format'
                ? t('duplicatesModule.policy.formats.open', { count: formatOrder.length })
                : t('duplicatesModule.policy.folders.open', { count: folders.length })
            }}
          </UiButton>
          <UiSwitch
            :checked="item.on"
            :aria-label="t('duplicatesModule.policy.use', { name: ruleName(item.rule) })"
            @click="toggle(item.rule)"
          />
          <div
            v-if="item.rule === 'format' && openList === 'format'"
            :id="`dup-list-${item.rule}`"
            class="sublist"
            data-testid="dup-formats"
          >
            <UiReorderList
              v-model:items="formatOrder"
              :key-of="(f) => f"
              :label-of="formatName"
              :label="t('duplicatesModule.policy.formats.label')"
            >
              <template #default="{ item: f }">
                <span class="rule-name">{{ formatName(f) }}</span>
                <UiIconButton
                  icon="close"
                  size="sm"
                  :label="t('duplicatesModule.policy.formats.remove', { name: formatName(f) })"
                  @click="formatOrder = formatOrder.filter((x) => x !== f)"
                />
              </template>
            </UiReorderList>
            <p class="muted small">{{ t('duplicatesModule.policy.formats.rest') }}</p>
            <UiSelect
              v-if="formatOptions.length"
              :model-value="null"
              :options="formatOptions"
              :placeholder="t('duplicatesModule.policy.formats.add')"
              :aria-label="t('duplicatesModule.policy.formats.add')"
              :fit="false"
              @update:model-value="$event && (formatOrder = [...formatOrder, $event as FontFormat])"
            />
          </div>
          <div
            v-if="item.rule === 'preferredFolder' && openList === 'preferredFolder'"
            :id="`dup-list-${item.rule}`"
            class="sublist"
            data-testid="dup-folders"
          >
            <UiReorderList
              v-if="folders.length"
              v-model:items="folders"
              :key-of="(f) => f"
              :label-of="(f) => f"
              :label="t('duplicatesModule.policy.folders.label')"
            >
              <template #default="{ item: f }">
                <code class="path rule-name"
                  ><bdi>{{ f }}</bdi></code
                >
                <UiIconButton
                  icon="close"
                  size="sm"
                  :label="t('duplicatesModule.policy.folders.remove', { name: f })"
                  @click="folders = folders.filter((x) => x !== f)"
                />
              </template>
            </UiReorderList>
            <p v-else class="muted small">{{ t('duplicatesModule.policy.folders.none') }}</p>
            <UiButton class="outlined" @click="addFolder">
              {{ t('duplicatesModule.policy.folders.add') }}
            </UiButton>
          </div>
        </template>
      </UiReorderList>
      <p class="muted small path-rule">{{ t('duplicatesModule.policy.rule.path') }}</p>

      <p class="preview" aria-live="polite" data-testid="dup-policy-preview">
        <template v-if="previewError">{{ errorMessage(previewError) }}</template>
        <template v-else-if="preview">
          {{ t('duplicatesModule.policy.preview.keeper', { count: preview.withKeeper }) }}
          {{ t('duplicatesModule.policy.preview.keepBoth', { count: preview.keepBoth }) }}
          {{ t('duplicatesModule.policy.preview.changed', { count: preview.changed }) }}
        </template>
      </p>
      <div class="actions">
        <button
          type="button"
          class="primary"
          :disabled="!dirty || saving"
          data-testid="dup-policy-save"
          @click="save"
        >
          {{ t('duplicatesModule.policy.save') }}
        </button>
        <UiButton class="outlined" @click="backToDefault">
          {{ t('duplicatesModule.policy.default') }}
        </UiButton>
        <span v-if="dirty" class="muted small" data-testid="dup-policy-unsaved">
          {{ t('duplicatesModule.policy.unsaved') }}
        </span>
        <span class="sr-only" aria-live="polite">{{ status }}</span>
      </div>
      <p v-if="settings.error" role="alert" class="error">{{ errorMessage(settings.error) }}</p>
    </section>
  </div>
</template>

<style scoped>
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
.muted {
  color: var(--fm-muted);
}
.small {
  font-size: var(--fm-text-sm);
}
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-2);
  margin: 0 0 var(--fm-space-4);
  padding: 0;
  list-style: none;
}
.language-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-1);
  height: 28px;
  padding-block: 0;
  padding-inline: var(--fm-space-3) var(--fm-space-1);
  border: 1px solid var(--fm-control-border);
  border-radius: 999px;
}
.policy {
  margin-top: var(--fm-space-8);
}
.guard {
  margin-bottom: var(--fm-space-4);
  padding: var(--fm-space-3) var(--fm-space-4);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface);
}
.guard p {
  margin: 0;
}
.guard-title {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  margin-bottom: var(--fm-space-1) !important;
}
.badge {
  padding: 0 var(--fm-space-2);
  border: 1px solid var(--fm-accent);
  border-radius: 999px;
  color: var(--fm-fg);
  font-size: var(--fm-text-xs);
}
.rule-name {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}
.rule-name.off {
  color: var(--fm-muted);
  text-decoration: line-through;
}
.sublist {
  display: grid;
  flex-basis: 100%;
  gap: var(--fm-space-2);
  justify-items: start;
  margin: var(--fm-space-1) 0 var(--fm-space-2) var(--fm-space-8);
}
.sublist > :first-child {
  justify-self: stretch;
}
.sublist p {
  margin: 0;
}
.path {
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-sm);
}
.path-rule {
  margin: var(--fm-space-2) 0 var(--fm-space-4) var(--fm-space-3);
}
.preview {
  min-height: 1.5em;
  margin: 0 0 var(--fm-space-3);
}
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-3);
}
.outlined {
  height: 32px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  color: var(--fm-fg);
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
.primary:disabled {
  opacity: 0.5;
}
.error {
  color: var(--fm-danger);
}
</style>
