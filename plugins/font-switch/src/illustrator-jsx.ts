// Font Switch's ExtendScript side in Illustrator. The adapter loads it into
// the engine once through `evalScript`; it is ES3 (no JSON object), so the
// fonts come back as lines of tab-separated names.

export const ILLUSTRATOR_JSX = String.raw`
// The text the font goes to: the selected text, or the text of each
// selected text frame. An insertion point and other objects are no text.
function fmFontSwitchTargets() {
  var targets = [];
  if (app.documents.length === 0) return targets;
  var selection = app.activeDocument.selection;
  if (!selection) return targets;
  if (selection.typename === 'TextRange') {
    if (selection.length > 0) targets.push(selection);
    return targets;
  }
  for (var i = 0; i < selection.length; i++) {
    if (selection[i].typename === 'TextFrame') targets.push(selection[i].textRange);
  }
  return targets;
}

/** "yes" when the selection has text in it. */
function fmFontSwitchHasText() {
  return fmFontSwitchTargets().length > 0 ? 'yes' : 'no';
}

/** The fonts Illustrator has: one "PostScript name<TAB>family<TAB>style" line each. */
function fmFontSwitchFonts() {
  var lines = [];
  for (var i = 0; i < app.textFonts.length; i++) {
    var font = app.textFonts[i];
    lines.push(font.name + '\t' + font.family + '\t' + font.style);
  }
  return lines.join('\n');
}

/** Applies the font with that PostScript name; "no" when no text is selected. */
function fmFontSwitchApply(name) {
  var targets = fmFontSwitchTargets();
  if (targets.length === 0) return 'no';
  var font = app.textFonts.getByName(name);
  for (var i = 0; i < targets.length; i++) targets[i].characterAttributes.textFont = font;
  return 'yes';
}
`;
