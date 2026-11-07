import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { h } from 'vue';
import SpecimenText from './SpecimenText';

const inP = (text: string, missing: string[] = []) =>
  mount({ render: () => h('p', [h(SpecimenText, { text, missing })]) });

describe('SpecimenText', () => {
  it('without tofu is one text node, the first child of its line', () => {
    const p = inP('Ljubazni fenjerdžija').element as HTMLElement;
    // A layout check reads the line's first text node (e2e/text-layouts.spec.ts).
    expect(p.firstChild?.nodeType).toBe(Node.TEXT_NODE);
    expect(p.firstChild?.textContent).toBe('Ljubazni fenjerdžija');
    expect(p.childNodes).toHaveLength(1);
  });

  it('with tofu draws the text and a box per missing character', () => {
    const p = inP('ažb', ['ž']).element as HTMLElement;
    const boxes = p.querySelectorAll('[role="img"]');
    expect([...boxes].map((b) => b.getAttribute('aria-label'))).toEqual(['missing glyph ž']);
    expect(p.textContent).toBe('ab');
  });
});
