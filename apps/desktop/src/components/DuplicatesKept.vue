<script setup lang="ts">
// "Hidden copies" and "Remembered decisions" of the Duplicates module (E56.15): what E56.8
// stores, as the API lists it. A hidden copy stays where it is and is out of the library;
// "Show again" brings it back (`showHiddenCopies`, which takes its merge back as an undo
// does). A remembered answer ("keep both", "not duplicates") keeps its set out of the
// queue; "Take back" lists the set again (`takeBackDuplicateDecision`).
import { computed, onMounted } from 'vue';
import type { DuplicateDecision, HiddenCopy } from '../api/types';
import { formatDate, t } from '../i18n';
import { useCleanupStore } from '../stores/cleanup';
import UiBadge from './ui/UiBadge.vue';
import UiButton from './ui/UiButton.vue';
import UiEmptyState from './ui/UiEmptyState.vue';

const props = defineProps<{ kind: 'hidden' | 'decisions' }>();

const cleanup = useCleanupStore();
onMounted(() => (props.kind === 'hidden' ? cleanup.loadHidden() : cleanup.loadDecisions()));

const name = (path: string) => path.split(/[\\/]/).pop() ?? path;
const hidden = computed<HiddenCopy[]>(() => cleanup.hidden ?? []);
const decisions = computed<DuplicateDecision[]>(() => cleanup.decisions ?? []);
/** A set id without its kind: "name:Inter-Regular" reads as "Inter-Regular". */
const setName = (id: string) => id.replace(/^(name|identical):/, '');
</script>

<template>
  <div v-if="props.kind === 'hidden'" class="page" data-testid="duplicates-hidden">
    <h2 class="heading">{{ t('duplicatesModule.nav.hidden') }}</h2>
    <ul v-if="hidden.length" class="rows">
      <li v-for="h in hidden" :key="h.path" class="row">
        <span class="text">
          <span>{{ h.family ?? name(h.path) }}</span>
          <code class="path"
            ><bdi>{{ h.path }}</bdi></code
          >
          <span class="muted small">
            {{ t('duplicatesModule.hidden.at', { date: formatDate(Date.parse(h.at)) }) }}
            <template v-if="h.kept">
              · {{ t('duplicatesModule.hidden.kept', { file: name(h.kept) }) }}</template
            >
          </span>
          <span v-if="!h.inEffect" class="muted small">{{
            t('duplicatesModule.hidden.otherFile')
          }}</span>
        </span>
        <UiButton
          class="outlined"
          :aria-label="t('duplicatesModule.hidden.showLabel', { file: name(h.path) })"
          @click="cleanup.showHidden(h.path)"
        >
          {{ t('duplicatesModule.hidden.show') }}
        </UiButton>
      </li>
    </ul>
    <UiEmptyState
      v-else-if="cleanup.hidden"
      :title="t('duplicatesModule.hidden.empty')"
      :body="t('duplicatesModule.hidden.body')"
    />
  </div>

  <div v-else class="page" data-testid="duplicates-decisions">
    <h2 class="heading">{{ t('duplicatesModule.nav.decisions') }}</h2>
    <ul v-if="decisions.length" class="rows">
      <li v-for="d in decisions" :key="d.id" class="row">
        <span class="text">
          <span>
            {{ setName(d.set) }}
            <UiBadge>{{ t(`duplicatesModule.decisions.answer.${d.answer}`) }}</UiBadge>
          </span>
          <span class="muted small">
            {{ formatDate(Date.parse(d.at)) }} ·
            {{ t('duplicatesModule.history.files', { count: d.files.length }) }}
          </span>
          <span v-if="d.outdated" class="muted small">{{
            t('duplicatesModule.decisions.outdated')
          }}</span>
        </span>
        <UiButton
          class="outlined"
          :aria-label="t('duplicatesModule.decisions.takeBackLabel', { name: setName(d.set) })"
          @click="cleanup.takeBackDecision(d.id)"
        >
          {{ t('duplicatesModule.plan.takeBack') }}
        </UiButton>
      </li>
    </ul>
    <UiEmptyState
      v-else-if="cleanup.decisions"
      :title="t('duplicatesModule.decisions.empty')"
      :body="t('duplicatesModule.decisions.body')"
    />
  </div>
</template>

<style scoped>
.page {
  max-width: 960px;
  padding: var(--fm-space-6) var(--fm-space-8);
}
.heading {
  margin: 0 0 var(--fm-space-4);
  font-size: var(--fm-text-xl);
}
.muted {
  color: var(--fm-muted);
}
.small {
  font-size: var(--fm-text-sm);
}
.rows {
  margin: 0;
  padding: 0;
  list-style: none;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
}
.rows > li + li {
  border-top: 1px solid var(--fm-border);
}
.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-4);
  padding: var(--fm-space-3) var(--fm-space-4);
}
.text {
  display: grid;
  gap: 2px;
  min-width: 0;
}
.path {
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-sm);
  overflow-wrap: anywhere;
}
.outlined {
  height: 32px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  color: var(--fm-fg);
}
</style>
