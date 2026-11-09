import { describe, expect, it } from 'vitest';
import { retireToast } from './retireToast';

describe('retireToast', () => {
  it('takes a leaving toast out of reach: test id, ids, screen readers, focus', () => {
    const el = document.createElement('div');
    el.dataset.testid = 'toast';
    el.innerHTML = '<p id="msg">Done</p><button>Undo</button>';
    document.body.append(el);
    retireToast(el);
    expect(el.hasAttribute('data-testid')).toBe(false);
    expect(el.getAttribute('aria-hidden')).toBe('true');
    expect(el.inert).toBe(true);
    expect(el.querySelector('[id]')).toBeNull();
    expect(document.getElementById('msg')).toBeNull();
    el.remove();
  });
});
