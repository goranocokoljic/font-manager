// A font file's bytes from the app's font scheme (`fontUrl`, `catalogFontUrl`),
// for the preview loader (E35.6): a `FontFace` made from an `ArrayBuffer`
// keeps its bytes on the JS heap, where the collector counts them and frees
// them once the face is dropped.

/** Fetches the font file at `url`; rejects on a network error or a non-2xx answer. */
export async function fetchFontFile(url: string): Promise<ArrayBuffer> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Font file ${url}: HTTP ${response.status}`);
  return response.arrayBuffer();
}
