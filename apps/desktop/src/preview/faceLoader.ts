// Registers the font files of list rows with the webview, one CSSOM
// `FontFace` (the script form of `@font-face`) per face, loaded from the
// app's font URI scheme by face id. Rows acquire a face while they are on
// screen and release it when they scroll away; a released face stays loaded
// for a while (a short scroll back is free) and is unloaded from
// `document.fonts` once more than `maxIdle` released faces pile up, so a long
// scroll does not leak faces. A catalog family's preview (E22.2) goes through
// the same loader by its own family name and URL (`acquireSource`).
//
// The app fetches each file into an `ArrayBuffer` and makes the face from
// those bytes (`FaceLoaderDeps.bytes`, E35.6). A face made from a `url()`
// source keeps its font data outside the JS heap, so the collector never
// counts it and a dropped face's bytes stayed in the renderer (about 100 KB
// per face scrolled past, `dev-docs/perf/webview-2026-10-29.md`); bytes in an
// `ArrayBuffer` are on the heap and go with the face.
import { catalogFontPath } from '../api/catalogFont';
import { useClient } from '../api/client';
import { fetchFontFile } from '../api/fontFile';
import type { CatalogId, FaceId } from '../api/types';

/** The parts of a `FontFace` the loader uses. */
export interface FontFaceLike {
  load(): Promise<unknown>;
}

/** The parts of a `FontFaceSet` (`document.fonts`) the loader uses. */
export interface FontFaceSetLike<F extends FontFaceLike = FontFaceLike> {
  add(font: F): unknown;
  delete(font: F): boolean;
}

/** What a loader talks to; tests pass fakes. */
export interface FaceLoaderDeps<F extends FontFaceLike = FontFaceLike> {
  /** Where faces are registered (`document.fonts`). */
  fonts: FontFaceSetLike<F>;
  /** Makes a face from a CSS source (`new FontFace(family, 'url(...)')`). */
  create(family: string, source: string): F;
  /**
   * Loads faces from their file's bytes instead of a `url()` source: the
   * loader fetches the file and makes the face from the buffer. The app
   * always passes this; component tests that do not care leave it out.
   */
  bytes?: FaceBytesDeps<F>;
  /** The URL of a face's font file. */
  url(face: FaceId): string;
  /** Released faces kept loaded before the oldest is unloaded. */
  maxIdle?: number;
}

/** How a loader gets a face's bytes and makes a face from them. */
export interface FaceBytesDeps<F extends FontFaceLike = FontFaceLike> {
  /** The font file at `url` (`fetchFontFile`); rejects when it cannot be read. */
  fetch(url: string): Promise<ArrayBuffer>;
  /** Makes a face from the file's bytes (`new FontFace(family, data)`). */
  create(family: string, data: ArrayBuffer): F;
}

/** Released faces kept loaded by default: about two screens of rows. */
export const DEFAULT_MAX_IDLE = 48;

/** The CSS `font-family` name a face is registered under. */
export function faceFamily(face: FaceId): string {
  return `fm-face-${face}`;
}

interface Entry<F> {
  /** Unset while the file's bytes are on their way. */
  font?: F;
  refs: number;
  ready: Promise<string>;
}

/** The CSS `font-family` name a catalog family's preview is registered under. */
export function catalogFamily(catalog: string, family: string): string {
  return `fm-${catalogFontPath(catalog as CatalogId, family)}`;
}

/** Ref-counted face registration with a bounded set of idle faces. */
export class FaceLoader<F extends FontFaceLike = FontFaceLike> {
  /** By CSS family name. */
  private readonly entries = new Map<string, Entry<F>>();
  /** Released faces, oldest first (a `Map` keeps insertion order). */
  private readonly idle = new Map<string, true>();
  private readonly maxIdle: number;

  constructor(private readonly deps: FaceLoaderDeps<F>) {
    this.maxIdle = Math.max(0, deps.maxIdle ?? DEFAULT_MAX_IDLE);
  }

