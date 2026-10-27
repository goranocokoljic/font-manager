#!/usr/bin/env node
// Builds the small design documents under fixtures/documents/ that the
// fm-core `document_fonts` tests read (E32.1). Each file is made from the
// published structure of its format, with nothing but Node's own zlib, so
// no third-party application or file is needed. See the README there for
// what each file holds.
//
//   node scripts/build-document-fixtures.mjs
//
// The output is deterministic (fixed zip times, fixed content); the
// committed files are the test input, so re-run this only to change them.

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { crc32, deflateRawSync, deflateSync } from 'node:zlib';

const out = join(dirname(fileURLToPath(import.meta.url)), '..', 'fixtures', 'documents');
mkdirSync(out, { recursive: true });
const write = (name, bytes) => writeFileSync(join(out, name), bytes);

const u16le = (n) => { const b = Buffer.alloc(2); b.writeUInt16LE(n); return b; };
const u32le = (n) => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return b; };
const u16be = (n) => { const b = Buffer.alloc(2); b.writeUInt16BE(n); return b; };
const u32be = (n) => { const b = Buffer.alloc(4); b.writeUInt32BE(n); return b; };

// --- zip (APPNOTE 4.3: local headers, central directory, EOCD) ------------

/** entries: [name, string|Buffer, 'store'|'deflate'] */
function zip(entries) {
  const locals = [];
  const central = [];
  let offset = 0;
  for (const [name, content, how = 'deflate'] of entries) {
    const data = Buffer.from(content);
    const packed = how === 'store' ? data : deflateRawSync(data);
    const method = how === 'store' ? 0 : 8;
    const nameBytes = Buffer.from(name, 'utf8');
    const crc = crc32(data);
    // 2026-01-01 00:00, DOS format.
    const time = 0;
    const date = ((2026 - 1980) << 9) | (1 << 5) | 1;
    const common = Buffer.concat([
      u16le(20), u16le(0x0800), u16le(method), u16le(time), u16le(date),
      u32le(crc), u32le(packed.length), u32le(data.length), u16le(nameBytes.length), u16le(0),
    ]);
    const local = Buffer.concat([u32le(0x04034b50), common, nameBytes, packed]);
    central.push(Buffer.concat([
      u32le(0x02014b50), u16le(20), common, u16le(0), u16le(0), u16le(0), u32le(0),
      u32le(offset), nameBytes,
    ]));
    locals.push(local);
    offset += local.length;
  }
  const dir = Buffer.concat(central);
  const eocd = Buffer.concat([
    u32le(0x06054b50), u16le(0), u16le(0), u16le(entries.length), u16le(entries.length),
    u32le(dir.length), u32le(offset), u16le(0),
  ]);
  return Buffer.concat([...locals, dir, eocd]);
}

const XML = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n';
const W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"';
const A = 'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"';
const P = 'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"';
const R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const rels = (list) =>
  `${XML}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
  list.map(([id, type, target]) => `<Relationship Id="${id}" Type="${R}/${type}" Target="${target}"/>`).join('') +
  '</Relationships>';

// --- .docx (ECMA-376 WordprocessingML) ------------------------------------

write('sample.docx', zip([
  ['[Content_Types].xml', `${XML}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>`, 'store'],
  ['_rels/.rels', rels([['rId1', 'officeDocument', 'word/document.xml']])],
  ['word/document.xml', `${XML}<w:document ${W}><w:body>
<w:p><w:r><w:rPr><w:rFonts w:ascii="Inter" w:hAnsi="Inter" w:eastAsia="Noto Sans JP" w:cs="Arial" w:asciiTheme="minorHAnsi"/></w:rPr><w:t>Hello &amp; welcome</w:t></w:r></w:p>
<w:p><w:r><w:rPr><w:rFonts w:ascii="Inter"/></w:rPr><w:t>Again</w:t></w:r></w:p>
</w:body></w:document>`],
  ['word/styles.xml', `${XML}<w:styles ${W}><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Georgia" w:hAnsi="Georgia"/></w:rPr></w:rPrDefault></w:docDefaults></w:styles>`, 'store'],
  ['word/fontTable.xml', `${XML}<w:fonts ${W}><w:font w:name="Calibri"><w:panose1 w:val="020F0502020204030204"/></w:font><w:font w:name="Cambria"/><w:font w:name="Arial"/></w:fonts>`],
  ['word/theme/theme1.xml', `${XML}<a:theme ${A} name="Office"><a:themeElements><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>`],
]));

