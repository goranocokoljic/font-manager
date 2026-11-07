<script setup lang="ts">
// The Pairings section of the family page (E42.7): up to eight faces of other families that
// fm-core's rules pair with the page's face, each card a mini layout. The switch sets the part
// the family plays; the request asks for faces in the other part (ADR 0021) and the cards put
// each face in its part. A card opens the Studio with the pairing; with no candidate in the
// library, one line and a way to the catalogs.
import { computed, watch } from 'vue';
import type { FaceId } from '../api/types';
import { errorMessage, t } from '../i18n';
import { readingPassage } from '../preview/readingTest';
import {
  type FamilyPairingRole,
  type PairingCard,
  useFamilyPairingsStore,
} from '../stores/familyPairings';
import FamilyPairingCard from './FamilyPairingCard.vue';
import UiSegmentedControl from './ui/UiSegmentedControl.vue';

const props = defineProps<{
  face: FaceId | null;
  /** The page's preview text: the headline's words. */
  text: string;
}>();
const emit = defineEmits<{ open: [card: PairingCard]; browse: [] }>();

const pairings = useFamilyPairingsStore();
watch(
  () => props.face,
  (face) => void pairings.load(face),
  { immediate: true },
);

const roles = computed(() =>
  (['headline', 'body'] as const).map((value) => ({
    value,
    label: t(`family.pairingRoles.${value}`),
  })),
);
const role = computed<FamilyPairingRole>({
  get: () => pairings.familyRole,
  set: (r) => void pairings.setFamilyRole(r),
});
const cards = computed(() =>
  pairings.results.map((s) => pairings.cardOf(s)).filter((c): c is PairingCard => c !== null),
);
const body = computed(() => readingPassage(props.text));
</script>

<template>
  <div class="family-pairings">
    <div class="head">
      <!-- The switch reads "This family | for headlines | for body"; the group carries the name. -->
      <span class="role-label" aria-hidden="true">{{ t('family.pairingRole') }}</span>
      <UiSegmentedControl v-model="role" :options="roles" :label="t('family.pairingRole')" />
      <p class="hint">{{ t(`family.pairingHints.${role}`) }}</p>
    </div>
    <p v-if="pairings.error" role="alert" class="error">
      {{ t('family.pairingsError', { message: errorMessage(pairings.error) }) }}
    </p>
    <p v-else-if="pairings.loading" class="quiet" aria-live="polite">
      {{ t('family.pairingsLoading') }}
    </p>
    <div v-else-if="!cards.length" class="empty" data-testid="pairings-empty">
      <p class="quiet">{{ t('family.pairingsNone') }}</p>
      <button type="button" class="browse" data-testid="pairings-browse" @click="emit('browse')">
        {{ t('family.browseCatalogs') }}
      </button>
    </div>
    <ul v-else class="cards" :aria-label="t('family.pairingsList')">
      <li v-for="card in cards" :key="card.suggestion.face.id">
        <FamilyPairingCard
          :card="card"
          :headline-text="text"
          :body-text="body"
          @open="emit('open', $event)"
        />
      </li>
    </ul>
  </div>
</template>

<style scoped>
.head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2) var(--fm-space-3);
  margin-block-end: var(--fm-space-3);
}
.role-label {
  color: var(--fm-fg);
  font-size: var(--fm-text-sm);
}
.hint,
.quiet {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
.empty {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-3);
}
.browse {
  padding: var(--fm-space-1) var(--fm-space-3);
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
  background: none;
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  cursor: pointer;
}
.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: var(--fm-space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}
.cards > li {
  display: grid;
}
</style>
