#!/usr/bin/env node
// pick.mjs — choose the next story for a go-harness lane.
//
// Usage:
//   node scripts/harness/pick.mjs --lane core                 # JSON: what the lane should do next
//   node scripts/harness/pick.mjs --lane core --claim         # ...and claim the pick (labels + assignee)
//   node scripts/harness/pick.mjs --lane ui --issues 61,62    # restrict candidates to these issues
//   node scripts/harness/pick.mjs --dry-run                   # human-readable plan for every lane; touches nothing
//
// Definition of Ready (the only condition under which a lane may start a story):
//   1. label type:story
//   2. none of: human, criteria:tbd, harness:in-progress, harness:blocked
//   3. every issue in blocked_by is closed (GitHub dependencies API, re-read every time)
//   4. open, and unassigned or assigned to the harness bot
// Then, per lane:
//   - one story in an ACTIVE stage per lane (claimed, PR missing or still draft) -> resume it
//   - at most `lane_max_unmerged` ready-for-human PRs waiting per lane (merge: human only)
//   - merge: auto -> a claimed story with a ready PR goes back through CI and the
//     merge gate (it stopped on a red main, or the lane stopped), unless it is
//     labelled harness:human-merge (left for a person on purpose)
//   - a candidate whose predicted areas overlap an open PR of a story the OTHER lane
//     still works on waits (blocked and human-merge PRs do not count)
// Ordering inside a lane: phase, epic number, story number, size.
//
// Output (stdout, one JSON object unless --dry-run):
//   { lane, action: "resume"|"start"|"wait"|"idle", story, reasons[], waiting_on_human[], in_flight }
//   in_flight: stories claimed by the harness in ANY lane (not left for a human).
//   A lane that is idle while in_flight > 0 may find work once those land.

import {
  loadConfig, laneOf, parseMeta, compareStories, predictAreas, areaOf, areasOverlap,
  storyBudget, gh, ghJson, blockedBy, currentLogin, labelNames, parseArgs, isMain,
} from './lib.mjs';

const EXCLUDE = ['human', 'criteria:tbd', 'harness:in-progress', 'harness:blocked'];
export const HUMAN_MERGE = 'harness:human-merge';
const STORY_BRANCH = /^(E\d+\.\d+)-/;
const MAX_CHECKED = 15; // blocked_by calls per lane per pick

export function loadWorld() {
  const stories = (ghJson([
    'issue', 'list', '--label', 'type:story', '--state', 'open', '--limit', '1000',
    '--json', 'number,title,body,labels,assignees,state',
  ]) ?? []).map((s) => ({ ...s, meta: parseMeta(s.body) ?? {}, labels: labelNames(s) }));

  const prs = (ghJson([
    'pr', 'list', '--state', 'open', '--limit', '200',
    '--json', 'number,headRefName,isDraft,files,url',
  ]) ?? []).map((p) => ({
    ...p,
    storyId: STORY_BRANCH.exec(p.headRefName)?.[1] ?? null,
    areas: [...new Set((p.files ?? []).map((f) => areaOf(f.path)).filter(Boolean))],
  }));
  return { stories, prs };
}

/** Static Definition-of-Ready checks (1, 2, 4). Returns a reason string or null. */
export function staticReadiness(story, bot) {
  if (!story.labels.includes('type:story')) return 'not a type:story';
  const bad = EXCLUDE.filter((l) => story.labels.includes(l));
  if (bad.length) return `label ${bad.join(', ')}`;
  if (story.meta.human) return 'fm-meta human: true';
  if (story.state && story.state !== 'OPEN') return 'not open';
  const who = (story.assignees ?? []).map((a) => a.login);
  if (who.length && !(bot && who.length === 1 && who[0] === bot)) return `assigned to ${who.join(', ')}`;
  return null;
}

/** Stories waiting on a `human` story, with the ids they block (fm-meta depends). */
export function humanWaits(stories) {
  const open = stories.filter((s) => s.labels.includes('human') || s.meta.human);
  return open
    .map((h) => ({
      number: h.number,
      id: h.meta.id,
      title: h.title,
      blocks: stories.filter((s) => s.meta.depends?.includes(h.meta.id)).map((s) => s.meta.id),
    }))
    .sort((a, b) => compareStories({ meta: { id: a.id } }, { meta: { id: b.id } }));
}