// An encrypted Office document is an OLE compound file, not a zip.
write('encrypted.docx', Buffer.concat([
  Buffer.from([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]), Buffer.alloc(504),
]));

// --- .pptx (ECMA-376 PresentationML + DrawingML themes) -------------------

const theme = (name, major, minor) =>
  `${XML}<a:theme ${A} name="${name}"><a:themeElements><a:fontScheme name="${name}">` +
  `<a:majorFont><a:latin typeface="${major[0]}"/><a:ea typeface="${major[1]}"/><a:cs typeface="${major[2]}"/></a:majorFont>` +
  `<a:minorFont><a:latin typeface="${minor[0]}"/><a:ea typeface="${minor[1]}"/><a:cs typeface="${minor[2]}"/></a:minorFont>` +
  '</a:fontScheme></a:themeElements></a:theme>';
const run = (fonts, text) => `<a:r><a:rPr lang="en-US">${fonts}</a:rPr><a:t>${text}</a:t></a:r>`;

write('sample.pptx', zip([
  ['[Content_Types].xml', `${XML}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>`, 'store'],
  ['ppt/presentation.xml', `${XML}<p:presentation ${P}/>`],
  ['ppt/slides/slide1.xml', `${XML}<p:sld ${P} ${A}><p:cSld><p:spTree><p:sp><p:txBody><a:p>` +
    run('<a:latin typeface="Roboto"/><a:ea typeface="+mn-ea"/><a:cs typeface=""/>', 'One') +
    run('<a:latin typeface="+mj-lt"/>', 'Title') +
    '</a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>'],
  ['ppt/slides/_rels/slide1.xml.rels', rels([['rId1', 'slideLayout', '../slideLayouts/slideLayout1.xml']])],
  ['ppt/slideLayouts/slideLayout1.xml', `${XML}<p:sldLayout ${P} ${A}/>`],
  ['ppt/slideLayouts/_rels/slideLayout1.xml.rels', rels([['rId1', 'slideMaster', '../slideMasters/slideMaster1.xml']])],
  ['ppt/slideMasters/slideMaster1.xml', `${XML}<p:sldMaster ${P} ${A}><p:txStyles><p:bodyStyle><a:lvl1pPr><a:defRPr><a:latin typeface="+mn-lt"/><a:ea typeface="+mj-ea"/></a:defRPr></a:lvl1pPr></p:bodyStyle></p:txStyles></p:sldMaster>`],
  ['ppt/slideMasters/_rels/slideMaster1.xml.rels', rels([
    ['rId1', 'slideLayout', '../slideLayouts/slideLayout1.xml'],
    ['rId2', 'theme', '../theme/theme1.xml'],
  ])],
  ['ppt/theme/theme1.xml', theme('Brand', ['Montserrat', '', ''], ['Open Sans', 'Noto Sans KR', ''])],
  // A theme no master uses: its fonts must not appear.
  ['ppt/theme/theme2.xml', theme('Unused', ['Comic Sans MS', '', ''], ['Papyrus', '', ''])],
]));

// --- .sketch (Sketch file format: zip of JSON, pages/<id>.json) -----------

const fontAttr = (name) => ({
  MSAttributedStringFontAttribute: { _class: 'fontDescriptor', attributes: { name, size: 24 } },
});
const textLayer = (names) => ({
  _class: 'text',
  name: 'Text',
  attributedString: {
    _class: 'attributedString',
    string: 'Hi',
    attributes: names.map((n, i) => ({
      _class: 'stringAttribute', location: i, length: 1, attributes: fontAttr(n),
    })),
  },
  style: { _class: 'style', textStyle: { _class: 'textStyle', encodedAttributes: fontAttr(names[0]) } },
});
write('sample.sketch', zip([
  ['document.json', JSON.stringify({ _class: 'document', pages: [] })],
  ['meta.json', JSON.stringify({ app: 'com.bohemiancoding.sketch3', version: 146 })],
  ['user.json', '{}'],
  ['pages/0A1B2C3D.json', JSON.stringify({
    _class: 'page',
    layers: [
      textLayer(['Inter-Bold', 'Inter-Regular']),
      { _class: 'group', layers: [{ _class: 'group', layers: [textLayer(['SFProText-Semibold'])] }] },
    ],
  })],
  ['pages/4E5F6A7B.json', JSON.stringify({ _class: 'page', layers: [textLayer(['Inter-Regular'])] })],
]));