  /**
   * Registers `face` (once) and resolves with its `font-family` name when the
   * file has loaded; rejects when it cannot load. Every call needs one
   * `release`.
   */
  acquire(face: FaceId): Promise<string> {
    return this.acquireSource(faceFamily(face), () => this.deps.url(face));
  }

  /** One row no longer shows `face`. */
  release(face: FaceId): void {
    this.releaseSource(faceFamily(face));
  }

  /**
   * Registers the file at `url` under the CSS name `family` (once), like `acquire` for a
   * face; a catalog row passes its preview's name and URL. Every call needs one
   * `releaseSource`.
   */
  acquireSource(family: string, url: () => string): Promise<string> {
    let entry = this.entries.get(family);
    if (!entry) {
      entry = this.register(family, url());
      // A failed fetch or load is reported to the rows that asked; keep the
      // rejection from surfacing as unhandled when none is waiting any more.
      entry.ready.catch(() => undefined);
      this.entries.set(family, entry);
    }
    this.idle.delete(family);
    entry.refs += 1;
    return entry.ready;
  }

  /** One row no longer shows the file registered as `family`. */
  releaseSource(family: string): void {
    const entry = this.entries.get(family);
    if (!entry || entry.refs === 0) return;
    entry.refs -= 1;
    if (entry.refs > 0) return;
    this.idle.set(family, true);
    while (this.idle.size > this.maxIdle) {
      const oldest = this.idle.keys().next().value as string;
      this.unload(oldest);
    }
  }

  /** Unloads every face no row uses. */
  trim(): void {
    for (const face of [...this.idle.keys()]) this.unload(face);
  }

  /** Faces registered with the webview right now. */
  get loaded(): number {
    return this.entries.size;
  }

  /** Faces some row uses right now. */
  get inUse(): number {
    return this.entries.size - this.idle.size;
  }

  /** Makes and registers the face of the file at `url`. */
  private register(family: string, url: string): Entry<F> {
    const bytes = this.deps.bytes;
    if (!bytes) {
      const font = this.deps.create(family, `url("${url}")`);
      this.deps.fonts.add(font);
      return { font, refs: 0, ready: font.load().then(() => family) };
    }
    const entry: Entry<F> = { refs: 0, ready: Promise.resolve(family) };
    entry.ready = bytes.fetch(url).then(async (data) => {
      // Unloaded while the bytes were on their way: no row wants the face.
      if (this.entries.get(family) !== entry) return family;
      const font = bytes.create(family, data);
      entry.font = font;
      this.deps.fonts.add(font);
      await font.load();
      return family;
    });
    return entry;
  }

  private unload(family: string): void {
    const entry = this.entries.get(family);
    this.idle.delete(family);
    if (!entry) return;
    if (entry.font) this.deps.fonts.delete(entry.font);
    this.entries.delete(family);
  }
}

let shared: FaceLoader | undefined;

/** The app's loader over `document.fonts` and the active API client. */
export function useFaceLoader(): FaceLoader {
  if (shared) return shared;
  const url = (face: FaceId) => useClient().fontUrl(face);
  shared =
    typeof FontFace === 'function' && typeof document !== 'undefined' && document.fonts
      ? new FaceLoader<FontFace>({
          fonts: document.fonts,
          create: (family, source) => new FontFace(family, source),
          bytes: {
            fetch: fetchFontFile,
            create: (family, data) => new FontFace(family, data),
          },
          url,
        })
      : // No CSS Font Loading API (jsdom): rows fall back to the UI font.
        new FaceLoader({
          fonts: { add: () => undefined, delete: () => true },
          create: () => ({ load: () => Promise.reject(new Error('FontFace is not available')) }),
          url,
        });
  return shared;
}

/** Replaces the shared loader (tests). */
export function setFaceLoader(loader: FaceLoader | undefined): void {
  shared = loader;
}
