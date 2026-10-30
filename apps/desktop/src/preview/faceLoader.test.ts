import { describe, expect, it } from 'vitest';
import { FaceLoader, catalogFamily, faceFamily, type FontFaceSetLike } from './faceLoader';

/** A stand-in for `FontFace`: records its family and source. */
class FakeFace {
  constructor(
    readonly family: string,
    readonly source: string,
    private readonly fail = false,
  ) {}
  load(): Promise<unknown> {
    return this.fail ? Promise.reject(new Error('bad font')) : Promise.resolve(this);
  }
}

/** A stand-in for `document.fonts`. */
class FakeSet implements FontFaceSetLike<FakeFace> {
  readonly faces = new Set<FakeFace>();
  added = 0;
  add(f: FakeFace) {
    this.added += 1;
    this.faces.add(f);
  }
  delete(f: FakeFace) {
    return this.faces.delete(f);
  }
  families() {
    return [...this.faces].map((f) => f.family).sort();
  }
}

function setup(maxIdle = 3, failing: number[] = []) {
  const fonts = new FakeSet();
  const loader = new FaceLoader<FakeFace>({
    fonts,
    create: (family, source) =>
      new FakeFace(
        family,
        source,
        failing.some((id) => family === faceFamily(id)),
      ),
    url: (face) => `fmfont://localhost/${face}`,
    maxIdle,
  });
  return { fonts, loader };
}

describe('FaceLoader', () => {
  it('registers a face once, from its URL, under its own family name', async () => {
    const { fonts, loader } = setup();
    await expect(loader.acquire(12)).resolves.toBe('fm-face-12');
    await loader.acquire(12);
    expect(fonts.added).toBe(1);
    const [face] = fonts.faces;
    expect(face!.family).toBe('fm-face-12');
    expect(face!.source).toBe('url("fmfont://localhost/12")');
  });

  it('keeps a face while any row still uses it', () => {
    const { fonts, loader } = setup(0);
    void loader.acquire(1);
    void loader.acquire(1);
    loader.release(1);
    expect(fonts.families()).toEqual(['fm-face-1']);
    loader.release(1);
    expect(fonts.families()).toEqual([]);
  });

  it('unloads the oldest released faces beyond the idle limit', () => {
    const { fonts, loader } = setup(2);
    for (const id of [1, 2, 3, 4]) void loader.acquire(id);
    for (const id of [1, 2, 3]) loader.release(id);
    expect(fonts.families()).toEqual(['fm-face-2', 'fm-face-3', 'fm-face-4']);
    expect(loader.loaded).toBe(3);
    expect(loader.inUse).toBe(1);
  });

  it('reuses an idle face without loading it again', () => {
    const { fonts, loader } = setup(2);
    void loader.acquire(1);
    loader.release(1);
    void loader.acquire(1);
    expect(fonts.added).toBe(1);
    for (const id of [2, 3, 4]) {
      void loader.acquire(id);
      loader.release(id);
    }
    // Face 1 is in use again, so it is not unloaded with the idle ones.
    expect(fonts.families()).toContain('fm-face-1');
  });

  it('does not leak faces over 1,000 rows scrolled past', () => {
    const { fonts, loader } = setup(48);
    const visible = 12;
    for (let top = 0; top < 1000; top++) {
      void loader.acquire(top + visible);
      if (top === 0) for (let id = 0; id < visible; id++) void loader.acquire(id);
      loader.release(top);
      expect(fonts.faces.size).toBeLessThanOrEqual(visible + 1 + 48);
    }
    expect(fonts.added).toBe(1000 + visible);
    expect(loader.inUse).toBe(visible);
    expect(fonts.faces.size).toBe(visible + 48);
    loader.trim();
    expect(fonts.faces.size).toBe(visible);
  });

  it('reports a face that cannot load and still unloads it', async () => {
    const { fonts, loader } = setup(0, [5]);
    await expect(loader.acquire(5)).rejects.toThrow('bad font');
    loader.release(5);
    expect(fonts.faces.size).toBe(0);
  });

  it('ignores a release without an acquire', () => {
    const { fonts, loader } = setup(0);
    loader.release(9);
    void loader.acquire(9);
    loader.release(9);
    loader.release(9);
    expect(loader.loaded).toBe(0);
    expect(fonts.faces.size).toBe(0);
  });

  it('registers a catalog preview by its own name and URL, apart from the faces', async () => {
    const { fonts, loader } = setup(1);
    const name = catalogFamily('googleFonts', 'Space Mono');
    expect(name).toBe('fm-catalog-googleFonts-5370616365204d6f6e6f');
    const url = () => '/fmfont/catalog-googleFonts-5370616365204d6f6e6f';
    await expect(loader.acquireSource(name, url)).resolves.toBe(name);
    await loader.acquireSource(name, url);
    await loader.acquire(1);
    expect(fonts.added).toBe(2);
    expect([...fonts.faces].find((f) => f.family === name)?.source).toBe(`url("${url()}")`);
    loader.releaseSource(name);
    expect(loader.inUse).toBe(2);
    loader.releaseSource(name);
    loader.release(1);
    // One idle face is kept: the preview, released first, is unloaded.
    expect(fonts.families()).toEqual([faceFamily(1)]);
  });
});

