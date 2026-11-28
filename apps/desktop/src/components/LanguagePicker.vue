<script setup lang="ts">
// "Add a language" (E58.2): the searchable picker over `list_languages`, in the API's order,
// without the languages already chosen. It only picks; the caller saves. Moved here from the
// Duplicates rules (E56.14), which no longer edit the languages; Settings › Languages uses it.
import { computed } from 'vue';
import type { LanguagePreset } from '../api/types';
import { t } from '../i18n';
import UiSelect from './ui/UiSelect.vue';

const props = defineProps<{
  /** The languages to offer, as `list_languages` answers them. */
  presets: readonly LanguagePreset[];
  /** Codes already chosen: they are not offered again. */
  chosen: readonly string[];
}>();
const emit = defineEmits<{ add: [code: string] }>();

const options = computed(() =>
  props.presets
    .filter((p) => !props.chosen.includes(p.code))
    .map((p) => ({ value: p.code, label: p.name })),
);
</script>

<template>
  <UiSelect
    :model-value="null"
    :options="options"
    :placeholder="t('languagePicker.add')"
    :aria-label="t('languagePicker.add')"
    :fit="false"
    :disabled="!options.length"
    searchable
    data-testid="language-picker"
    @update:model-value="$event && emit('add', $event as string)"
  />
</template>
