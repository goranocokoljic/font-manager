// stats.mjs — the numbers behind every dashboard view. Pure: no DOM, no files.
// Loaded by the page (served as /stats.mjs) and by the tests, so the browser
// and `node --test` compute exactly the same thing.
//
// Inputs are the go-harness telemetry records (dev-docs/harness/go-harness.md
// → Telemetry): stories (stories.jsonl), runs (tasks.jsonl, one per claude
// stage run), cycles (review-cycles.jsonl).
//
// Real vs. estimated: every `billed_*` is the authoritative total_cost_usd of
// claude runs. Phase `est_cost_usd` is an apportionment and is never summed
// into a bill here.

export const STAGES = ['build', 'ci-fix', 'review', 'finalize'];
export const LENSES = ['SPEC', 'COR', 'TST', 'REUSE', 'FFI', 'A11Y'];
export const DONE = new Set(['ready', 'merged']);
export const OS_LIST = ['windows', 'macos', 'linux', 'bench'];
export const TREND_WINDOW = 5;

export function median(arr) {
  const a = arr.filter((v) => v != null && !Number.isNaN(v)).sort((x, y) => x - y);
  if (!a.length) return null;
  const m = a.length >> 1;
  return a.length % 2 ? a[m] : (a[m - 1] + a[m]) / 2;
}

const sum = (arr, f = (x) => x) => arr.reduce((a, x) => a + (Number(f(x)) || 0), 0);