/** A stand-in for `FontFace` made from bytes: records its family and buffer. */
class FakeDataFace {
  constructor(
    readonly family: string,
    readonly data: ArrayBuffer,
  ) {}
  load(): Promise<unknown> {
    return this.data.byteLength === 0
      ? Promise.reject(new Error('bad font'))
      : Promise.resolve(this);
  }
}

function setupBytes(maxIdle = 3, fetch?: (url: string) => Promise<ArrayBuffer>) {
  const fonts = new FakeSet() as unknown as FontFaceSetLike<FakeDataFace> & {
    faces: Set<FakeDataFace>;
    added: number;
    families(): string[];
  };
  const fetched: string[] = [];
  const created: FakeDataFace[] = [];
  const loader = new FaceLoader<FakeDataFace>({
    fonts,
    create: () => {
      throw new Error('a face with bytes must not be made from a url() source');
    },
    bytes: {
      fetch:
        fetch ??
        ((url) => {
          fetched.push(url);
          // Face 0's file is empty: it fetches but does not load.
          return Promise.resolve(new ArrayBuffer(url.endsWith('/0') ? 0 : 8));
        }),
      create: (family, data) => {
        const face = new FakeDataFace(family, data);
        created.push(face);
        return face;
      },
    },
    url: (face) => `fmfont://localhost/${face}`,
    maxIdle,
  });
  return { fonts, loader, fetched, created };
}

describe('FaceLoader from file bytes (E35.6)', () => {
  it("fetches the face's file once and makes the face from its buffer", async () => {
    const { fonts, loader, fetched, created } = setupBytes();
    const first = loader.acquire(12);
    const second = loader.acquire(12);
    await expect(first).resolves.toBe('fm-face-12');
    await expect(second).resolves.toBe('fm-face-12');
    expect(fetched).toEqual(['fmfont://localhost/12']);
    expect(fonts.added).toBe(1);
    expect(created[0]!.family).toBe('fm-face-12');
    expect(created[0]!.data).toBeInstanceOf(ArrayBuffer);
    expect(fonts.families()).toEqual(['fm-face-12']);
  });

  it('reports a file that cannot be fetched the way a failed load is reported', async () => {
    const { fonts, loader } = setupBytes(0, () => Promise.reject(new Error('HTTP 404')));
    await expect(loader.acquire(5)).rejects.toThrow('HTTP 404');
    expect(fonts.faces.size).toBe(0);
    expect(loader.loaded).toBe(1);
    loader.release(5);
    expect(loader.loaded).toBe(0);
  });

  it('reports a fetched file that does not load and still unloads it', async () => {
    const { fonts, loader } = setupBytes(0);
    await expect(loader.acquire(0)).rejects.toThrow('bad font');
    expect(fonts.families()).toEqual(['fm-face-0']);
    loader.release(0);
    expect(fonts.faces.size).toBe(0);
  });

  it('does not register a face that was unloaded while its bytes were on the way', async () => {
    const { fonts, loader } = setupBytes(0);
    const ready = loader.acquire(7);
    loader.release(7);
    await ready;
    expect(fonts.added).toBe(0);
    expect(loader.loaded).toBe(0);
    // Asked for again, it is fetched and registered anew.
    await loader.acquire(7);
    expect(fonts.families()).toEqual(['fm-face-7']);
  });

  it('keeps the idle limit: no face leaks over 1,000 rows scrolled past', async () => {
    const { fonts, loader } = setupBytes(48);
    const visible = 12;
    await Promise.all(Array.from({ length: visible }, (_, i) => loader.acquire(i + 1)));
    for (let top = 1; top <= 1000; top++) {
      await loader.acquire(top + visible);
      loader.release(top);
      expect(fonts.faces.size).toBeLessThanOrEqual(visible + 48);
    }
    expect(fonts.added).toBe(1000 + visible);
    expect(loader.inUse).toBe(visible);
    expect(fonts.faces.size).toBe(visible + 48);
    loader.trim();
    expect(fonts.faces.size).toBe(visible);
  });

  it('loads a catalog preview from the bytes of its own URL', async () => {
    const { loader, fetched } = setupBytes(1);
    const name = catalogFamily('googleFonts', 'Space Mono');
    const url = () => '/fmfont/catalog-googleFonts-5370616365204d6f6e6f';
    await expect(loader.acquireSource(name, url)).resolves.toBe(name);
    expect(fetched).toEqual([url()]);
  });
});
