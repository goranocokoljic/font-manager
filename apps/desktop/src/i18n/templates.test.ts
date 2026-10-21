// Every user-visible string in a .vue template comes from the catalog (E19.2): this
// fails on literal text, a literal text attribute (`aria-label`, `title`, …) or a
// literal sentence inside a template expression.
import { describe, expect, it } from 'vitest';
import { parse } from 'vue/compiler-sfc';

/** Every .vue file under src, by its path from src (`components/X.vue`). */
const VUE_FILES = Object.fromEntries(
  Object.entries(
    import.meta.glob<string>('../**/*.vue', { query: '?raw', import: 'default', eager: true }),
  ).map(([path, source]) => [path.replace(/^\.\.\//, ''), source]),
);

/** Attributes whose value the user reads or hears. */
const TEXT_ATTRS = new Set([
  'aria-label',
  'aria-description',
  'aria-placeholder',
  'aria-roledescription',
  'aria-valuetext',
  'title',
  'placeholder',
  'alt',
  'label',
]);

const LETTER = /\p{L}/u;
/** A string literal in an expression that reads like words, not like an id or a class. */
const SENTENCE = /\p{Lu}\p{Ll}|\p{L}\s+\p{L}/u;
const QUOTED = /'((?:[^'\\]|\\.)*)'|"((?:[^"\\]|\\.)*)"|`((?:[^`\\]|\\.)*)`/g;

interface Node {
  type: number;
  tag?: string;
  content?: string | { content?: string };
  name?: string;
  value?: { content: string };
  arg?: { content?: string };
  exp?: { content?: string };
  props?: Node[];
  children?: Node[];
  loc: { start: { line: number } };
}

// The node types of @vue/compiler-core.
const ELEMENT = 1;
const TEXT = 2;
const INTERPOLATION = 5;
const ATTRIBUTE = 6;
const DIRECTIVE = 7;

/** The sentence-like string literals of an expression; `t('key')` keys are not text. */
function literalSentences(expression: string): string[] {
  const withoutKeys = expression.replace(/\bt\(\s*(['"`])[^'"`]*\1/g, 't(');
  const found: string[] = [];
  for (const m of withoutKeys.matchAll(QUOTED)) {
    const text = m[1] ?? m[2] ?? m[3] ?? '';
    // A template literal's `${…}` parts are code.
    const plain = text.replace(/\$\{[^}]*\}/g, ' ');
    if (SENTENCE.test(plain)) found.push(text);
  }
  return found;
}

/** Every literal the user would see in `source`'s template, as `line: text`. */
export function literalTexts(source: string, file = 'inline.vue'): string[] {
  const { descriptor } = parse(source, { filename: file });
  const ast = descriptor.template?.ast as Node | undefined;
  if (!ast) return [];
  const out: string[] = [];
  const walk = (node: Node) => {
    const line = node.loc.start.line;
    if (node.type === TEXT && typeof node.content === 'string') {
      const text = node.content.trim();
      if (LETTER.test(text)) out.push(`${line}: ${text}`);
    }
    if (node.type === INTERPOLATION && typeof node.content === 'object') {
      for (const s of literalSentences(node.content.content ?? '')) out.push(`${line}: {{ ${s} }}`);
    }
    if (node.type === ELEMENT) {
      for (const prop of node.props ?? []) {
        const pl = prop.loc.start.line;
        if (prop.type === ATTRIBUTE && prop.name && TEXT_ATTRS.has(prop.name)) {
          const value = prop.value?.content ?? '';
          if (LETTER.test(value)) out.push(`${pl}: ${prop.name}="${value}"`);
        }
        if (prop.type === DIRECTIVE && prop.exp?.content) {
          // Bound text attributes and event handlers that show text (a toast) alike.
          for (const s of literalSentences(prop.exp.content)) {
            out.push(`${pl}: ${prop.name}${prop.arg?.content ? `:${prop.arg.content}` : ''}="${s}"`);
          }
        }
      }
    }
    for (const child of node.children ?? []) walk(child);
  };
  walk(ast);
  return out;
}

describe('template literals', () => {
  it('finds literal text, literal text attributes and sentences in expressions', () => {
    const found = literalTexts(`<template>
  <div>
    <button aria-label="Close" title="Close it">×</button>
    <p>Hello</p>
    <span :title="busy ? 'Working…' : t('a.b')">{{ t('x.y') }} · {{ n }}</span>
    <i class="kind-dark" :class="mode === 'session' ? 'on' : 'off'" />
  </div>
</template>`);
    expect(found).toEqual([
      '3: aria-label="Close"',
      '3: title="Close it"',
      '4: Hello',
      '5: bind:title="Working…"',
    ]);
  });

  it('no .vue template in src shows literal text', () => {
    expect(Object.keys(VUE_FILES).length).toBeGreaterThan(40);
    const offenders = Object.entries(VUE_FILES).flatMap(([file, source]) =>
      literalTexts(source, file).map((hit) => `${file}:${hit}`),
    );
    expect(offenders).toEqual([]);
  });
});
