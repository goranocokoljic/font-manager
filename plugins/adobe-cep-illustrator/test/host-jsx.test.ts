import { ES5_PLUS, codeOnly } from '@fm/plugin-shared/testing';
import { describe, expect, it } from 'vitest';
import hostJsx from '../jsx/host.jsx?raw';

// Illustrator runs `jsx/host.jsx` in ExtendScript, which is ES3 (`codeOnly`
// and `ES5_PLUS` in `@fm/plugin-shared/testing` say what that rules out).

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
