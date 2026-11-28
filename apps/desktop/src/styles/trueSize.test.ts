// The interface size (E58.7, ADR 0029): the webview zooms the whole page, and every
// surface that shows a font takes the zoom back through ONE shared piece, the class
// `fm-true-size` in theme.css. This fails when a component draws a face (it holds one
// with `useHeldFace`, names a face's CSS family, or binds a `fontFamily` style) and
// its template has no `fm-true-size`, unless ALLOWED names it with a reason; and when
// any file but theme.css writes its own zoom rule or reads the interface factor.
import { describe, expect, it } from 'vitest';

const raw = (glob: Record<string, string>) =>
  Object.fromEntries(
    Object.entries(glob).map(([path, source]) => [
      path.replace(/^\.\.\//, '').replace(/^\.\//, 'styles/'),
      source,
    ]),
  );

/** Every .vue file under src, by its path from src (`components/X.vue`). */
const COMPONENTS = raw(
  import.meta.glob<string>('../**/*.vue', { query: '?raw', import: 'default', eager: true }),
);
/** Every .css file under src. */
const STYLESHEETS = raw(
  import.meta.glob<string>('../**/*.css', { query: '?raw', import: 'default', eager: true }),
);

/** The class that takes the zoom back. */
const TRUE_SIZE = 'fm-true-size';

/**
 * Components that draw a face but not at a size the user set: a font's name or a
 * glyph in it at the size of the chrome, which grows with the interface like any
 * label. Each with its reason.
 */
const ALLOWED: Record<string, string> = {
  'components/ui/UiSelect.vue':
    "An option with a fontFamily shows the family's name in its font at the menu's text size: chrome.",
  'components/StudioRolesRail.vue':
    "The rail names each role's family in its font at the rail's text size; the Studio paper is FontLayout's.",
  'components/LayoutBlock.vue':
    "A block of the Studio paper: it renders only inside FontLayout's page, which takes the zoom back.",
  'components/WallCountLine.vue':
    'The sort chip shows the liked glyph in its font at the count line’s text size: chrome.',
};

/** What a component does when it draws a face. */
const DRAWS_A_FACE = /\buseHeldFace\(|\bfaceFamily\(|\bcatalogFamily\(|\bfontFamily\s*:/;

function template(source: string): string {
  const start = source.indexOf('<template');
  const end = source.lastIndexOf('</template>');
  return start < 0 || end < 0 ? '' : source.slice(start, end);
}

describe('the font surfaces take the interface zoom back', () => {
  it('finds the components that draw a face', () => {
    // A pattern that stopped matching would pass every component below.
    const drawing = Object.keys(COMPONENTS).filter((p) => DRAWS_A_FACE.test(COMPONENTS[p]));
    expect(drawing).toEqual(
      expect.arrayContaining(['components/FontPreviewRow.vue', 'views/FamilyView.vue']),
    );
    expect(drawing.length).toBeGreaterThan(10);
  });

  it('every component that draws a face uses the shared class', () => {
    const missing = Object.entries(COMPONENTS)
      .filter(([path, source]) => DRAWS_A_FACE.test(source) && !(path in ALLOWED))
      .filter(([, source]) => !template(source).includes(TRUE_SIZE))
      .map(([path]) => path);
    expect(missing, `draw a face without ${TRUE_SIZE}`).toEqual([]);
  });

  it('every allowed component still draws a face', () => {
    const stale = Object.keys(ALLOWED).filter((p) => !DRAWS_A_FACE.test(COMPONENTS[p] ?? ''));
    expect(stale).toEqual([]);
  });

  it('no component or stylesheet but theme.css writes its own zoom', () => {
    const own = [
      ...Object.entries(COMPONENTS),
      ...Object.entries(STYLESHEETS).filter(([p]) => p !== 'styles/theme.css'),
    ]
      .filter(([, source]) => /(^|[\s;{"'])zoom\s*:|--fm-interface-factor/m.test(source))
      .map(([path]) => path);
    expect(own).toEqual([]);
  });

  it('the shared class divides by the factor, on the screen only, once', () => {
    const css = STYLESHEETS['styles/theme.css'];
    const screen = css.slice(css.indexOf('@media screen'));
    expect(screen).toMatch(
      /\.fm-true-size\s*\{\s*zoom:\s*calc\(1 \/ var\(--fm-interface-factor\)\);\s*\}/,
    );
    expect(screen).toMatch(/\.fm-true-size \.fm-true-size\s*\{\s*zoom:\s*1;\s*\}/);
    expect(css.slice(0, css.indexOf('@media screen'))).not.toMatch(/\.fm-true-size\s*\{/);
  });
});
