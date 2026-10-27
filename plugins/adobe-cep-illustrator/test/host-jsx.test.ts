import { describe, expect, it } from 'vitest';
import hostJsx from '../jsx/host.jsx?raw';

// Illustrator runs `jsx/host.jsx` in ExtendScript, which is ES3: syntax V8
// accepts (and so the other tests, which run the file in V8) does not load
// there, and then no `fm*` function exists. These are the ES5+ forms a
// formatter or a hand edit can bring in.

/** The code with comments and string literals blanked out. */
function codeOnly(source: string): string {
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

const ES5_PLUS: [string, RegExp][] = [
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

describe('jsx/host.jsx', () => {
  const code = codeOnly(hostJsx);

  it.each(ES5_PLUS)('has no %s (ES3)', (_, pattern) => {
    expect(code).not.toMatch(pattern);
  });

  it('is checked on its code, not its comments and strings', () => {
    expect(codeOnly('a(1, // x,)\n "y,)" );')).toBe('a(1, \n "" );');
    expect(codeOnly("f('a', 'b',\n)")).toMatch(ES5_PLUS[0][1]);
  });
});
