// Calls into an ExtendScript engine (Illustrator's, through CEP's
// `evalScript`): the script runner and the string-literal quoting.

/** Runs a script in the host's ExtendScript engine. */
export interface ExtendScript {
  evalScript(script: string): Promise<string>;
}

/**
 * `value` as an ExtendScript string literal. ExtendScript is ES3, where
 * U+2028 and U+2029 end a line, so JSON leaves them unescaped and they are
 * escaped here.
 */
export function scriptString(value: string): string {
  return JSON.stringify(value)
    .replace(/\u2028/g, '\\u2028')
    .replace(/\u2029/g, '\\u2029');
}

/** A call of an ExtendScript function with string arguments. */
export function scriptCall(name: string, ...args: string[]): string {
  return `${name}(${args.map(scriptString).join(',')})`;
}
