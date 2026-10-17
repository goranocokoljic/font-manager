import { describe, expect, it } from 'vitest';
import { FaceLoader, faceFamily, type FontFaceSetLike } from './faceLoader';

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
});
