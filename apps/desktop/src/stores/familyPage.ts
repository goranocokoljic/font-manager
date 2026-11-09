import { defineStore } from 'pinia';
import { computed, ref, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type {
  ActivationMode,
  ApiError,
  FaceId,
  FaceSummary,
  Family,
  FamilyId,
  FamilySummary,
  VirtualStyle,
} from '../api/types';
import { errorMessage, t } from '../i18n';
import { faceKey, familyKey, type FaceRow, type FamilyRow } from '../list/rows';
import { LEADING, MEASURE } from '../preview/readingTest';
import { useActivationStore } from './activation';
import { useFontListStore } from './fontList';
import { useLibraryStore } from './library';
import { parseFaceId, useSelectionStore } from './selection';
import { useToastStore } from './toast';
import { useVirtualStylesStore } from './virtualStyles';

/** A family id from a route param; family and face ids have the same form. */
export const parseFamilyId: (value: unknown) => FamilyId | null = parseFaceId;

/** The sections of the page, in the order of the switch. */
export const FAMILY_SECTIONS = [
  'styles',
  'text',
  'characters',
  'features',
  'pairings',
  'info',
] as const;
export type FamilySection = (typeof FAMILY_SECTIONS)[number];

/** The Styles section's list of named styles; any other view is the tag of a ramped axis. */
export const NAMED_STYLES = 'named';

/** The ground of the Text section: dark on light, or light on dark. */
export type TextGround = 'paper' | 'ink';

/**
 * The family page (E42): which family it shows, its place in the list the user came from
 * and what its top bar acts on. The order is the font list's, so the arrows follow the
 * current search and filters. The chosen section stays while the arrows change the family.
 */
export const useFamilyPageStore = defineStore('familyPage', () => {
  const fontList = useFontListStore();
  const library = useLibraryStore();

  const id = ref<FamilyId | null>(null);
  const detail = shallowRef<Family | null>(null);
  const error = ref<ApiError | null>(null);
  const favorite = ref(false);
  const section = ref<FamilySection>('styles');
  /** What the Styles section lists: the named styles, or the ramp of the axis with this tag. */
  const stylesView = ref<string>(NAMED_STYLES);
  /** The Text section's settings: they stay while the arrows change the family. */
  const textMeasure = ref<number>(MEASURE.default);
  const textLeading = ref<number>(LEADING.default);
  const textGround = ref<TextGround>('paper');
  /** Set by a push from the library; the page that mounts takes it to know how it came in. */
  const pushed = ref(false);
  /** Bumped by every read; an answer to an older one is dropped. */
  let request = 0;
  let pinRequest = 0;
  /** The read `show` started last. It never rejects: a failed read is `error`. */
  let reading: Promise<void> = Promise.resolve();

  function takePushed(): boolean {
    const was = pushed.value;
    pushed.value = false;
    return was;
  }

  /** The family's place in the list; -1 when a search or filter hides it (or a deep link). */
  const index = computed(() => fontList.families.findIndex((f) => f.id === id.value));
  const total = computed(() => fontList.families.length);
  const previous = computed<FamilyId | null>(
    () => (index.value > 0 && fontList.families[index.value - 1]?.id) || null,
  );
  const next = computed<FamilyId | null>(
    () => (index.value >= 0 && fontList.families[index.value + 1]?.id) || null,
  );
  /** The family `by` places from `of` in the list; null at an end or when the list hides `of`. */
  function neighbor(of: FamilyId, by: -1 | 1): FamilyId | null {
    const at = fontList.families.findIndex((f) => f.id === of);
    return (at >= 0 && fontList.families[at + by]?.id) || null;
  }

  /** The list's summary when it has one: activation events keep that one current. */
  const summary = computed<FamilySummary | null>(
    () => fontList.families[index.value] ?? detail.value?.summary ?? null,
  );
  const faces = computed(() => detail.value?.faces ?? []);
  /** The style the route names (`?face=`); one the family does not have counts as none. */
  const chosenFace = ref<FaceId | null>(null);
  /** The face the page previews and its panels describe: the chosen style, else the default. */
  const face = computed(
    () =>
      faces.value.find((f) => f.id === chosenFace.value) ??
      faces.value.find((f) => f.id === detail.value?.defaultFace) ??
      faces.value[0] ??
      null,
  );
  /** The instances pinned from this family, in every collection and set. */
  const pins = shallowRef<VirtualStyle[]>([]);

  /** Reads the family's pins. fm-api lists them per holder, so this asks each holder. */
  async function readPins(): Promise<void> {
    const mine = ++pinRequest;
    const own = new Set(faces.value.filter((f) => f.isVariable).map((f) => f.id));
    if (!own.size) {
      pins.value = [];
      return;
    }
    try {
      const client = useClient();
      const [collections, sets] = await Promise.all([client.listCollections(), client.listSets()]);
      const lists = await Promise.all([
        ...collections
          .filter((c) => !c.rules)
          .map((c) => client.listVirtualStyles({ collection: c.id })),
        ...sets.map((s) => client.listVirtualStyles({ set: s.id })),
      ]);
      if (mine === pinRequest) pins.value = lists.flat().filter((v) => own.has(v.face));
    } catch {
      // Pins are extra rows: without them the family's own styles still show.
      if (mine === pinRequest) pins.value = [];
    }
  }

  async function read(family: FamilyId): Promise<void> {
    const mine = ++request;
    try {
      const got = await useClient().getFamily(family);
      const first = got.faces[0];
      const meta = first ? await useClient().getFace(first.id) : null;
      if (mine !== request) return;
      detail.value = got;
      favorite.value = meta?.favorite ?? false;
      error.value = null;
      void readPins();
    } catch (e) {
      if (mine !== request) return;
      detail.value = null;
      pins.value = [];
      error.value = e as ApiError;
    }
  }

  /** Shows `family`, or the not-found state for `null` (a route that names no family). */
  async function show(family: FamilyId | null): Promise<void> {
    id.value = family;
    detail.value = detail.value?.summary.id === family ? detail.value : null;
    error.value = null;
    if (family === null) {
      request++;
      pinRequest++;
      pins.value = [];
      reading = Promise.resolve();
      return;
    }
    reading = read(family);
    await reading;
  }

  /** Resolves when the family the page shows has been read (or failed), so its page is drawn. */
  function settled(): Promise<void> {
    return reading;
  }

  // A scan or a change elsewhere may have changed or removed the family.
  watch(
    () => library.families,
    () => {
      if (id.value !== null) void read(id.value);
    },
  );

  // A favorite set in the Info section on the first face is the state the top bar shows.
  watch(
    () => useSelectionStore().detail,
    (shown) => {
      if (shown && shown.face.id === faces.value[0]?.id) favorite.value = shown.favorite;
    },
  );

  /** The family a face from elsewhere (a Similar result, a document's font) belongs to. */
  async function familyOf(of: FaceId): Promise<FamilyId | null> {
    try {
      return (await useClient().getFace(of)).face.family;
    } catch (e) {
      useToastStore().show(errorMessage(e));
      return null;
    }
  }

  /** Activates what is inactive, or deactivates a fully active family, as A does on its row. */
  async function toggleActivation(mode?: ActivationMode): Promise<void> {
    const family = summary.value;
    if (!family) return;
    const row: FamilyRow = {
      kind: 'family',
      key: familyKey(family.id),
      family,
      position: index.value + 1,
      expanded: false,
    };
    await useActivationStore().toggleRows([row], mode);
    if (id.value === family.id) await read(family.id);
  }

  /** One style's switch: activates that face in the set mode, or deactivates it. */
  async function toggleFace(target: FaceSummary): Promise<void> {
    const family = summary.value;
    if (!family) return;
    const row: FaceRow = {
      kind: 'face',
      key: faceKey(target.id),
      face: target,
      family,
      position: faces.value.findIndex((f) => f.id === target.id) + 1,
      count: faces.value.length,
    };
    await useActivationStore().toggleRows([row]);
    if (id.value === family.id) await read(family.id);
  }

  /** Deletes a pinned instance from its holder; the face stays. */
  async function deletePin(pin: VirtualStyle): Promise<void> {
    try {
      await useVirtualStylesStore().remove(pin.id);
      await readPins();
      useToastStore().show(t('family.pinDeleted', { name: pin.name }));
    } catch (e) {
      useToastStore().show(errorMessage(e));
    }
  }

  /** Favorites every face of the family, or none. The first face tells the current state. */
  async function toggleFavorite(): Promise<void> {
    const all = faces.value.map((f) => f.id);
    if (!all.length) return;
    const wanted = !favorite.value;
    try {
      await useClient().updateUserMeta({ faces: all, favorite: wanted });
      favorite.value = wanted;
      await useSelectionStore().reload();
    } catch (e) {
      useToastStore().show(errorMessage(e));
    }
  }

  return {
    id,
    detail,
    error,
    favorite,
    section,
    stylesView,
    textMeasure,
    textLeading,
    textGround,
    pushed,
    takePushed,
    index,
    total,
    previous,
    next,
    neighbor,
    summary,
    faces,
    chosenFace,
    face,
    pins,
    readPins,
    show,
    settled,
    familyOf,
    toggleActivation,
    toggleFace,
    deletePin,
    toggleFavorite,
  };
});
