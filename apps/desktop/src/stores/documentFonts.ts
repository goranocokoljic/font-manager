import { defineStore } from 'pinia';
import { ref, shallowRef } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type { DocumentFontsCheck, Event } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useActivationStore } from './activation';
import { useToastStore } from './toast';

/**
 * "Check a document's fonts…" (E32.2): the document the user picked and fm-api's answer
 * for it, each font active, in the library or missing. The statuses come from
 * `checkDocumentFonts` only; after an `activationChanged` event the panel asks again, so
 * the rows follow the activation. A file that cannot be read is a toast and no panel.
 */
export const useDocumentFontsStore = defineStore('documentFonts', () => {
  const open = ref(false);
  /** The document the panel shows. */
  const path = ref<string | null>(null);
  const report = shallowRef<DocumentFontsCheck | null>(null);
  const loading = ref(false);
  /** Bumped by every check and by `close`; an answer to an older one is dropped. */
  let request = 0;
  let unsubscribe: Promise<Unsubscribe> | undefined;

  function onEvent(event: Event): void {
    if (event.type === 'activationChanged' && open.value && path.value)
      void check(path.value, false);
  }

  /**
   * Asks fm-api for the fonts of `doc`. A new document that fails is a toast with the
   * plain-word reason and no panel; a re-check that fails keeps the rows it has.
   */
  async function check(doc: string, fresh: boolean): Promise<void> {
    const mine = ++request;
    if (fresh) loading.value = true;
    try {
      const got = await useClient().checkDocumentFonts({ path: doc });
      if (mine !== request) return;
      path.value = doc;
      report.value = got;
      open.value = true;
      unsubscribe ??= useClient().onEvent(onEvent);
    } catch (e) {
      if (mine !== request) return;
      if (fresh) {
        open.value = false;
        report.value = null;
        path.value = null;
      }
      useToastStore().show(errorMessage(e));
    } finally {
      if (mine === request) loading.value = false;
    }
  }

  /** The native file picker, then the check; a cancel changes nothing. */
  async function choose(): Promise<void> {
    let picked: string | null;
    try {
      picked = await useClient().pickDocument(
        t('documentFonts.pickTitle'),
        t('documentFonts.filter'),
      );
    } catch (e) {
      useToastStore().show(errorMessage(e));
      return;
    }
    if (picked) await check(picked, true);
  }

  /** "Activate N fonts": the in-library faces, for the session, through the activation call. */
  async function activate(): Promise<void> {
    const faces = report.value?.toActivate ?? [];
    if (faces.length) await useActivationStore().activateFaces(faces, 'session');
  }

  async function close(): Promise<void> {
    request++;
    open.value = false;
    loading.value = false;
    report.value = null;
    path.value = null;
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  return { open, path, report, loading, choose, check, activate, close };
});