export function planLane(lane, world, cfg, { issues = null, bot = '', checkDeps = blockedBy } = {}) {
  const { stories, prs } = world;
  const prOf = (s) => prs.find((p) => p.storyId === s.meta.id) ?? null;
  const mine = stories.filter((s) => laneOf(s.meta.track, cfg) === lane);
  const reasons = [];
  const inFlight = stories.filter((s) => s.labels.includes('harness:in-progress') && !s.labels.includes(HUMAN_MERGE)).length;
  const result = (action, story, extra = {}) => ({
    lane, action, story: story ? describe(story, cfg, prOf(story)) : null, reasons, ...extra,
    waiting_on_human: humanWaits(stories), in_flight: inFlight,
  });

  // Lane capacity: an active story (claimed, PR missing or draft) is resumed, not doubled.
  const claimed = mine.filter((s) => s.labels.includes('harness:in-progress'));
  const active = claimed.filter((s) => { const p = prOf(s); return !p || p.isDraft; });
  if (active.length) return result('resume', active.sort(compareStories)[0]);
  const waiting = claimed.filter((s) => prOf(s) && !prOf(s).isDraft);
  // With merge: auto a ready PR not left for a human goes back to the merge
  // gate; the ones labelled harness:human-merge do not hold the lane.
  if (cfg.merge === 'auto') {
    const again = waiting.filter((s) => !s.labels.includes(HUMAN_MERGE));
    if (again.length) return result('resume', again.sort(compareStories)[0]);
  } else if (waiting.length >= cfg.lane_max_unmerged) {
    reasons.push({ why: `${waiting.length} PR(s) wait for a human merge: ${waiting.map((s) => `#${prOf(s).number}`).join(', ')}` });
    return result('wait', null);
  }

  // Areas other lanes are working in right now: open PRs of stories still in
  // flight. A PR that is blocked or waits for a human merge holds no area; if
  // it lands later, the sync stage resolves the conflict.
  const working = (s) => s && s.labels.includes('harness:in-progress')
    && !s.labels.includes(HUMAN_MERGE) && !s.labels.includes('harness:blocked');
  const otherAreas = prs
    .filter((p) => {
      const s = p.storyId && stories.find((x) => x.meta.id === p.storyId);
      return working(s) && laneOf(s.meta.track, cfg) !== lane;
    })
    .flatMap((p) => p.areas.map((a) => ({ area: a, pr: p.number, story: p.storyId })));

  let candidates = mine.slice().sort(compareStories);
  if (issues) candidates = candidates.filter((s) => issues.includes(s.number));
  let checked = 0;
  for (const s of candidates) {
    const why = staticReadiness(s, bot);
    if (why) { if (issues) reasons.push({ number: s.number, id: s.meta.id, why }); continue; }
    if (checked++ >= MAX_CHECKED) break;
    const open = checkDeps(s.number).filter((d) => d.state !== 'closed');
    if (open.length) {
      reasons.push({ number: s.number, id: s.meta.id, why: `blocked by open ${open.map((d) => `#${d.number}`).join(', ')}` });
      continue;
    }
    const predicted = predictAreas(s);
    const clash = otherAreas.filter((o) => predicted.some((a) => areasOverlap(a, o.area)));
    if (clash.length) {
      reasons.push({ number: s.number, id: s.meta.id, why: `overlaps ${clash.map((c) => `${c.area} (PR #${c.pr}, ${c.story})`).join(', ')}` });
      continue;
    }
    return result('start', s);
  }
  if (!candidates.length) reasons.push({ why: issues ? 'none of the given issues belong to this lane' : 'no open stories in this lane' });
  return result('idle', null);
}

function describe(s, cfg, pr) {
  return {
    number: s.number, id: s.meta.id, title: s.title, epic: s.meta.epic, phase: s.meta.phase,
    size: s.meta.size, track: s.meta.track, budget_usd: storyBudget(s.meta.size, cfg),
    pr: pr ? { number: pr.number, draft: pr.isDraft, branch: pr.headRefName, url: pr.url } : null,
  };
}

export function claim(number, bot) {
  gh(['issue', 'edit', String(number), '--add-label', 'harness:in-progress', '--remove-label', 'harness:ready',
    ...(bot ? ['--add-assignee', bot] : [])]);
}

function printDry(plans) {
  for (const p of plans) {
    console.log(`\n== lane ${p.lane}: ${p.action.toUpperCase()}`);
    if (p.story) console.log(`   -> #${p.story.number} ${p.story.title}  [${p.story.size}, ${p.story.track}, budget $${p.story.budget_usd}]`);
    for (const r of p.reasons) console.log(`   skip ${r.id ? `#${r.number} ${r.id}` : ''}: ${r.why}`);
  }
  const hw = plans[0]?.waiting_on_human ?? [];
  if (hw.length) {
    console.log('\n== waiting on a human');
    for (const h of hw) console.log(`   #${h.number} ${h.title}${h.blocks.length ? `  -> blocks ${h.blocks.join(', ')}` : ''}`);
  }
}

if (isMain(import.meta.url)) {
  const args = parseArgs(process.argv.slice(2));
  const cfg = loadConfig();
  const bot = cfg.bot_login || currentLogin();
  const issues = args.issues ? String(args.issues).split(',').map(Number) : null;
  try {
    const world = loadWorld();
    const lanes = args.lane ? [String(args.lane)] : Object.keys(cfg.lanes);
    const plans = lanes.map((l) => planLane(l, world, cfg, { issues, bot }));
    if (args['dry-run']) { printDry(plans); process.exit(0); }
    const plan = plans[0];
    if (plan.action === 'start') {
      gh(['issue', 'edit', String(plan.story.number), '--add-label', 'harness:ready'], { allowFail: true });
      if (args.claim) claim(plan.story.number, bot);
    }
    process.stdout.write(JSON.stringify(plan) + '\n');
  } catch (e) {
    console.error(e.message);
    process.exit(3);
  }
}
