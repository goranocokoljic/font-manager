// Just enough of a PDF reader for the print checks (E24.4): the text Chromium's PDF writer
// put on the pages, and the names of the fonts it embedded. It reads uncompressed object
// syntax with Flate streams (what `page.pdf()` writes); it is not a general PDF parser.
import { inflateSync } from 'node:zlib';

interface PdfObject {
  dict: string;
  stream: Buffer | null;
}

/** Every `n 0 obj ... endobj` of the file, by object number; streams inflated. */
function objects(pdf: Buffer): Map<number, PdfObject> {
  const text = pdf.toString('latin1');
  const out = new Map<number, PdfObject>();
  const head = /(\d+) 0 obj\b/g;
  let m: RegExpExecArray | null;
  while ((m = head.exec(text))) {
    const start = m.index + m[0].length;
    const end = text.indexOf('endobj', start);
    if (end < 0) break;
    const body = text.slice(start, end);
    const at = body.search(/\bstream\r?\n/);
    let dict = body;
    let stream: Buffer | null = null;
    if (at >= 0) {
      dict = body.slice(0, at);
      const dataStart = start + body.indexOf('\n', at) + 1;
      let dataEnd = text.lastIndexOf('endstream', end);
      if (text[dataEnd - 1] === '\n') dataEnd--;
      if (text[dataEnd - 1] === '\r') dataEnd--;
      const raw = pdf.subarray(dataStart, dataEnd);
      try {
        stream = /\/FlateDecode/.test(dict) ? inflateSync(raw) : raw;
      } catch {
        stream = raw;
      }
    }
    out.set(Number(m[1]), { dict, stream });
    head.lastIndex = end;
  }
  return out;
}

/** A ToUnicode CMap: code (hex, as written) to text; and the code width in hex digits. */
function cmapOf(source: string): { map: Map<string, string>; width: number } {
  const map = new Map<string, string>();
  let width = 4;
  const utf16 = (hex: string) =>
    String.fromCharCode(...(hex.match(/.{4}/g) ?? []).map((h) => parseInt(h, 16)));
  for (const block of source.matchAll(/beginbfchar([\s\S]*?)endbfchar/g)) {
    for (const [, src, dst] of block[1]!.matchAll(/<([0-9a-fA-F]+)>\s*<([0-9a-fA-F]+)>/g)) {
      width = src!.length;
      map.set(src!.toUpperCase(), utf16(dst!));
    }
  }
  for (const block of source.matchAll(/beginbfrange([\s\S]*?)endbfrange/g)) {
    const ranges = block[1]!.matchAll(
      /<([0-9a-fA-F]+)>\s*<([0-9a-fA-F]+)>\s*(<[0-9a-fA-F]+>|\[[^\]]*\])/g,
    );
    for (const [, lo, hi, dst] of ranges) {
      width = lo!.length;
      const from = parseInt(lo!, 16);
      const to = parseInt(hi!, 16);
      const list = dst!.startsWith('[') ? [...dst!.matchAll(/<([0-9a-fA-F]+)>/g)] : null;
      for (let c = from; c <= to; c++) {
        const code = c.toString(16).toUpperCase().padStart(lo!.length, '0');
        if (list) {
          const item = list[c - from];
          if (item) map.set(code, utf16(item[1]!));
        } else {
          const base = dst!.slice(1, -1);
          const last = parseInt(base.slice(-4), 16) + (c - from);
          map.set(code, utf16(base.slice(0, -4) + last.toString(16).padStart(4, '0')));
        }
      }
    }
  }
  return { map, width };
}

/** A stretch of page text drawn in one font (`font` is its BaseFont, or '' for none). */
export interface TextRun {
  font: string;
  text: string;
}

/**
 * The text on the pages of `pdf` in content order, as one string and as runs per font,
 * and the names of the embedded font programs.
 */
export function readPdf(pdf: Buffer): { text: string; runs: TextRun[]; fonts: string[] } {
  const objs = objects(pdf);
  const latin1 = (b: Buffer | null) => (b ? b.toString('latin1') : '');

  // Font resource names (`/F3`) to their ToUnicode CMap and BaseFont.
  type FontRes = { cmap: ReturnType<typeof cmapOf>; base: string };
  const cmaps = new Map<string, FontRes>();
  const fonts: string[] = [];
  const fontObjs = new Map<number, FontRes | null>();
  for (const [n, o] of objs) {
    if (!/\/Type\s*\/Font\b/.test(o.dict)) continue;
    const base = /\/BaseFont\s*\/([^\s/<>[\]]+)/.exec(o.dict)?.[1];
    if (base && /\/FontDescriptor|\/DescendantFonts/.test(o.dict)) fonts.push(base);
    const ref = /\/ToUnicode\s+(\d+)\s+0\s+R/.exec(o.dict)?.[1];
    const cmap = ref ? cmapOf(latin1(objs.get(Number(ref))?.stream ?? null)) : null;
    fontObjs.set(n, cmap ? { cmap, base: base ?? '' } : null);
  }
  for (const o of objs.values()) {
    if (/\/FontFile\d?\s+\d+\s+0\s+R/.test(o.dict)) {
      const name = /\/FontName\s*\/([^\s/<>[\]]+)/.exec(o.dict)?.[1];
      if (name) fonts.push(name);
    }
    for (const dict of o.dict.matchAll(/\/Font\s*<<([\s\S]*?)>>/g)) {
      for (const [, name, ref] of dict[1]!.matchAll(/\/([^\s/<>]+)\s+(\d+)\s+0\s+R/g)) {
        const cmap = fontObjs.get(Number(ref));
        if (cmap) cmaps.set(name!, cmap);
      }
    }
  }

  // Page content streams: `/F3 12 Tf`, then `<…> Tj` / `[<…> -250 <…>] TJ`.
  const runs: TextRun[] = [];
  const add = (font: string, more: string) => {
    const last = runs[runs.length - 1];
    if (last && last.font === font) last.text += more;
    else runs.push({ font, text: more });
  };
  const contents = new Set<number>();
  for (const o of objs.values()) {
    if (!/\/Type\s*\/Page\b/.test(o.dict)) continue;
    const c = /\/Contents\s*(\[[^\]]*\]|\d+\s+0\s+R)/.exec(o.dict)?.[1] ?? '';
    for (const [, ref] of c.matchAll(/(\d+)\s+0\s+R/g)) contents.add(Number(ref));
  }
  for (const n of contents) {
    const ops = latin1(objs.get(n)?.stream ?? null);
    let res: FontRes | undefined;
    const tokens =
      /\/([^\s/<>[\]()]+)\s+[-\d.]+\s+Tf|<([0-9a-fA-F\s]*)>|\(((?:\\.|[^\\)])*)\)|(-?\d+(?:\.\d+)?)|\b(ET|Td|TD|T\*|Tm)\b/g;
    let t: RegExpExecArray | null;
    while ((t = tokens.exec(ops))) {
      const font = res?.base ?? '';
      if (t[1] !== undefined) res = cmaps.get(t[1]);
      else if (t[2] !== undefined && res) {
        const { map, width } = res.cmap;
        const hex = t[2].replace(/\s/g, '').toUpperCase();
        let more = '';
        for (let i = 0; i + width <= hex.length; i += width)
          more += map.get(hex.slice(i, i + width)) ?? '';
        add(font, more);
      } else if (t[3] !== undefined) add(font, t[3]);
      else if (t[4] !== undefined && Number(t[4]) < -200) add(font, ' ');
      else if (t[5] !== undefined) add(font, ' ');
    }
  }
  return { text: runs.map((r) => r.text).join(''), runs, fonts };
}
