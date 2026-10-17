// Fuzzy matching for the command palette: the query's characters must appear in
// the text in order; matches at word starts and in runs score higher.

export interface FuzzyMatch {
  /** Higher is better. */
  score: number;
  /** Indices of the matched characters in the text, ascending. */
  indices: number[];
}

const isWordStart = (text: string, i: number) => i === 0 || /[\s\-_/.,:()]/.test(text[i - 1]);

/**
 * Matches `query` against `text`, ignoring case and spaces in the query. Returns null when
 * the characters do not all appear in order. An empty query matches everything with score 0.
 */
export function fuzzyMatch(query: string, text: string): FuzzyMatch | null {
  const q = query.toLowerCase().replace(/\s+/g, '');
  if (!q) return { score: 0, indices: [] };
  const t = text.toLowerCase();

  // Greedy left-to-right pass; a character that does not continue a run jumps to the
  // next word start that has it, so initials ("gtl") land on the words ("Go to list").
  const indices: number[] = [];
  let from = 0;
  for (const ch of q) {
    let at = t.indexOf(ch, from);
    // A jump to a word start can use up characters a later query character needed.
    if (at < 0) return plainMatch(q, text, t);
    const prev = indices.at(-1);
    if (prev === undefined || at !== prev + 1) {
      for (let j = at; j < t.length; j++) {
        if (t[j] === ch && isWordStart(text, j)) {
          at = j;
          break;
        }
      }
    }
    indices.push(at);
    from = at + 1;
  }
  return { score: score(text, indices), indices };
}

function plainMatch(q: string, text: string, t: string): FuzzyMatch | null {
  const indices: number[] = [];
  let from = 0;
  for (const ch of q) {
    const at = t.indexOf(ch, from);
    if (at < 0) return null;
    indices.push(at);
    from = at + 1;
  }
  return { score: score(text, indices), indices };
}

function score(text: string, indices: number[]): number {
  let s = 0;
  indices.forEach((at, k) => {
    s += 1;
    if (isWordStart(text, at)) s += 8;
    if (k > 0 && at === indices[k - 1] + 1) s += 5;
  });
  if (indices[0] === 0) s += 4;
  // Shorter texts and earlier, tighter matches win ties.
  const span = indices[indices.length - 1] - indices[0];
  return s - span * 0.1 - text.length * 0.01;
}

/** Splits `text` into runs of matched and unmatched characters, for highlighting. */
export function highlight(text: string, indices: number[]): { text: string; match: boolean }[] {
  const hit = new Set(indices);
  const parts: { text: string; match: boolean }[] = [];
  for (let i = 0; i < text.length; i++) {
    const match = hit.has(i);
    const last = parts.at(-1);
    if (last && last.match === match) last.text += text[i];
    else parts.push({ text: text[i], match });
  }
  return parts;
}