export function dayKey(d) {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

/**
 * One row per story issue. A story that was blocked and later re-run has
 * several records: costs and attempts add up, the newest record decides the
 * outcome and the rest of the fields.
 */
export function collapseStories(records) {
  const byIssue = new Map();
  for (const r of [...records].sort((a, b) => Date.parse(a.ts) - Date.parse(b.ts))) {
    const prev = byIssue.get(r.issue);
    if (!prev) { byIssue.set(r.issue, { ...r, attempts: 1, ci_failed_platforms: [...(r.ci_failed_platforms ?? [])] }); continue; }
    byIssue.set(r.issue, {
      ...r,
      attempts: prev.attempts + 1,
      billed_usd: (prev.billed_usd ?? 0) + (r.billed_usd ?? 0),
      ci_attempts: (prev.ci_attempts ?? 0) + (r.ci_attempts ?? 0),
      ci_failed_platforms: [...prev.ci_failed_platforms, ...(r.ci_failed_platforms ?? [])],
      review_rounds: (prev.review_rounds ?? 0) + (r.review_rounds ?? 0),
      must_total: (prev.must_total ?? 0) + (r.must_total ?? 0),
      must_fixed: (prev.must_fixed ?? 0) + (r.must_fixed ?? 0),
      should_total: (prev.should_total ?? 0) + (r.should_total ?? 0),
      rejected_total: (prev.rejected_total ?? 0) + (r.rejected_total ?? 0),
      purecontext_calls: (prev.purecontext_calls ?? 0) + (r.purecontext_calls ?? 0),
      first_claim_ts: prev.first_claim_ts ?? prev.claim_ts,
    });
  }
  return [...byIssue.values()].map((s) => ({ ...s, date: new Date(s.ts), first_claim_ts: s.first_claim_ts ?? s.claim_ts }));
}

/** filter: { period: 'all'|'90'|'30'|'7', lane: 'all'|'core'|'ui', outcome: 'all'|'done'|'blocked' } */
export function filterStories(stories, filter, now = new Date()) {
  const cutoff = filter.period && filter.period !== 'all' ? new Date(now.getTime() - Number(filter.period) * 86400000) : null;
  return stories
    .filter((s) => !cutoff || s.date >= cutoff)
    .filter((s) => !filter.lane || filter.lane === 'all' || s.lane === filter.lane)
    .filter((s) => {
      if (!filter.outcome || filter.outcome === 'all') return true;
      return filter.outcome === 'done' ? DONE.has(s.outcome) : !DONE.has(s.outcome);
    })
    .sort((a, b) => a.date - b.date);
}

/** Runs and cycles that belong to the visible stories. */
export function scope(list, stories) {
  const keep = new Set(stories.map((s) => s.issue));
  return list.filter((x) => keep.has(x.issue));
}

export function kpis(stories, runs, cycles) {
  const done = stories.filter((s) => DONE.has(s.outcome));
  const blocked = stories.filter((s) => !DONE.has(s.outcome));
  const verdicts = lensVerdicts(cycles);
  const confirmed = sum(Object.values(verdicts), (v) => v.must);
  const rejected = sum(Object.values(verdicts), (v) => v.rejected);
  const full = cycles.filter((c) => c.review_cycle === 1);
  return {
    stories: stories.length,
    done: done.length,
    merged: stories.filter((s) => s.outcome === 'merged').length,
    blocked: blocked.length,
    billed: sum(runs, (r) => r.billed_cost_usd),
    medianBilledDone: median(done.map((s) => s.billed_usd)),
    overBudget: stories.filter((s) => s.budget_usd && s.billed_usd > s.budget_usd).length,
    medianClaimToGreen: median(done.map((s) => s.claim_to_green_sec)),
    ciFirstPass: stories.filter((s) => (s.ci_attempts ?? 0) === 0 && s.green_ts).length,
    ciReached: stories.filter((s) => s.green_ts).length,
    mustPerStory: median(full.map((c) => c.findings?.must ?? 0)),
    mustFixed: sum(stories, (s) => s.must_fixed),
    should: sum(full, (c) => c.findings?.should),
    // Of the must findings the verifier judged, the share it threw out.
    rejectRate: confirmed + rejected ? rejected / (confirmed + rejected) : null,
    rejected,
    reviewRounds: median(stories.map((s) => s.review_rounds)),
    pcPerStory: median(stories.map((s) => s.purecontext_calls)),
    pcZero: stories.filter((s) => !s.purecontext_calls).length,
  };
}

/** Billed cost per calendar day, split by stage; gaps between days are kept. */
export function costByDay(runs) {
  const dated = runs.map((r) => ({ ...r, date: new Date(r.ts) })).sort((a, b) => a.date - b.date);
  if (!dated.length) return [];
  const byDay = new Map();
  for (const r of dated) {
    const k = dayKey(r.date);
    if (!byDay.has(k)) byDay.set(k, []);
    byDay.get(k).push(r);
  }
  const days = [];
  const d = new Date(dated[0].date.getFullYear(), dated[0].date.getMonth(), dated[0].date.getDate());
  const end = dated[dated.length - 1].date;
  while (d <= end) {
    const list = byDay.get(dayKey(d)) ?? [];
    const sums = Object.fromEntries(STAGES.map((st) => [st, sum(list.filter((r) => r.stage === st), (r) => r.billed_cost_usd)]));
    days.push({ date: new Date(d), sums, total: sum(STAGES, (st) => sums[st]), issues: [...new Set(list.map((r) => r.id ?? `#${r.issue}`))] });
    d.setDate(d.getDate() + 1);
  }
  return days;
}

/**
 * Confirmed must findings in each story's FIRST review cycle (the full review),
 * with a rolling average. Falling = the KB and the implementer notes are working.
 */
export function mustTrend(cycles, stories) {
  const order = new Map(stories.map((s) => [s.issue, s]));
  const pts = cycles
    .filter((c) => c.review_cycle === 1 && order.has(c.issue))
    .sort((a, b) => Date.parse(a.ts) - Date.parse(b.ts))
    .map((c) => ({ issue: c.issue, id: c.id, date: new Date(c.ts), must: c.findings?.must ?? 0, should: c.findings?.should ?? 0, rejected: c.findings?.rejected ?? 0 }));
  pts.forEach((p, i) => {
    const w = pts.slice(Math.max(0, i - TREND_WINDOW + 1), i + 1);
    p.roll = sum(w, (q) => q.must) / w.length;
  });
  return pts;
}

/** The verifier's verdicts per lens, summed over full reviews (cycle 1). */
export function lensVerdicts(cycles) {
  const out = {};
  for (const c of cycles) {
    if (c.review_cycle !== 1 || !c.by_lens) continue;
    for (const [lens, v] of Object.entries(c.by_lens)) {
      out[lens] ??= { must: 0, should: 0, rejected: 0 };
      out[lens].must += v.must ?? 0;
      out[lens].should += v.should ?? 0;
      out[lens].rejected += v.rejected ?? 0;
    }
  }
  return out;
}

export function lensRows(cycles) {
  const v = lensVerdicts(cycles);
  const names = [...LENSES.filter((l) => v[l]), ...Object.keys(v).filter((l) => !LENSES.includes(l))];
  return names.map((lens) => {
    const r = v[lens];
    const judged = r.must + r.rejected;
    return { lens, ...r, total: r.must + r.should + r.rejected, rejectRate: judged ? r.rejected / judged : null };
  });
}

/** What each review cycle did: full review vs verify, and the fix outcome. */
export function cycleRows(cycles) {
  const byCycle = new Map();
  for (const c of cycles) {
    const k = c.review_cycle;
    const r = byCycle.get(k) ?? { cycle: k, mode: k === 1 ? 'full' : 'verify', runs: 0, must: 0, should: 0, rejected: 0, resolved: 0, fixed: 0, disputed: 0, reviewSec: [], fixSec: [] };
    r.runs++;
    for (const f of ['must', 'should', 'rejected', 'resolved']) r[f] += c.findings?.[f] ?? 0;
    r.fixed += c.dispositions?.fixed ?? 0;
    r.disputed += c.dispositions?.disputed ?? 0;
    if (c.review_sec != null) r.reviewSec.push(c.review_sec);
    if (c.fix_sec != null) r.fixSec.push(c.fix_sec);
    byCycle.set(k, r);
  }
  return [...byCycle.values()].sort((a, b) => a.cycle - b.cycle)
    .map((r) => ({ ...r, medianReviewSec: median(r.reviewSec), medianFixSec: median(r.fixSec) }));
}

/** Per stage: median run time, real billed total and its share of all billed cost. */
export function stageRows(runs, stories) {
  const total = sum(runs, (r) => r.billed_cost_usd) || 1;
  const rows = STAGES.map((st) => {
    const list = runs.filter((r) => r.stage === st);
    const billed = sum(list, (r) => r.billed_cost_usd);
    return { stage: st, runs: list.length, medianSec: median(list.map((r) => r.total_sec)), billed, share: billed / total, pc: sum(list, (r) => r.purecontext_calls) };
  });
  // CI waiting is harness time, not a claude run: wall clock minus the stage runs.
  const waits = stories.map((s) => {
    const own = sum(runs.filter((r) => r.issue === s.issue), (r) => r.total_sec);
    return s.wall_sec != null ? Math.max(0, s.wall_sec - own) : null;
  });
  rows.push({ stage: 'ci wait', runs: stories.length, medianSec: median(waits), billed: 0, share: 0, pc: 0 });
  return rows;
}

/** CI failures per OS (and bench), counted over attempts. */
export function ciByOs(stories) {
  const counts = Object.fromEntries(OS_LIST.map((o) => [o, 0]));
  for (const s of stories) for (const o of s.ci_failed_platforms ?? []) counts[o] = (counts[o] ?? 0) + 1;
  return Object.entries(counts).map(([os, failures]) => ({ os, failures }));
}

export function budgetRows(stories) {
  return [...stories]
    .sort((a, b) => b.date - a.date)
    .map((s) => ({ issue: s.issue, id: s.id, size: s.size, billed: s.billed_usd ?? 0, budget: s.budget_usd ?? null, over: s.budget_usd ? s.billed_usd > s.budget_usd : false }));
}

export function epicRows(stories) {
  const byEpic = new Map();
  for (const s of stories) {
    const e = s.epic ?? String(s.id ?? '').split('.')[0];
    const r = byEpic.get(e) ?? { epic: e, stories: 0, done: 0, blocked: 0, billed: 0, ids: [] };
    r.stories++;
    if (DONE.has(s.outcome)) r.done++; else r.blocked++;
    r.billed += s.billed_usd ?? 0;
    r.ids.push(s.id);
    byEpic.set(e, r);
  }
  const n = (e) => Number(String(e).replace(/\D/g, '')) || 0;
  return [...byEpic.values()].sort((a, b) => n(a.epic) - n(b.epic));
}

export function lessonSummary(lessons) {
  const by = { candidate: 0, active: 0, graduated: 0, retired: 0 };
  for (const l of lessons) by[l.status] = (by[l.status] ?? 0) + 1;
  return by;
}