// --- .fig (canvas.fig: "fig-kiwi", version, schema chunk, data chunk) -----

// kiwi (github.com/evanw/kiwi) binary encoding.
const varUint = (n) => {
  const bytes = [];
  do {
    let b = n & 0x7f;
    n = Math.floor(n / 128);
    if (n) b |= 0x80;
    bytes.push(b);
  } while (n);
  return Buffer.from(bytes);
};
const varInt = (n) => varUint(n < 0 ? (~n << 1) | 1 : n << 1);
const kstr = (s) => Buffer.concat([Buffer.from(s, 'utf8'), Buffer.from([0])]);
const kfloat = (f) => {
  const b = Buffer.alloc(4);
  b.writeFloatLE(f);
  let bits = b.readUInt32LE();
  bits = ((bits >>> 23) | (bits << 9)) >>> 0;
  if (bits === 0) return Buffer.from([0]);
  const o = Buffer.alloc(4);
  o.writeUInt32LE(bits);
  return o;
};
const [BOOL, UINT, FLOAT, STRING] = [-1, -4, -5, -6];
const ENUM = 0, STRUCT = 1, MESSAGE = 2;
// Definitions: index = type id.
const defs = [
  ['NodeType', ENUM, [['DOCUMENT', 0, false, 0], ['TEXT', 0, false, 1]]],
  ['FontName', STRUCT, [['family', STRING, false, 1], ['style', STRING, false, 2], ['postscript', STRING, false, 3]]],
  ['Vector', STRUCT, [['x', FLOAT, false, 1], ['y', FLOAT, false, 2]]],
  ['NodeChange', MESSAGE, [
    ['type', 0, false, 1], ['name', STRING, false, 2], ['fontName', 1, false, 3],
    ['size', 2, false, 4], ['visible', BOOL, false, 5], ['tags', STRING, true, 6], ['opacity', FLOAT, false, 7],
  ]],
  ['Message', MESSAGE, [['type', UINT, false, 1], ['nodeChanges', 3, true, 2]]],
];
const schema = Buffer.concat([
  varUint(defs.length),
  ...defs.flatMap(([name, kind, fields]) => [
    kstr(name), Buffer.from([kind]), varUint(fields.length),
    ...fields.flatMap(([f, type, array, value]) => [kstr(f), varInt(type), Buffer.from([array ? 1 : 0]), varUint(value)]),
  ]),
]);
const fontName = (family, style, ps) => Buffer.concat([kstr(family), kstr(style), kstr(ps)]);
const node = (fields) => Buffer.concat([...fields.map(([id, v]) => Buffer.concat([varUint(id), v])), varUint(0)]);
const nodes = [
  node([[1, varUint(0)], [2, kstr('Document')]]),
  node([[1, varUint(1)], [2, kstr('Title')], [3, fontName('Inter', 'Bold', 'Inter-Bold')],
    [4, Buffer.concat([kfloat(10.5), kfloat(0)])], [5, Buffer.from([1])]]),
  node([[1, varUint(1)], [3, fontName('Inter', 'Regular', 'Inter-Regular')], [6, Buffer.concat([varUint(2), kstr('a'), kstr('b')])], [7, kfloat(1)]]),
  node([[1, varUint(1)], [3, fontName('Inter', 'Bold', 'Inter-Bold')]]),
  // No PostScript name: Figma leaves it empty for some fonts.
  node([[1, varUint(1)], [3, fontName('Roboto Mono', 'Regular', '')]]),
];
const message = node([[1, varUint(1)], [2, Buffer.concat([varUint(nodes.length), ...nodes])]]);
const canvas = (schemaChunk, dataChunk) => Buffer.concat([
  Buffer.from('fig-kiwi'), u32le(48),
  u32le(schemaChunk.length), schemaChunk, u32le(dataChunk.length), dataChunk,
]);
write('sample.fig', zip([
  ['canvas.fig', canvas(deflateRawSync(schema), deflateRawSync(message)), 'store'],
  ['meta.json', JSON.stringify({ file_name: 'Sample' })],
]));
// Newer Figma files compress the data chunk with zstd (frame magic 28 B5 2F FD).
write('zstd.fig', canvas(deflateRawSync(schema), Buffer.from([0x28, 0xb5, 0x2f, 0xfd, 0x00, 0x58, 0x00, 0x00])));

