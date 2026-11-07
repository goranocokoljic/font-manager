import { defineStore } from 'pinia';
import { ref, shallowRef } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FaceId, PairingRole, PairingSuggestion } from '../api/types';

/** The most cards the Pairings section shows; `suggestPairings` is asked for this many. */
export const FAMILY_PAIRING_LIMIT = 8;

/** The part the page's family plays in the cards: the switch "for headlines | for body". */
export type FamilyPairingRole = Extract<PairingRole, 'headline' | 'body'>;

/** The two faces of one pairing card: which face sets the headline and which the body. */
export interface PairingCard {
  /** The suggestion the card shows. */
  suggestion: PairingSuggestion;
  headline: FaceId;
  body: FaceId;
}

/** The role the suggested faces play: the other part from the family's. */
export function candidateRole(familyRole: FamilyPairingRole): FamilyPairingRole {
  return familyRole === 'headline' ? 'body' : 'headline';
}

/**
 * The family page's Pairings section (E42.7): the faces of other families fm-core's rules pair
 * with the page's face, for the part they play next to it (ADR 0021). The switch sets the
 * family's part; the request asks for faces in the other part, and each card sets the family
 * and the candidate in their parts. The switch stays while the arrows change the family.
 */
export const useFamilyPairingsStore = defineStore('familyPairings', () => {
  const familyRole = ref<FamilyPairingRole>('headline');
  /** The face the suggestions are for; null when the page shows none. */
  const face = ref<FaceId | null>(null);
  const results = shallowRef<PairingSuggestion[]>([]);
  const error = ref<ApiError | null>(null);
  const loading = ref(false);
  /** Bumped by every `load`; an answer to an older one is dropped. */
  let request = 0;

  /** Asks the API for the faces that pair with `id` in the other part; null clears the list. */
  async function load(id: FaceId | null): Promise<void> {
    const mine = ++request;
    face.value = id;
    results.value = [];
    error.value = null;
    loading.value = id !== null;
    if (id === null) return;
    try {
      const got = await useClient().suggestPairings({
        face: id,
        limit: FAMILY_PAIRING_LIMIT,
        role: candidateRole(familyRole.value),
      });
      if (mine === request) results.value = got.slice(0, FAMILY_PAIRING_LIMIT);
    } catch (e) {
      if (mine === request) error.value = e as ApiError;
    } finally {
      if (mine === request) loading.value = false;
    }
  }

  /** Sets the family's part and asks again for the current face. */
  async function setFamilyRole(role: FamilyPairingRole): Promise<void> {
    if (familyRole.value === role) return;
    familyRole.value = role;
    await load(face.value);
  }

  /** The card of a suggestion: the family and the candidate in their parts. */
  function cardOf(suggestion: PairingSuggestion): PairingCard | null {
    const mine = face.value;
    if (mine === null) return null;
    const theirs = suggestion.face.id;
    return familyRole.value === 'headline'
      ? { suggestion, headline: mine, body: theirs }
      : { suggestion, headline: theirs, body: mine };
  }

  return { familyRole, face, results, error, loading, load, setFamilyRole, cardOf };
});
