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

/**
 * The fonts of every text frame of the document with that key, once each:
 * [{"postscriptName","family","style"}] from `textFont`, or "null" when no
 * open document has the key.
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
      try {
        font = ranges[r].characterAttributes.textFont;
      } catch (e) {
        // A run Illustrator cannot describe has no font for us.
      }
      if (!font || !font.name || seen['$' + font.name]) continue;
      seen['$' + font.name] = true;
      parts.push(
        '{"postscriptName":' +
          fmQuote(font.name) +
          ',"family":' +
          fmQuote(font.family || '') +
          ',"style":' +
          fmQuote(font.style || '') +
          '}'
      );
    }
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