// --- XMP (ISO 16684-1; xmpTPg:Fonts with stFnt:Font structs) --------------

const BOM = String.fromCharCode(0xfeff);
const xmp = (fonts) => `<?xpacket begin="${BOM}" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmpTPg="http://ns.adobe.com/xap/1.0/t/pg/"
    xmlns:stFnt="http://ns.adobe.com/xap/1.0/sType/Font#">
   <xmpTPg:Fonts>
    <rdf:Bag>
${fonts}
    </rdf:Bag>
   </xmpTPg:Fonts>
   <xmpTPg:PlateNames><rdf:Seq><rdf:li>Cyan</rdf:li><rdf:li>Black</rdf:li></rdf:Seq></xmpTPg:PlateNames>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>`;
const liElements = (ps, family, face) => `     <rdf:li rdf:parseType="Resource">
      <stFnt:fontName>${ps}</stFnt:fontName>
      <stFnt:fontFamily>${family}</stFnt:fontFamily>
      <stFnt:fontFace>${face}</stFnt:fontFace>
      <stFnt:fontType>Open Type</stFnt:fontType>
     </rdf:li>`;
const liAttrs = (ps, family, face) =>
  `     <rdf:li stFnt:fontName="${ps}" stFnt:fontFamily="${family}" stFnt:fontFace="${face}" stFnt:fontType="TrueType"/>`;

// --- .indd (database header GUID, then pages; XMP stored in plain) --------

const inddGuid = Buffer.from([0x06, 0x06, 0xed, 0xf5, 0xd8, 0x1d, 0x46, 0xe5, 0xbd, 0x31, 0xef, 0xe7, 0xfe, 0x74, 0xb7, 0x1d]);
write('sample.indd', Buffer.concat([
  inddGuid, Buffer.from('DOCUMENT'), Buffer.alloc(4072, 0xa5),
  Buffer.from(xmp([
    liElements('MinionPro-Regular', 'Minion Pro', 'Regular'),
    liAttrs('MyriadPro-Bold', 'Myriad Pro', 'Bold'),
    liAttrs('Brand&amp;Co-Sans', 'Brand&amp;Co Sans', 'Book'),
  ].join('\n')), 'utf8'),
  Buffer.alloc(2048, 0x5a),
]));

// --- .ai (PDF 1.6 with XMP metadata, font dictionaries, an object stream) -

function pdf(objects, trailerExtra = '') {
  const parts = [Buffer.from('%PDF-1.6\n%\xe2\xe3\xcf\xd3\n', 'latin1')];
  for (const [i, body] of objects.entries()) {
    parts.push(Buffer.from(`${i + 1} 0 obj\n`), Buffer.from(body), Buffer.from('\nendobj\n'));
  }
  parts.push(Buffer.from(`trailer\n<< /Root 1 0 R /Size ${objects.length + 1}${trailerExtra} >>\nstartxref\n0\n%%EOF\n`));
  return Buffer.concat(parts);
}
const stream = (dict, data) => Buffer.concat([
  Buffer.from(`<< ${dict} /Length ${data.length} >>\nstream\n`), data, Buffer.from('\nendstream'),
]);
const metadata = Buffer.from(xmp(liAttrs('MinionPro-Regular', 'Minion Pro', 'Regular')), 'utf8');
const objStm = deflateSync(Buffer.from('7 0 << /Type /Font /Subtype /Type1 /BaseFont /Lato-Bold >>'));
const aiObjects = [
  '<< /Type /Catalog /Pages 2 0 R /Metadata 3 0 R >>',
  '<< /Type /Pages /Kids [] /Count 0 >>',
  stream('/Type /Metadata /Subtype /XML', metadata),
  '<< /Type /Font /Subtype /Type1 /BaseFont /ABCDEF+MinionPro-Regular >>',
  '<< /Type /Font /Subtype /TrueType /BaseFont /Source#20Sans#20Pro >>',
  stream('/Type /ObjStm /N 1 /First 4 /Filter /FlateDecode', objStm),
];
write('sample.ai', pdf(aiObjects));
write('encrypted.ai', pdf(aiObjects, ' /Encrypt << /Filter /Standard /V 2 >>'));

