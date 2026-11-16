import { defineStore } from 'pinia';
import { useClient, type Unsubscribe } from '../api/client';
import type { Event, FaceId, SourceId } from '../api/types';
import { t } from '../i18n';
import { useSearchStore } from './search';
import { useSourcesStore } from './sources';
import { useToastStore } from './toast';

/** A folder's own name, as the Sources cards show it. */
function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

/**
 * The quiet notice of the folder watcher (E51.2): when the watcher adds fonts to a source,
 * a toast in the toast corner says "3 fonts added from Fonts" with Show, which filters the
 * wall to them. Removals and every other library change show nothing; the lists update
 * themselves from the same event (the library and sources stores).
 */
export const useWatchNoticeStore = defineStore('watchNotice', () => {
  let unsubscribe: Promise<Unsubscribe> | undefined;
  /** Opens the font wall (the app's router, given by `start`). */
  let openWall: () => unknown = () => undefined;

  /** The folder name of `source`: from the sources store when it has it, else asked. */
  async function sourceName(source: SourceId | null): Promise<string> {
    if (source === null) return t('watcher.unknownSource');
    const known = useSourcesStore().sources.find((s) => s.id === source);
    if (known) return folderName(known.path);
    try {
      const found = (await useClient().listSources()).find((s) => s.id === source);
      if (found) return folderName(found.path);
    } catch {
      // The notice still comes, without the folder's name.
    }
    return t('watcher.unknownSource');
  }

  /** "Show": the wall lists only the added fonts, under a chip that takes the filter off. */
  function show(faces: readonly FaceId[], name: string): void {
    useSearchStore().showOnly(faces, t('watcher.chip', { source: name, count: faces.length }));
    void openWall();
  }

  async function notify(source: SourceId | null, added: readonly FaceId[]): Promise<void> {
    const name = await sourceName(source);
    const faces = [...added];
    useToastStore().show(t('watcher.added', { count: faces.length, source: name }), {
      label: t('watcher.show'),
      run: () => show(faces, name),
    });
  }

  function onEvent(event: Event): void {
    if (event.type !== 'libraryChanged' || !event.fromWatcher || !event.added.length) return;
    void notify(event.source, event.added);
  }

  /** Listens to the watcher's library changes (once); `open` brings the font wall up. */
  async function start(open: () => unknown): Promise<void> {
    openWall = open;
    unsubscribe ??= useClient().onEvent(onEvent);
    await unsubscribe.catch(() => undefined);
  }

  /** Stops listening. */
  async function stop(): Promise<void> {
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  return { start, stop, show };
});
