import { afterEach, describe, expect, it } from 'vitest';
import { formatRelativeTime, setLocale } from '.';

const NOW = Date.UTC(2026, 10, 15, 12, 0, 0);
const ago = (seconds: number) => NOW - seconds * 1000;

afterEach(() => setLocale('en'));

describe('formatRelativeTime (E50.13: "scanned 2 hr. ago")', () => {
  it('says the largest whole unit, and now under a minute', () => {
    setLocale('en');
    expect(formatRelativeTime(ago(20), NOW)).toBe('now');
    expect(formatRelativeTime(ago(5 * 60), NOW)).toBe('5 min. ago');
    expect(formatRelativeTime(ago(2 * 3600 + 59), NOW)).toBe('2 hr. ago');
    expect(formatRelativeTime(ago(26 * 3600), NOW)).toBe('yesterday');
    expect(formatRelativeTime(ago(3 * 86_400), NOW)).toBe('3 days ago');
    expect(formatRelativeTime(ago(40 * 86_400), NOW)).toBe('last mo.');
    expect(formatRelativeTime(new Date(ago(400 * 86_400)), NOW)).toBe('last yr.');
  });

  it("follows the UI's locale", () => {
    setLocale('de');
    expect(formatRelativeTime(ago(26 * 3600), NOW)).toBe('gestern');
  });
});