// --- .psd (Adobe Photoshop File Format: header, sections, layer records) --

// Engine data as Photoshop writes it: PostScript-like, strings UTF-16BE
// with a byte-order mark and `\` escapes for ( ) and \.
const psString = (s) => {
  const utf16 = Buffer.alloc(2 + s.length * 2);
  utf16[0] = 0xfe;
  utf16[1] = 0xff;
  for (let i = 0; i < s.length; i++) utf16.writeUInt16BE(s.charCodeAt(i), 2 + i * 2);
  const escaped = [];
  for (const b of utf16) {
    if (b === 0x28 || b === 0x29 || b === 0x5c) escaped.push(0x5c);
    escaped.push(b);
  }
  return Buffer.concat([Buffer.from('('), Buffer.from(escaped), Buffer.from(')')]);
};
const engineData = (fonts) => Buffer.concat([
  Buffer.from('\n\n<<\n\t/EngineDict\n\t<<\n\t\t/Editor\n\t\t<<\n\t\t\t/Text '), psString('Hi\r'),
  Buffer.from('\n\t\t>>\n\t>>\n\t/ResourceDict\n\t<<\n\t\t/StyleSheetSet\n\t\t[\n\t\t<<\n\t\t\t/Name '), psString('Normal RGB'),
  Buffer.from('\n\t\t>>\n\t\t]\n\t\t/FontSet\n\t\t[\n'),
  ...fonts.flatMap((f) => [Buffer.from('\t\t<<\n\t\t\t/Name '), psString(f), Buffer.from('\n\t\t\t/Script 0\n\t\t\t/FontType 1\n\t\t\t/Synthetic 0\n\t\t>>\n')]),
  Buffer.from('\t\t]\n\t\t/SuperscriptSize .583\n\t>>\n>>'),
]);
const padTo = (buf, n) => Buffer.concat([buf, Buffer.alloc((n - (buf.length % n)) % n)]);
const typeTool = (fonts) => {
  const engine = engineData(fonts);
  const descriptor = Buffer.concat([
    u32be(1), u16be(0), // name: one UTF-16 code unit, empty
    u32be(0), Buffer.from('TxLr'), // class id
    u32be(1), // one item
    u32be(10), Buffer.from('EngineData'), Buffer.from('tdta'), u32be(engine.length), engine,
  ]);
  return Buffer.concat([u16be(1), Buffer.alloc(48), u16be(50), u32be(16), descriptor]);
};
const block = (key, data) => {
  const body = padTo(data, 4);
  return Buffer.concat([Buffer.from('8BIM'), Buffer.from(key), u32be(body.length), body]);
};
const layerRecord = (name, blocks) => {
  const pascal = padTo(Buffer.concat([Buffer.from([name.length]), Buffer.from(name, 'latin1')]), 4);
  const extra = Buffer.concat([u32be(0), u32be(0), pascal, ...blocks]);
  return Buffer.concat([
    Buffer.alloc(16), u16be(0), Buffer.from('8BIM'), Buffer.from('norm'),
    Buffer.from([255, 0, 0, 0]), u32be(extra.length), extra,
  ]);
};
const layers = Buffer.concat([
  u16be(3),
  layerRecord('Title', [block('luni', Buffer.concat([u32be(5), Buffer.from('\0T\0i\0t\0l\0e', 'latin1')])),
    block('TySh', typeTool(['ArialMT', 'AdobeInvisFont', 'Inter-Bold']))]),
  layerRecord('Body', [block('TySh', typeTool(['Inter-Bold', 'Georgia', 'AdobeInvisFont']))]),
  layerRecord('Photo', []),
]);
const layerInfo = padTo(layers, 4);
const layerAndMask = Buffer.concat([u32be(layerInfo.length), layerInfo, u32be(0)]);
write('sample.psd', Buffer.concat([
  Buffer.from('8BPS'), u16be(1), Buffer.alloc(6), u16be(3), u32be(1), u32be(1), u16be(8), u16be(3),
  u32be(0), // colour mode data
  u32be(0), // image resources
  u32be(layerAndMask.length), layerAndMask,
  u16be(0), Buffer.from([0x20, 0x40, 0x60]), // raw image data, 1×1 RGB
]));

console.log(`wrote fixtures to ${out}`);
