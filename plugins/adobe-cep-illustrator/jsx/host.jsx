// Font Manager's ExtendScript side in Illustrator (ES3: no JSON object).
// The panel calls these through CSInterface's evalScript; each returns a
// string: JSON for the data, "yes"/"no" for the question.

function fmQuote(s) {
  s = String(s);
  var out = '"';
  for (var i = 0; i < s.length; i++) {
    var c = s.charAt(i);
    var code = s.charCodeAt(i);
    if (c === '"' || c === '\\') out += '\\' + c;
    else if (code < 32 || code === 0x2028 || code === 0x2029) {
      var hex = code.toString(16);
      while (hex.length < 4) hex = '0' + hex;
      out += '\\u' + hex;
    } else out += c;
  }
  return out + '"';
}

// The file path of a saved document, else its name ("Untitled-1"): unique
// among the open documents, and stable while one stays open unsaved.
function fmKey(doc) {
  try {
    if (String(doc.path) !== '') return String(doc.fullName.fsName);
  } catch (e) {
    // An unsaved document has no file.
  }
  return String(doc.name);
}

/** The open documents: [{"key","name"}]. */
function fmOpenDocuments() {
  var parts = [];
  for (var i = 0; i < app.documents.length; i++) {
    var doc = app.documents[i];
    parts.push('{"key":' + fmQuote(fmKey(doc)) + ',"name":' + fmQuote(doc.name) + '}');
  }
  return '[' + parts.join(',') + ']';
}

/** One font as `fonts_needed` takes it, once per name. */
function fmAddFont(seen, parts, name, family, style) {
  if (!name || seen['$' + name]) return;
  seen['$' + name] = true;
  parts.push(
    '{"postscriptName":' +
      fmQuote(name) +
      ',"family":' +
      fmQuote(family || '') +
      ',"style":' +
      fmQuote(style || '') +
      '}'
  );
}

function fmXmlDecode(s) {
  return s
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, '&');
}

function fmXmpField(item, field) {
  var m = new RegExp('<stFnt:' + field + '>([^<]*)</stFnt:' + field + '>').exec(item);
  return m ? fmXmlDecode(m[1]) : '';
}

/**
 * The fonts a saved document names in its XMP metadata (`xmpTPg:Fonts`):
 * the only place Illustrator still names a font it does not have. For such
 * a font a text run's `textFont` is an object whose every property throws
 * UKFG ("unknown font"), so the runs cannot name it (E30.0 spike,
 * Illustrator 29.7). [{name, family, style}]; [] for an unsaved document or
 * one without the block.
 */
function fmXmpFonts(doc) {
  var fonts = [];
  var xmp = '';
  try {
    xmp = String(doc.XMPString);
  } catch (e) {
    return fonts;
  }
  var block = /<xmpTPg:Fonts>([\s\S]*?)<\/xmpTPg:Fonts>/.exec(xmp);
  if (!block) return fonts;
  var item = /<rdf:li[^>]*>([\s\S]*?)<\/rdf:li>/g;
  var m;
  while ((m = item.exec(block[1])) !== null) {
    fonts.push({
      name: fmXmpField(m[1], 'fontName'),
      family: fmXmpField(m[1], 'fontFamily'),
      style: fmXmpField(m[1], 'fontFace')
    });
  }
  return fonts;
}

/**
 * The fonts of the document with that key, once each:
 * [{"postscriptName","family","style"}] from every text run's `textFont`,
 * then from the XMP metadata (the fonts Illustrator does not have), or
 * "null" when no open document has the key.
 */
function fmDocumentFonts(key) {
  var doc = null;
  for (var i = 0; i < app.documents.length; i++) {
    if (fmKey(app.documents[i]) === key) doc = app.documents[i];
  }
  if (doc === null) return 'null';
  var seen = {};
  var parts = [];
  for (var f = 0; f < doc.textFrames.length; f++) {
    var ranges = doc.textFrames[f].textRanges;
    for (var r = 0; r < ranges.length; r++) {
      var font = null;
      var name = '';
      try {
        font = ranges[r].characterAttributes.textFont;
        name = String(font.name);
      } catch (e) {
        // A run Illustrator cannot describe, or a font it does not have
        // (UKFG): the XMP list below names those.
        continue;
      }
      var family = '';
      var style = '';
      try {
        family = font.family;
      } catch (e) {
        family = '';
      }
      try {
        style = font.style;
      } catch (e) {
        style = '';
      }
      fmAddFont(seen, parts, name, family, style);
    }
  }
  var xmp = fmXmpFonts(doc);
  for (var x = 0; x < xmp.length; x++) {
    fmAddFont(seen, parts, xmp[x].name, xmp[x].family, xmp[x].style);
  }
  return '[' + parts.join(',') + ']';
}

/** A message that does not ask anything. */
function fmTell(message) {
  alert(message, 'Font Manager');
  return '';
}

/** The deactivate offer: "yes" for Deactivate, "no" for Keep them. */
function fmAsk(message) {
  var w = new Window('dialog', 'Font Manager');
  w.add('statictext', undefined, message);
  var buttons = w.add('group');
  buttons.add('button', undefined, 'Keep them', { name: 'cancel' });
  buttons.add('button', undefined, 'Deactivate', { name: 'ok' });
  return w.show() === 1 ? 'yes' : 'no';
}
