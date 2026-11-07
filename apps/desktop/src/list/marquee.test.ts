import { describe, expect, it } from 'vitest';
import { boxBetween, cardsInBox, isClick, type WallGeometry } from './marquee';

// Three columns of 100 × 50 cards, 10 px apart, 20 px from the start edge; 8 cards.
// Card i: inline from 20 + (i % 3) * 110, block from 10 + floor(i / 3) * 60.
const g: WallGeometry = {
  count: 8,
  columns: 3,
  cardWidth: 100,
  cardHeight: 50,
  linePx: 60,
  gap: 10,
  inset: 20,
};

describe('the wall selection box', () => {
  it('is the same box whichever way the drag went', () => {
    expect(boxBetween({ inline: 50, block: 90 }, { inline: 10, block: 5 })).toEqual({
      inline0: 10,
      block0: 5,
      inline1: 50,
      block1: 90,
    });
  });

  it('touches only the card it overlaps', () => {
    // Inside card 4 (inline 130–230, block 70–120).
    expect(
      cardsInBox(boxBetween({ inline: 140, block: 80 }, { inline: 150, block: 90 }), g),
    ).toEqual([4]);
  });

  it('selects every card it touches, even by a corner, in list order', () => {
    // From the gap left of card 1 down into card 4: cards 1 and 4; wider, into 2 and 5 too.
    const box = boxBetween({ inline: 230, block: 115 }, { inline: 125, block: 20 });
    expect(cardsInBox(box, g)).toEqual([1, 4]);
    const wide = boxBetween({ inline: 125, block: 20 }, { inline: 300, block: 115 });
    expect(cardsInBox(wide, g)).toEqual([1, 2, 4, 5]);
  });

  it('touches nothing in the gaps between cards', () => {
    // The gap between card 0 and card 1 (inline 120–130), between line 0 and 1 (block 60–70).
    expect(
      cardsInBox(boxBetween({ inline: 121, block: 61 }, { inline: 129, block: 69 }), g),
    ).toEqual([]);
    // The inset before the first column.
    expect(cardsInBox(boxBetween({ inline: 0, block: 0 }, { inline: 19, block: 300 }), g)).toEqual(
      [],
    );
  });

  it('stops at the last card, not the last column of the last line', () => {
    // Line 2 has cards 6 and 7 only.
    const box = boxBetween({ inline: 0, block: 125 }, { inline: 999, block: 999 });
    expect(cardsInBox(box, g)).toEqual([6, 7]);
  });

  it('selects cards off screen: it uses the layout, not the rendered cards', () => {
    const many = { ...g, count: 3000 };
    const box = boxBetween(
      { inline: 20, block: 10 + 900 * 60 },
      { inline: 25, block: 15 + 900 * 60 },
    );
    expect(cardsInBox(box, many)).toEqual([2700]);
  });

  it('touches nothing on an empty wall', () => {
    expect(
      cardsInBox(boxBetween({ inline: 0, block: 0 }, { inline: 500, block: 500 }), {
        ...g,
        count: 0,
      }),
    ).toEqual([]);
  });

  it('tells a click from a box', () => {
    expect(isClick(boxBetween({ inline: 10, block: 10 }, { inline: 12, block: 13 }))).toBe(true);
    expect(isClick(boxBetween({ inline: 10, block: 10 }, { inline: 10, block: 30 }))).toBe(false);
  });
});
