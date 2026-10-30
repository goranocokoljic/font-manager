import { describe, expect, it } from 'vitest';
import {
  FRAME_MS,
  SETTLE_MS,
  jump,
  observeScroll,
  restingAt,
  settleCheck,
  settlesAt,
  type ScrollMotion,
} from './settle';

const ROW = 64;

/** Feeds `steps` scroll events of `dy` px, `dt` ms apart. */
function scroll(m: ScrollMotion, steps: number, dy: number, dt: number): ScrollMotion {
  for (let i = 0; i < steps; i++) m = observeScroll(m, m.top + dy, m.time + dt, ROW);
  return m;
}

describe('observeScroll', () => {
  it('starts settled', () => {
    const m = restingAt(0, 0);
    expect(m.moving).toBe(false);
    expect(settlesAt(m)).toBeNull();
  });

  it('one fast frame is not a fling; two in a row are', () => {
    let m = scroll(restingAt(0, 0), 1, ROW * 3, FRAME_MS);
    expect(m.moving).toBe(false);
    m = scroll(m, 1, ROW * 3, FRAME_MS);
    expect(m.moving).toBe(true);
  });

  it('a slow frame between fast ones resets the count', () => {
    let m = scroll(restingAt(0, 0), 1, ROW * 3, FRAME_MS);
    m = scroll(m, 1, 2, FRAME_MS);
    m = scroll(m, 1, ROW * 3, FRAME_MS);
    expect(m.moving).toBe(false);
  });

  it('stays moving through a sustained fling', () => {
    let m = restingAt(0, 0);
    for (let i = 0; i < 120; i++) {
      m = scroll(m, 1, ROW * 2, FRAME_MS);
      if (i >= 1) expect(m.moving).toBe(true);
    }
    // Events faster than a frame count as one frame.
    m = scroll(m, 10, ROW * 2, 4);
    expect(m.moving).toBe(true);
  });

  it('settles once the position rests within one row for SETTLE_MS', () => {
    let m = scroll(restingAt(0, 0), 30, ROW * 4, FRAME_MS);
    expect(m.moving).toBe(true);
    // The fling slows: small moves, under one row from where it came to rest.
    m = scroll(m, 3, ROW / 4, FRAME_MS);
    expect(m.moving).toBe(true);
    const at = settlesAt(m)!;
    expect(at - m.restSince).toBe(SETTLE_MS);
    expect(settleCheck(m, at - 1).moving).toBe(true);
    const done = settleCheck(m, at);
    expect(done.moving).toBe(false);
    expect(settlesAt(done)).toBeNull();
  });

  it('a move of more than one row restarts the rest', () => {
    let m = scroll(restingAt(0, 0), 5, ROW * 4, FRAME_MS);
    const since = m.restSince;
    m = observeScroll(m, m.top + ROW * 2, m.time + 80, ROW);
    expect(m.restSince).toBeGreaterThan(since);
    expect(settleCheck(m, since + SETTLE_MS).moving).toBe(true);
  });

  it('a slow wheel (a notch every 50 ms) counts as settled', () => {
    // A wheel notch scrolls about 100 px; a 50 px row is the densest list.
    for (const row of [50, ROW, 120]) {
      let m = restingAt(0, 0);
      for (let i = 0; i < 40; i++) {
        m = observeScroll(m, m.top + 100, m.time + 50, row);
        expect(m.moving).toBe(false);
      }
    }
  });

  it('a jump is settled at once, even mid-fling', () => {
    const m = scroll(restingAt(0, 0), 10, ROW * 4, FRAME_MS);
    expect(m.moving).toBe(true);
    const j = jump(50_000, m.time);
    expect(j.moving).toBe(false);
    // The scroll event of the jump itself does not start a fling.
    expect(observeScroll(j, 50_000, j.time + 1, ROW).moving).toBe(false);
  });
});
