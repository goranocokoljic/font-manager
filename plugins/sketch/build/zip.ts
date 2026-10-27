// A zip writer for the plugin bundle, so the build needs no archiver package.
// The same files always give the same bytes: entries are sorted, every time
// stamp is the zip epoch (1980-01-01 00:00) and the permissions are fixed.

import { crc32, deflateRawSync } from 'node:zlib';

const LOCAL = 0x04034b50;
const CENTRAL = 0x02014b50;
const END = 0x06054b50;
// Made by Unix (3), zip 2.0, so macOS keeps the modes below on unzip.
const MADE_BY = (3 << 8) | 20;
const NEEDED = 20;
const UTF8 = 0x0800;
const DOS_DATE = (0 << 9) | (1 << 5) | 1;
const FILE_MODE = 0o100644;
const DIR_MODE = 0o040755;
const DIR_ATTR = 0x10;

interface Entry {
  name: string;
  data: Uint8Array;
  packed: Uint8Array;
  crc: number;
  method: number;
  external: number;
}

/**
 * A zip archive of `files` (path with `/` → content). Every parent folder
 * gets its own entry, as the Finder writes them.
 */
export function zip(files: Record<string, Uint8Array>): Uint8Array {
  const names = new Set<string>();
  for (const path of Object.keys(files)) {
    if (path.startsWith('/') || path.endsWith('/') || path.split('/').includes('..')) {
      throw new Error(`not a relative file path: ${path}`);
    }
    names.add(path);
    const parts = path.split('/');
    for (let i = 1; i < parts.length; i++) names.add(`${parts.slice(0, i).join('/')}/`);
  }
  const entries: Entry[] = [...names].sort().map((name) => {
    if (name.endsWith('/')) {
      const empty = new Uint8Array(0);
      return {
        name,
        data: empty,
        packed: empty,
        crc: 0,
        method: 0,
        external: ((DIR_MODE << 16) | DIR_ATTR) >>> 0,
      };
    }
    const data = files[name];
    return {
      name,
      data,
      packed: deflateRawSync(data, { level: 9 }),
      crc: crc32(data),
      method: 8,
      external: (FILE_MODE << 16) >>> 0,
    };
  });

  const parts: Uint8Array[] = [];
  const offsets: number[] = [];
  let offset = 0;
  const push = (chunk: Uint8Array) => {
    parts.push(chunk);
    offset += chunk.length;
  };
  const encoder = new TextEncoder();

  for (const e of entries) {
    offsets.push(offset);
    const name = encoder.encode(e.name);
    const h = new DataView(new ArrayBuffer(30));
    h.setUint32(0, LOCAL, true);
    h.setUint16(4, NEEDED, true);
    h.setUint16(6, UTF8, true);
    h.setUint16(8, e.method, true);
    h.setUint16(10, 0, true);
    h.setUint16(12, DOS_DATE, true);
    h.setUint32(14, e.crc, true);
    h.setUint32(18, e.packed.length, true);
    h.setUint32(22, e.data.length, true);
    h.setUint16(26, name.length, true);
    h.setUint16(28, 0, true);
    push(new Uint8Array(h.buffer));
    push(name);
    push(e.packed);
  }

  const start = offset;
  entries.forEach((e, i) => {
    const name = encoder.encode(e.name);
    const h = new DataView(new ArrayBuffer(46));
    h.setUint32(0, CENTRAL, true);
    h.setUint16(4, MADE_BY, true);
    h.setUint16(6, NEEDED, true);
    h.setUint16(8, UTF8, true);
    h.setUint16(10, e.method, true);
    h.setUint16(12, 0, true);
    h.setUint16(14, DOS_DATE, true);
    h.setUint32(16, e.crc, true);
    h.setUint32(20, e.packed.length, true);
    h.setUint32(24, e.data.length, true);
    h.setUint16(28, name.length, true);
    // Extra field, comment, disk and internal attributes stay 0.
    h.setUint32(38, e.external, true);
    h.setUint32(42, offsets[i], true);
    push(new Uint8Array(h.buffer));
    push(name);
  });

  const end = new DataView(new ArrayBuffer(22));
  end.setUint32(0, END, true);
  end.setUint16(8, entries.length, true);
  end.setUint16(10, entries.length, true);
  end.setUint32(12, offset - start, true);
  end.setUint32(16, start, true);
  push(new Uint8Array(end.buffer));

  const out = new Uint8Array(offset);
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}
