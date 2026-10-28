// A check for ExtendScript sources (ES3): syntax V8 accepts, and so the unit
// tests that run a `.jsx` file in V8, does not load in Illustrator, and then
// no function of the file exists. These are the ES5+ forms a formatter or a
// hand edit can bring in.

/** The code with comments and string literals blanked out. */
export function codeOnly(source: string): string {
  let out = '';
  let i = 0;
  while (i < source.length) {
    const c = source[i];
    const next = source[i + 1];
    if (c === '/' && next === '/') {
      while (i < source.length && source[i] !== '\n') i++;
    } else if (c === '/' && next === '*') {
      i = source.indexOf('*/', i + 2) + 2;
    } else if (c === '"' || c === "'") {
      i++;
      while (i < source.length && source[i] !== c) i += source[i] === '\\' ? 2 : 1;
      i++;
      out += '""';
    } else {
      out += c;
      i++;
    }
  }
  return out;
}

export const ES5_PLUS: [string, RegExp][] = [
  ['a trailing comma in a call, array or object', /,\s*[)\]}]/],
  ['let or const', /\b(let|const)\s/],
  ['an arrow function', /=>/],
  ['a template literal', /`/],
  ['a class', /\bclass\s/],
  ['the JSON object', /\bJSON\./],
  ['spread or rest', /\.\.\./],
  ['ES5 array methods', /\.(forEach|map|filter|reduce|indexOf|some|every)\(/],
  ['String.prototype.trim', /\.trim\(/],
];
