# Motion

The table the redesign epics implement against. It is board 6 of the concept canvas, written
out; the picture is [6-motion.png](../design/redesign/6-motion.png).

**Everything moves for a reason, and nothing bounces.** Three durations, one easing. Motion
explains where something came from or went to; it never decorates. Every transition below has
a reduced-motion fallback (an instant cut or a plain fade), and the app reads
`prefers-reduced-motion` once at start and on change.

## Tokens

In `apps/desktop/src/styles/theme.css`. All three durations are `0ms` under
`prefers-reduced-motion: reduce`.

| Token              | Value                     | For                                                                                  |
|--------------------|---------------------------|--------------------------------------------------------------------------------------|
| `--fm-motion-fast` | 120 ms                    | hover, press, toggle, chip, focus ring                                               |
| `--fm-motion-move` | 200 ms                    | panels, drawers, popovers, reflow, selection bar                                     |
| `--fm-motion-page` | 280 ms                    | family page push and pop, Studio open, Quick Look                                    |
| `--fm-ease`        | `cubic-bezier(.2, 0, 0, 1)` | the only curve; exits may use `--fm-ease-exit` at 0.8× the duration                |
| `--fm-ease-exit`   | `ease-in`                 | exits, such as "Back to the wall"                                                    |

## Moments

| Moment | What moves | How | Duration | Reduced motion |
|--------|------------|-----|----------|----------------|
| Open a family | The card grows into the page: specimen scales up to the hero position, the wall fades under it, the top bar crossfades to the back button | Shared-element (View Transitions); fallback: page slides in 24 px from the right over a fade | 280 ms | Cut, with a 120 ms fade of the new page |
| Back to the wall | The hero shrinks back into its card; the wall fades in; scroll position is where it was | Reverse of open, exit curve | 220 ms | Cut |
| Size slider | Every visible card resizes and the grid reflows; cards keep their identity | FLIP on the visible cards; off-screen cards jump | 200 ms | Instant reflow |
| Switch view (wall → lines → table) | Rows crossfade; the toolbar segmented thumb slides to the new segment | Opacity crossfade of the two lists; thumb translateX | 200 ms / 120 ms | Cut; thumb jumps |
| Filters drawer | Slides in from the left edge over the sidebar; the wall does not move; a 6 % scrim | translateX(−100 % → 0) + scrim opacity | 200 ms | Fade only |
| Type in the preview field | Nothing animates: every specimen repaints as you type | — | 0 | — |
| Select a card | Ring draws in; the selection bar rises from the bottom edge on the first selection | Ring opacity; bar translateY(16 px → 0) + opacity | 120 ms / 200 ms | Ring and bar appear |
| Drag to a collection | The ghost lifts (scale 0.9, −3°, shadow) and follows the pointer; the target row gains its ring as the pointer enters; drop: the ghost flies into the row and the count ticks up | transform on the ghost; count uses a 120 ms crossfade of old and new number | 120 ms lift / 200 ms drop | Ghost follows without tilt; drop is a cut |
| Activate | The dot fills; on the family page the switch knob slides; a quiet toast rises | Dot scale 0 → 1; knob translateX; toast translateY + opacity | 120 ms / 200 ms | States change at once |
| Quick Look (Space) | The specimen sheet fades up over a blurred wall; ← → crossfade the family inside the sheet | Opacity + translateY(8 px); backdrop blur fixed, not animated | 200 ms / 120 ms | Fade only, no blur |
| Open the Studio | The page lifts off the paper ground and the roles rail slides in | Paper scale .98 → 1 + opacity; rail translateX | 280 ms | Fade only |
| Hover anything | Tint eases in on pointer enter and out on leave | background-color on the tint layer only | 120 ms | Same (hover tint is not motion) |

## Helpers

In `apps/desktop/src/motion/`. They animate `transform` and `opacity` only.

| Helper | Use it for |
|--------|------------|
| `useReducedMotion()` | motion that a script decides on; CSS follows the tokens by itself |
| `<FmFade speed>` | crossfades and fade-ins: a ring, a count, a list (`mode="out-in"` passes through) |
| `<FmSlide from distance speed>` | a sheet, drawer, bar or toast: `from="start" distance="100%"` for the drawer, `from="bottom" distance="16px"` for the selection bar |
| `<FmFlipGroup tag>` | keyed children that change place (`--fm-motion-move`) |
| `withViewTransition(fn)` | a page change; it runs `fn` directly without view transitions or under reduced motion |

`speed` is `fast`, `move` (the default) or `page`.

## Where the board and the tokens do not line up yet

The story that builds each moment decides these, without a literal value in a component:

- **Fades under reduced motion.** The board keeps a fade in some rows ("Fade only", "Cut, with
  a 120 ms fade"), but all three tokens are `0ms` and `theme.css` turns every transition off
  under reduced motion. Today those rows are a cut. A fade needs its own token and a way past
  that rule.
- **Hover under reduced motion.** The board keeps the tint ("Same"); today it is instant, for
  the same reason.
- **Exits.** The board allows `ease-in` at 0.8× the duration (220 ms for "Back to the wall").
  The curve is `--fm-ease-exit`; the time is `calc(var(--fm-motion-page) * 0.8)` (224 ms).
  The helpers leave on the same time and curve as they enter.
- **Other properties.** "Hover anything" animates `background-color`, and the ghost's shadow
  is not a transform. Those are not the helpers' job; the helpers stay transform and opacity.
