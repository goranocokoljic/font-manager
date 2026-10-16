// sfnt.mjs — just enough font-file sniffing to pick and check the fixture corpus.
// The real parser is fontations in fm-core; this only reads magic numbers and the
// table directory, and returns null instead of throwing on a malformed file.

/** The container format from the first bytes, or null when it is none we know. */
export function sniffFormat(buf) {
  if (buf.length < 4) return null;
  const tag = buf.toString('latin1', 0, 4);
  if (tag === 'wOF2') return 'woff2';
  if (tag === 'wOFF') return 'woff';
  if (tag === 'ttcf') return 'ttc';
  if (tag === 'OTTO') return 'otf';
  if (buf.readUInt32BE(0) === 0x00010000 || tag === 'true') return 'ttf';
  // PFB: segment marker 0x80 0x01, then the cleartext "%!PS-AdobeFont" / "%!FontType1".
  if (buf[0] === 0x80 && buf[1] === 0x01 && /%!(PS-AdobeFont|FontType1)/.test(buf.toString('latin1', 6, 40))) {
    return 'type1';
  }
  if (/^%!(PS-AdobeFont|FontType1)/.test(buf.toString('latin1', 0, 20))) return 'type1';
  return null;
}

/**
 * Table tags of a bare sfnt (TTF/OTF), or null when the table directory does not fit
 * in the file or a table points past its end — the "broken" signal.
 */
export function sfntTables(buf, base = 0) {
  if (buf.length < base + 12) return null;
  const numTables = buf.readUInt16BE(base + 4);
  if (numTables === 0 || buf.length < base + 12 + numTables * 16) return null;
  const tags = [];
  for (let i = 0; i < numTables; i++) {
    const rec = base + 12 + i * 16;
    const offset = buf.readUInt32BE(rec + 8);
    const length = buf.readUInt32BE(rec + 12);
    if (offset + length > buf.length) return null;
    tags.push(buf.toString('latin1', rec, rec + 4));
  }
  return tags;
}

/** The faces of a TTC as table-tag lists, or null when it is malformed. */
export function ttcFaces(buf) {
  if (sniffFormat(buf) !== 'ttc' || buf.length < 12) return null;
  const n = buf.readUInt32BE(8);
  if (n === 0 || buf.length < 12 + 4 * n) return null;
  const faces = [];
  for (let i = 0; i < n; i++) {
    const tags = sfntTables(buf, buf.readUInt32BE(12 + 4 * i));
    if (!tags) return null;
    faces.push(tags);
  }
  return faces;
}

const COLOR_TABLES = ['COLR', 'CBDT', 'sbix', 'SVG '];

/**
 * What a file is, for the fixture manifest: woff2, woff, ttc, type1, color, variable,
 * otf, ttf — or broken when it claims to be a font and its table directory is unusable.
 * WOFF/WOFF2 are compressed and reported by container only.
 */
export function classify(buf) {
  const format = sniffFormat(buf);
  if (format === 'woff2' || format === 'woff' || format === 'type1') return format;
  if (format === 'ttc') return ttcFaces(buf) ? 'ttc' : 'broken';
  if (format === 'ttf' || format === 'otf') {
    const tags = sfntTables(buf);
    if (!tags) return 'broken';
    if (COLOR_TABLES.some((t) => tags.includes(t))) return 'color';
    if (tags.includes('fvar')) return 'variable';
    return format;
  }
  return 'broken';
}

/**
 * Pack bare sfnt fonts into one TrueType Collection. Each font is copied whole at a
 * 4-byte aligned offset and its table offsets are moved by that offset; no table
 * data changes.
 */
export function packTtc(fonts) {
  const align = (n) => (n + 3) & ~3;
  const header = 12 + 4 * fonts.length;
  const bases = [];
  let at = align(header);
  for (const f of fonts) {
    if (!sfntTables(f)) throw new Error('packTtc: not a usable sfnt');
    bases.push(at);
    at = align(at + f.length);
  }
  const out = Buffer.alloc(at);
  out.write('ttcf', 0, 'latin1');
  out.writeUInt32BE(0x00010000, 4);
  out.writeUInt32BE(fonts.length, 8);
  fonts.forEach((f, i) => {
    out.writeUInt32BE(bases[i], 12 + 4 * i);
    f.copy(out, bases[i]);
    const numTables = f.readUInt16BE(4);
    for (let t = 0; t < numTables; t++) {
      const rec = bases[i] + 12 + t * 16 + 8;
      out.writeUInt32BE(out.readUInt32BE(rec) + bases[i], rec);
    }
  });
  return out;
}
