import { afterEach, describe, expect, it, vi } from 'vitest';
import { fetchFontFile } from './fontFile';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('fetchFontFile', () => {
  it("resolves with the file's bytes", async () => {
    const fetch = vi.fn(async () => new Response(new Uint8Array([0, 1, 0, 0]), { status: 200 }));
    vi.stubGlobal('fetch', fetch);
    const data = await fetchFontFile('http://fmfont.localhost/3');
    expect(fetch).toHaveBeenCalledWith('http://fmfont.localhost/3');
    expect([...new Uint8Array(data)]).toEqual([0, 1, 0, 0]);
  });

  it('rejects when the scheme answers with an error status', async () => {
    vi.stubGlobal('fetch', async () => new Response('no such face', { status: 404 }));
    await expect(fetchFontFile('http://fmfont.localhost/9')).rejects.toThrow(
      'Font file http://fmfont.localhost/9: HTTP 404',
    );
  });

  it('rejects when the request fails', async () => {
    vi.stubGlobal('fetch', async () => {
      throw new TypeError('Failed to fetch');
    });
    await expect(fetchFontFile('http://fmfont.localhost/9')).rejects.toThrow('Failed to fetch');
  });
});
