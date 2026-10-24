// Registers the font files of list rows with the webview, one CSSOM
// `FontFace` (the script form of `@font-face`) per face, loaded from the
// app's font URI scheme by face id. Rows acquire a face while they are on
// screen and release it when they scroll away; a released face stays loaded
// for a while (a short scroll back is free) and is unloaded from
// `document.fonts` once more than `maxIdle` released faces pile up, so a long
// scroll does not leak faces. A catalog family's preview (E22.2) goes through
// the same loader by its own family name and URL (`acquireSource`).
import { catalogFontPath } from '../api/catalogFont';
import { useClient } from '../api/client';
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
  /** Makes a face (`new FontFace(family, source)`). */
  create(family: string, source: string): F;
  /** The URL of a face's font file. */
  url(face: FaceId): string;
  /** Released faces kept loaded before the oldest is unloaded. */
  maxIdle?: number;
}

/** Released faces kept loaded by default: about two screens of rows. */
export const DEFAULT_MAX_IDLE = 48;

/** The CSS `font-family` name a face is registered under. */
export function faceFamily(face: FaceId): string {
  return `fm-face-${face}`;
}

interface Entry<F> {
  font: F;
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
      const font = this.deps.create(family, `url("${url()}")`);
      this.deps.fonts.add(font);
      entry = { font, refs: 0, ready: font.load().then(() => family) };
      // A failed load is reported to the rows that asked; keep the rejection
      // from surfacing as unhandled when none is waiting any more.
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

  private unload(family: string): void {
    const entry = this.entries.get(family);
    this.idle.delete(family);
    if (!entry) return;
    this.deps.fonts.delete(entry.font);
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
