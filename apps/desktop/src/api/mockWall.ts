// The "wall" library of browser mode (E40.2): 60 families that look like a real
// designer's library, for the redesign's screenshots (`?mockLibrary=wall`, the default
// of the e2e @screens spec). Every family renders from a file in fixtures/fonts/
// (OFL, Apache or UFL only): `wallFixtures()` maps each face to it, and the @screens
// spec serves that file at `/fmfont/<id>`.
//
// This module imports types only, so the Playwright spec (Node) can load it.
import type {
  Axis,
  Classification,
  DuplicateGroup,
  FaceDetail,
  FaceId,
  FamilyId,
  FeatureInfo,
  FontFormat,
  LanguagePreset,
  LicenseInfo,
  Problem,
} from './types';

/** The group a family is counted in; the six `variable` families are sans or serif. */
export type WallBucket = 'serif' | 'sans' | 'slab' | 'script' | 'mono' | 'display' | 'variable';

/**
 * What a family's character set reaches: `ascii` only basic Latin (no accents: a
 * deliberate gap), `western` Latin-1 too, `european` Latin Extended-A too.
 */
type Reach = 'ascii' | 'western' | 'european';

/** One family of the wall, as its foundry ships it. */
export interface WallFamily {
  name: string;
  bucket: WallBucket;
  class: Classification;
  designer: string;
  foundry: string;
  year: number;
  license: 'ofl' | 'apache' | 'ufl';
  /** The fixture every face of the family renders from, under fixtures/fonts/. */
  file: string;
  /** Style names; a variable family has one face per entry (its upright or italic). */
  styles: string[];
  reach: Reach;
  cyrl?: boolean;
  grek?: boolean;
  viet?: boolean;
  /** The fixture's real `fvar` axes (variable families only). */
  axes?: Axis[];
}

const ax = (tag: string, name: string, min: number, def: number, max: number): Axis => ({
  tag,
  name,
  min,
  default: def,
  max,
});
const WGHT = (min: number, def: number, max: number) => ax('wght', 'Weight', min, def, max);
const WDTH = (min: number, def: number, max: number) => ax('wdth', 'Width', min, def, max);

const SIX = ['Light', 'Regular', 'Italic', 'Medium', 'Bold', 'Bold Italic'];
const FOUR = ['Regular', 'Italic', 'Bold', 'Bold Italic'];

/** The 60 families by bucket; `WALL_FAMILIES` deals them out. */
const TABLE: WallFamily[] = [
  // Variable (6): two with 3+ axes (the two Bitcount families share the fixture's 11 axes).
  {
    name: 'Asap Sharp',
    bucket: 'variable',
    class: 'sans',
    designer: 'Pablo Cosgaya',
    foundry: 'Omnibus-Type',
    year: 2024,
    license: 'ofl',
    file: 'google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
    styles: ['Regular'],
    reach: 'european',
    viet: true,
    axes: [WGHT(100, 400, 900), WDTH(75, 100, 125)],
  },
  {
    name: 'Bitcount Grid Single',
    bucket: 'variable',
    class: 'sans',
    designer: 'Petr van Blokland',
    foundry: 'TYPETR',
    year: 2025,
    license: 'ofl',
    file: 'google-repo/bitcountgridsingleink/BitcountGridSingleInk[CRSV,ELSH,ELXP,SZP1,SZP2,XPN1,XPN2,YPN1,YPN2,slnt,wght].ttf',
    styles: ['Regular'],
    reach: 'european',
    axes: [
      WGHT(100, 400, 900),
      ax('slnt', 'Slant', -8, 0, 0),
      ax('CRSV', 'Cursive', 0, 0.5, 1),
      ax('ELSH', 'Element Shape', 0, 0, 100),
      ax('ELXP', 'Element Expansion', 0, 0, 100),
    ],
  },
  {
    name: 'Bitcount Grid Single Ink',
    bucket: 'variable',
    class: 'sans',
    designer: 'Petr van Blokland',
    foundry: 'TYPETR',
    year: 2025,
    license: 'ofl',
    file: 'google-repo/bitcountgridsingleink/BitcountGridSingleInk[CRSV,ELSH,ELXP,SZP1,SZP2,XPN1,XPN2,YPN1,YPN2,slnt,wght].ttf',
    styles: ['Regular'],
    reach: 'european',
    axes: [
      WGHT(100, 400, 900),
      ax('ELXP', 'Element Expansion', 0, 0, 100),
      ax('ELSH', 'Element Shape', 0, 0, 100),
      ax('slnt', 'Slant', -8, 0, 0),
      ax('CRSV', 'Cursive', 0, 0.5, 1),
      ax('SZP1', 'Size of Paint 1', 0, 30, 100),
      ax('XPN1', 'Horizontal Position of Paint 1', -100, 0, 100),
      ax('YPN1', 'Vertical Position of Paint 1', -100, 0, 100),
      ax('SZP2', 'Size of Paint 2', 0, 30, 100),
      ax('XPN2', 'Horizontal Position of Paint 2', -100, 0, 100),
      ax('YPN2', 'Vertical Position of Paint 2', -100, 0, 100),
    ],
  },
  {
    name: 'Hedvig Letters Serif',
    bucket: 'variable',
    class: 'serif',
    designer: 'Kanon Foundry',
    foundry: 'Kanon Foundry',
    year: 2023,
    license: 'ofl',
    file: 'google-repo/hedviglettersserif/HedvigLettersSerif[opsz].ttf',
    styles: ['Regular'],
    reach: 'european',
    axes: [ax('opsz', 'Optical size', 12, 24, 24)],
  },
  {
    name: 'Sour Gummy',
    bucket: 'variable',
    class: 'sans',
    designer: 'Stefie Justprince',
    foundry: 'Stefie Justprince',
    year: 2024,
    license: 'ofl',
    file: 'google-repo/sourgummy/SourGummy-Italic[wdth,wght].ttf',
    styles: ['Italic'],
    reach: 'european',
    viet: true,
    axes: [WDTH(100, 100, 125), WGHT(100, 900, 900)],
  },
  {
    name: 'Tourney',
    bucket: 'variable',
    class: 'sans',
    designer: 'Tyler Finck',
    foundry: 'Etcetera Type Co',
    year: 2021,
    license: 'ofl',
    file: 'google-repo/tourney/Tourney-Italic[wdth,wght].ttf',
    styles: ['Italic'],
    reach: 'european',
    viet: true,
    axes: [WGHT(100, 100, 900), WDTH(75, 100, 125)],
  },

  // Serif (12).
  {
    name: 'Crimson Pro',
    bucket: 'serif',
    class: 'serif',
    designer: 'Jacques Le Bailly',
    foundry: 'Jacques Le Bailly',
    year: 2018,
    license: 'ofl',
    file: 'google-repo/crimsonpro/CrimsonPro[wght].ttf',
    styles: SIX,
    reach: 'european',
    viet: true,
  },
  {
    name: 'Libre Baskerville',
    bucket: 'serif',
    class: 'serif',
    designer: 'Pablo Impallari',
    foundry: 'Impallari Type',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/librebaskerville/LibreBaskerville[wght].ttf',
    styles: ['Regular', 'Italic', 'Bold'],
    reach: 'european',
  },
  {
    name: 'Libre Bodoni',
    bucket: 'serif',
    class: 'serif',
    designer: 'Pablo Impallari, Rodrigo Fuenzalida',
    foundry: 'Impallari Type',
    year: 2014,
    license: 'ofl',
    file: 'google-repo/librebodoni/LibreBodoni[wght].ttf',
    styles: FOUR,
    reach: 'european',
    viet: true,
  },
  {
    name: 'Castoro',
    bucket: 'serif',
    class: 'serif',
    designer: 'John Hudson, Paul Hanslow',
    foundry: 'Tiro Typeworks',
    year: 2020,
    license: 'ofl',
    file: 'google-repo/castoro/Castoro[wght].ttf',
    styles: ['Regular', 'Italic'],
    reach: 'european',
  },
  {
    name: 'Instrument Serif',
    bucket: 'serif',
    class: 'serif',
    designer: 'Rodrigo Fuenzalida, Jordan Egstad',
    foundry: 'Instrument',
    year: 2023,
    license: 'ofl',
    file: 'google-repo/instrumentserif/InstrumentSerif-Italic.ttf',
    styles: ['Regular', 'Italic'],
    reach: 'western',
  },
  {
    name: 'Neuton',
    bucket: 'serif',
    class: 'serif',
    designer: 'Brian Zick',
    foundry: 'Brian Zick',
    year: 2010,
    license: 'ofl',
    file: 'google-repo/neuton/Neuton-Light.ttf',
    styles: ['ExtraLight', 'Light', 'Regular', 'Italic', 'Bold', 'ExtraBold'],
    reach: 'european',
  },
  {
    name: 'Rufina',
    bucket: 'serif',
    class: 'serif',
    designer: 'Martin Sommaruga',
    foundry: 'Martin Sommaruga',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/rufina/Rufina-Regular.ttf',
    styles: ['Regular', 'Bold'],
    reach: 'european',
  },
  {
    name: 'Asul',
    bucket: 'serif',
    class: 'serif',
    designer: 'Mariela Monsalve',
    foundry: 'Mariela Monsalve',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/asul/Asul-Bold.ttf',
    styles: ['Regular', 'Bold'],
    reach: 'western',
  },
  {
    name: 'Petrona',
    bucket: 'serif',
    class: 'serif',
    designer: 'Ringo R. Seeber',
    foundry: 'Ringo R. Seeber',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/petrona/Petrona[wght].ttf',
    styles: SIX,
    reach: 'european',
    viet: true,
  },
  {
    name: 'Manuale',
    bucket: 'serif',
    class: 'serif',
    designer: 'Pablo Impallari, Rodrigo Fuenzalida',
    foundry: 'Impallari Type',
    year: 2017,
    license: 'ofl',
    file: 'google-repo/manuale/Manuale-Italic[wght].ttf',
    styles: FOUR,
    reach: 'european',
    viet: true,
  },
  {
    name: 'Grenze',
    bucket: 'serif',
    class: 'serif',
    designer: 'Renata Polastri',
    foundry: 'Omnibus-Type',
    year: 2018,
    license: 'ofl',
    file: 'google-repo/grenze/Grenze[wght].ttf',
    styles: ['Light', 'Regular', 'Italic', 'Bold'],
    reach: 'european',
    viet: true,
  },
  {
    name: 'Lisu Bosa',
    bucket: 'serif',
    class: 'serif',
    designer: 'SIL International',
    foundry: 'SIL International',
    year: 2021,
    license: 'ofl',
    file: 'google-repo/lisubosa/LisuBosa-Black.ttf',
    styles: ['Regular', 'Italic', 'Bold', 'Black'],
    reach: 'european',
  },

  // Sans (16).
  {
    name: 'Archivo',
    bucket: 'sans',
    class: 'sans',
    designer: 'Héctor Gatti',
    foundry: 'Omnibus-Type',
    year: 2016,
    license: 'ofl',
    file: 'fontshare/archivo/Archivo-Bold.woff2',
    styles: SIX,
    reach: 'european',
    viet: true,
  },
  {
    name: 'Asap',
    bucket: 'sans',
    class: 'sans',
    designer: 'Pablo Cosgaya',
    foundry: 'Omnibus-Type',
    year: 2011,
    license: 'ofl',
    file: 'fontshare/asap/Asap-SemiBold.otf',
    styles: ['Regular', 'Italic', 'SemiBold', 'Bold'],
    reach: 'european',
    viet: true,
  },
  {
    name: 'Be Vietnam Pro',
    bucket: 'sans',
    class: 'sans',
    designer: 'Lâm Bảo, Tony Le, ViệtAnh Nguyễn',
    foundry: 'Be',
    year: 2021,
    license: 'ofl',
    file: 'fontshare/beVietnam-pro/BeVietnamPro-Bold.otf',
    styles: ['Thin', 'Light', 'Regular', 'SemiBold Italic', 'Bold', 'Black'],
    reach: 'european',
    viet: true,
  },
  {
    name: 'Epilogue',
    bucket: 'sans',
    class: 'sans',
    designer: 'Tyler Finck',
    foundry: 'Etcetera Type Co',
    year: 2020,
    license: 'ofl',
    file: 'google-repo/epilogue/Epilogue[wght].ttf',
    styles: ['Thin', 'Thin Italic', 'Regular', 'Bold'],
    reach: 'european',
    viet: true,
  },
  {
    name: 'Familjen Grotesk',
    bucket: 'sans',
    class: 'sans',
    designer: 'Familjen STHLM',
    foundry: 'Familjen STHLM AB',
    year: 2022,
    license: 'ofl',
    file: 'fontshare/familjen-grotesk/FamiljenGrotesk-BoldItalic.woff',
    styles: FOUR,
    reach: 'european',
  },
  {
    name: 'Montserrat',
    bucket: 'sans',
    class: 'sans',
    designer: 'Julieta Ulanovsky',
    foundry: 'Julieta Ulanovsky',
    year: 2011,
    license: 'ofl',
    file: 'fontshare/montserrat/Montserrat-LightItalic.woff2',
    styles: ['Light', 'Light Italic', 'Regular', 'Medium', 'Bold', 'Black'],
    reach: 'european',
    cyrl: true,
    viet: true,
  },
  {
    name: 'Nunito',
    bucket: 'sans',
    class: 'sans',
    designer: 'Vernon Adams, Jacques Le Bailly',
    foundry: 'Vernon Adams',
    year: 2014,
    license: 'ofl',
    file: 'fontshare/nunito/Nunito-ExtraLightItalic.otf',
    styles: ['ExtraLight Italic', 'Regular', 'Italic', 'Bold', 'Black'],
    reach: 'european',
    cyrl: true,
    viet: true,
  },
  {
    name: 'Outfit',
    bucket: 'sans',
    class: 'sans',
    designer: 'Rodrigo Fuenzalida',
    foundry: 'On Brand Investments',
    year: 2021,
    license: 'ofl',
    file: 'fontshare/outfit/Outfit-Light.woff2',
    styles: ['Thin', 'Light', 'Regular', 'Medium', 'Bold'],
    reach: 'western',
  },
  {
    name: 'Plus Jakarta Sans',
    bucket: 'sans',
    class: 'sans',
    designer: 'Gumpita Rahayu',
    foundry: 'Tokotype',
    year: 2020,
    license: 'ofl',
    file: 'fontshare/plus-jakarta-sans/PlusJakartaSans-Light.otf',
    styles: ['ExtraLight Italic', 'Light', 'Regular', 'Medium', 'Bold'],
    reach: 'european',
    viet: true,
  },
  {
    name: 'Poppins',
    bucket: 'sans',
    class: 'sans',
    designer: 'Ninad Kale, Jonny Pinhorn',
    foundry: 'Indian Type Foundry',
    year: 2014,
    license: 'ofl',
    file: 'fontshare/poppins/Poppins-Medium.woff2',
    styles: ['Light', 'Regular', 'Medium', 'SemiBold', 'Bold', 'Black'],
    reach: 'european',
  },
  {
    name: 'Public Sans',
    bucket: 'sans',
    class: 'sans',
    designer: 'Dan Williams',
    foundry: 'USWDS',
    year: 2019,
    license: 'ofl',
    file: 'fontshare/public-sans/PublicSans-Bold.ttf',
    styles: ['Regular', 'Italic', 'Bold', 'Black'],
    reach: 'european',
    viet: true,
  },
  {
    name: 'Space Grotesk',
    bucket: 'sans',
    class: 'sans',
    designer: 'Florian Karsten',
    foundry: 'Florian Karsten',
    year: 2018,
    license: 'ofl',
    file: 'fontshare/space-grotesk/SpaceGrotesk-SemiBold.woff2',
    styles: ['Light', 'Regular', 'Medium', 'SemiBold', 'Bold'],
    reach: 'european',
    viet: true,
  },
  {
    name: 'Spline Sans',
    bucket: 'sans',
    class: 'sans',
    designer: 'Eben Sorkin, Mirko Velimirović',
    foundry: 'Sorkin Type',
    year: 2021,
    license: 'ofl',
    file: 'fontshare/spline-sans/SplineSans-Light.ttf',
    styles: ['Light', 'Regular', 'Medium', 'Bold'],
    reach: 'european',
  },
  {
    name: 'Ubuntu',
    bucket: 'sans',
    class: 'sans',
    designer: 'Dalton Maag',
    foundry: 'Canonical',
    year: 2010,
    license: 'ufl',
    file: 'google-repo/ubuntu/Ubuntu-Regular.ttf',
    styles: ['Light', 'Regular', 'Italic', 'Medium', 'Bold', 'Bold Italic'],
    reach: 'european',
    cyrl: true,
    grek: true,
  },
  {
    name: 'DM Sans',
    bucket: 'sans',
    class: 'sans',
    designer: 'Colophon Foundry',
    foundry: 'Colophon Foundry',
    year: 2019,
    license: 'ofl',
    file: 'google-static/dmsans/DMSans_36pt-Regular.ttf',
    styles: SIX,
    reach: 'european',
  },
  {
    name: 'Mona Sans',
    bucket: 'sans',
    class: 'sans',
    designer: 'Degarism Studio',
    foundry: 'GitHub',
    year: 2022,
    license: 'ofl',
    file: 'google-static/monasans/MonaSans-BlackItalic.ttf',
    styles: ['Regular', 'Medium', 'Bold', 'Black Italic'],
    reach: 'european',
    viet: true,
  },

  // Slab (6).
  {
    name: 'Roboto Slab',
    bucket: 'slab',
    class: 'slab',
    designer: 'Christian Robertson',
    foundry: 'Google',
    year: 2013,
    license: 'apache',
    file: 'google-static/robotoslab/RobotoSlab-ExtraBold.ttf',
    styles: ['Thin', 'Light', 'Regular', 'Bold', 'ExtraBold', 'Black'],
    reach: 'european',
    cyrl: true,
    grek: true,
    viet: true,
  },
  {
    name: 'Volkhov',
    bucket: 'slab',
    class: 'slab',
    designer: 'Ivan Petrov',
    foundry: 'Cyreal',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/volkhov/Volkhov-Italic.ttf',
    styles: FOUR,
    reach: 'european',
  },
  {
    name: 'Rammetto One',
    bucket: 'slab',
    class: 'slab',
    designer: 'Eben Sorkin',
    foundry: 'Sorkin Type',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/rammettoone/RammettoOne-Regular.ttf',
    styles: ['Regular'],
    reach: 'european',
  },
  {
    name: 'Diplomata',
    bucket: 'slab',
    class: 'slab',
    designer: 'Eduardo Tunni',
    foundry: 'Eduardo Tunni',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/diplomata/Diplomata-Regular.ttf',
    styles: ['Regular'],
    reach: 'western',
  },
  {
    name: 'Kumar One',
    bucket: 'slab',
    class: 'slab',
    designer: 'Erin McLaughlin',
    foundry: 'Erin McLaughlin',
    year: 2017,
    license: 'ofl',
    file: 'google-repo/kumarone/KumarOne-Regular.ttf',
    styles: ['Regular'],
    reach: 'european',
  },
  {
    name: 'Labrada',
    bucket: 'slab',
    class: 'slab',
    designer: 'Mercedes Jáuregui',
    foundry: 'Omnibus-Type',
    year: 2021,
    license: 'ofl',
    file: 'google-repo/labrada/Labrada[wght].ttf',
    styles: ['Light', 'Regular', 'Medium', 'Bold'],
    reach: 'european',
    viet: true,
  },

  // Script and hand (6): several reach basic Latin only, so accents show as tofu.
  {
    name: 'Clicker Script',
    bucket: 'script',
    class: 'script',
    designer: 'Brian J. Bonislawsky',
    foundry: 'Astigmatic',
    year: 2011,
    license: 'ofl',
    file: 'google-repo/clickerscript/ClickerScript-Regular.ttf',
    styles: ['Regular'],
    reach: 'western',
  },
  {
    name: 'Zeyada',
    bucket: 'script',
    class: 'script',
    designer: 'Kimberly Geswein',
    foundry: 'Kimberly Geswein',
    year: 2010,
    license: 'ofl',
    file: 'google-repo/zeyada/Zeyada.ttf',
    styles: ['Regular'],
    reach: 'ascii',
  },
  {
    name: 'Permanent Marker',
    bucket: 'script',
    class: 'script',
    designer: 'Font Diner',
    foundry: 'Font Diner',
    year: 2010,
    license: 'apache',
    file: 'google-repo/permanentmarker/PermanentMarker-Regular.ttf',
    styles: ['Regular'],
    reach: 'ascii',
  },
  {
    name: 'Schoolbell',
    bucket: 'script',
    class: 'script',
    designer: 'Font Diner',
    foundry: 'Font Diner',
    year: 2010,
    license: 'apache',
    file: 'google-repo/schoolbell/Schoolbell-Regular.ttf',
    styles: ['Regular'],
    reach: 'ascii',
  },
  {
    name: 'Rancho',
    bucket: 'script',
    class: 'script',
    designer: 'Sideshow',
    foundry: 'Sideshow',
    year: 2011,
    license: 'apache',
    file: 'google-repo/rancho/Rancho-Regular.ttf',
    styles: ['Regular'],
    reach: 'ascii',
  },
  {
    name: 'Oleo Script Swash Caps',
    bucket: 'script',
    class: 'script',
    designer: 'soytutype',
    foundry: 'soytutype',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/oleoscriptswashcaps/OleoScriptSwashCaps-Bold.ttf',
    styles: ['Regular', 'Bold'],
    reach: 'european',
  },

  // Monospace (6).
  {
    name: 'JetBrains Mono',
    bucket: 'mono',
    class: 'mono',
    designer: 'Philipp Nurullin, Konstantin Bulenkov',
    foundry: 'JetBrains',
    year: 2020,
    license: 'ofl',
    file: 'fontshare/jet-brains-mono/JetBrainsMono-Bold.otf',
    styles: ['Light', 'Regular', 'Italic', 'Bold', 'ExtraBold Italic'],
    reach: 'european',
    cyrl: true,
    grek: true,
    viet: true,
  },
  {
    name: 'Azeret Mono',
    bucket: 'mono',
    class: 'mono',
    designer: 'Martin Vácha',
    foundry: 'Displaay',
    year: 2021,
    license: 'ofl',
    file: 'fontshare/azeret-mono/AzeretMono-ExtraBoldItalic.woff',
    styles: ['Regular', 'Italic', 'Bold', 'ExtraBold Italic'],
    reach: 'european',
  },
  {
    name: 'Inconsolata',
    bucket: 'mono',
    class: 'mono',
    designer: 'Raph Levien',
    foundry: 'Levien',
    year: 2006,
    license: 'ofl',
    file: 'google-repo/inconsolata/Inconsolata-SemiExpandedBold.ttf',
    styles: ['Regular', 'Bold', 'SemiExpanded Bold'],
    reach: 'european',
    cyrl: true,
    viet: true,
  },
  {
    name: 'Ubuntu Mono',
    bucket: 'mono',
    class: 'mono',
    designer: 'Dalton Maag',
    foundry: 'Canonical',
    year: 2011,
    license: 'ufl',
    file: 'google-repo/ubuntumono/UbuntuMono-Bold.ttf',
    styles: FOUR,
    reach: 'european',
    cyrl: true,
    grek: true,
  },
  {
    name: 'Spline Sans Mono',
    bucket: 'mono',
    class: 'mono',
    designer: 'Eben Sorkin',
    foundry: 'Sorkin Type',
    year: 2022,
    license: 'ofl',
    file: 'google-repo/splinesansmono/SplineSansMono-Italic[wght].ttf',
    styles: ['Light', 'Regular', 'Italic', 'Bold'],
    reach: 'european',
  },
  {
    name: 'Atkinson Hyperlegible Mono',
    bucket: 'mono',
    class: 'mono',
    designer: 'Applied Design Works',
    foundry: 'Braille Institute',
    year: 2024,
    license: 'ofl',
    file: 'google-static/atkinsonhyperlegiblemono/AtkinsonHyperlegibleMono-Italic.ttf',
    styles: ['Regular', 'Italic', 'Bold'],
    reach: 'european',
  },

  // Display (8).
  {
    name: 'Bebas Neue',
    bucket: 'display',
    class: 'display',
    designer: 'Ryoichi Tsunekawa',
    foundry: 'Dharma Type',
    year: 2010,
    license: 'ofl',
    file: 'google-repo/bebasneue/BebasNeue-Regular.ttf',
    styles: ['Regular'],
    reach: 'european',
  },
  {
    name: 'Almendra Display',
    bucket: 'display',
    class: 'display',
    designer: 'Ana Sanfelippo',
    foundry: 'Ana Sanfelippo',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/almendradisplay/AlmendraDisplay-Regular.ttf',
    styles: ['Regular'],
    reach: 'western',
  },
  {
    name: 'Italiana',
    bucket: 'display',
    class: 'display',
    designer: 'Santiago Orozco',
    foundry: 'Typemade',
    year: 2011,
    license: 'ofl',
    file: 'google-repo/italiana/Italiana-Regular.ttf',
    styles: ['Regular'],
    reach: 'western',
  },
  {
    name: 'Marcellus SC',
    bucket: 'display',
    class: 'display',
    designer: 'Brian J. Bonislawsky',
    foundry: 'Astigmatic',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/marcellussc/MarcellusSC-Regular.ttf',
    styles: ['Regular'],
    reach: 'european',
  },
  {
    name: 'Nosifer',
    bucket: 'display',
    class: 'display',
    designer: 'Typomondo',
    foundry: 'Typomondo',
    year: 2011,
    license: 'ofl',
    file: 'google-repo/nosifer/Nosifer-Regular.ttf',
    styles: ['Regular'],
    reach: 'ascii',
  },
  {
    name: 'Press Start 2P',
    bucket: 'display',
    class: 'display',
    designer: 'CodeMan38',
    foundry: 'CodeMan38',
    year: 2011,
    license: 'ofl',
    file: 'google-repo/pressstart2p/PressStart2P-Regular.ttf',
    styles: ['Regular'],
    reach: 'european',
    cyrl: true,
    grek: true,
  },
  {
    name: 'Emblema One',
    bucket: 'display',
    class: 'display',
    designer: 'Riccardo De Franceschi',
    foundry: 'Riccardo De Franceschi',
    year: 2012,
    license: 'ofl',
    file: 'google-repo/emblemaone/EmblemaOne-Regular.ttf',
    styles: ['Regular'],
    reach: 'ascii',
  },
  {
    name: 'Cherry Cream Soda',
    bucket: 'display',
    class: 'display',
    designer: 'Font Diner',
    foundry: 'Font Diner',
    year: 2010,
    license: 'apache',
    file: 'google-repo/cherrycreamsoda/CherryCreamSoda-Regular.ttf',
    styles: ['Regular'],
    reach: 'ascii',
  },
];

const BUCKETS: WallBucket[] = ['variable', 'serif', 'sans', 'slab', 'script', 'mono', 'display'];

/**
 * The 60 families in list order: one of each bucket in turn, so the first screen
 * mixes classes like a real library (row 1 Asap Sharp, whose Regular is face 1 for
 * `?face=1`; row 2 Crimson Pro, six styles; then sans, slab, script, mono, display).
 */
export const WALL_FAMILIES: WallFamily[] = (() => {
  const queues = BUCKETS.map((b) => TABLE.filter((f) => f.bucket === b));
  const out: WallFamily[] = [];
  while (queues.some((q) => q.length))
    for (const q of queues) {
      const next = q.shift();
      if (next) out.push(next);
    }
  return out;
})();

/** Family ids of the wall: `WALL_FAMILY_BASE + index`, apart from every other mock id. */
export const WALL_FAMILY_BASE = 500;

/** Face ids the opt-in mock faces use (`?mockColor=1`, `?mockHebrew=1`): the wall skips them. */
const RESERVED_FACE_IDS = new Set<FaceId>([8, 9]);

const WEIGHTS: [string, number][] = [
  ['Thin', 100],
  ['ExtraLight', 200],
  ['Light', 300],
  ['Regular', 400],
  ['Medium', 500],
  ['SemiBold', 600],
  ['Bold', 700],
  ['ExtraBold', 800],
  ['Black', 900],
];

/** Weight, slant and width class of a style name ("SemiBold Italic", "SemiExpanded Bold"). */
function styleOf(style: string): { weight: number; slant: 'upright' | 'italic'; width: number } {
  const words = style.split(' ');
  const italic = words.includes('Italic');
  const width = words.includes('SemiExpanded') ? 6 : 5;
  const w = words.find((x) => WEIGHTS.some(([n]) => n === x));
  const weight = WEIGHTS.find(([n]) => n === w)?.[1] ?? 400;
  return { weight, slant: italic ? 'italic' : 'upright', width };
}

const range = (from: number, to: number) =>
  Array.from({ length: to - from }, (_, i) => String.fromCodePoint(from + i)).join('');
const ASCII = range(0x20, 0x7f);
const LATIN_1 = range(0xa0, 0x100);
const LATIN_EXT_A = range(0x100, 0x180);
/** Russian, Ukrainian and Serbian letters. */
const CYRILLIC = range(0x410, 0x450) + 'ЁёЂђЈјЉљЊњЋћЏџЄєІіЇїҐґ';
/** Monotonic Greek: the alphabet with its tonos forms. */
const GREEK = range(0x391, 0x3a2) + range(0x3a3, 0x3aa) + range(0x3ac, 0x3cf) + 'Ά';
/** Vietnamese: ơ ư and the stacked tone marks of Latin Extended Additional. */
const VIETNAMESE = 'ƠơƯư' + range(0x1ea0, 0x1efa);

/** The characters a wall family maps. */
function charsOf(f: WallFamily): string {
  let s = ASCII;
  if (f.reach !== 'ascii') s += LATIN_1;
  if (f.reach === 'european') s += LATIN_EXT_A;
  if (f.viet) s += VIETNAMESE;
  if (f.cyrl) s += CYRILLIC;
  if (f.grek) s += GREEK;
  return s;
}

const LICENSES: Record<WallFamily['license'], LicenseInfo> = {
  ofl: {
    description: 'SIL Open Font License 1.1',
    url: 'https://openfontlicense.org',
    openLicense: 'ofl',
    embedding: 'installable',
    declared: null,
  },
  apache: {
    description: 'Licensed under the Apache License, Version 2.0',
    url: 'https://www.apache.org/licenses/LICENSE-2.0',
    openLicense: 'apache',
    embedding: 'installable',
    declared: null,
  },
  ufl: {
    description: 'Ubuntu Font Licence 1.0',
    url: 'https://ubuntu.com/legal/font-licence',
    openLicense: 'ufl',
    embedding: 'installable',
    declared: null,
  },
};

/** Vertical metrics by class, so lines and size factors differ like real fonts. */
const METRICS: Record<Classification, FaceDetail['metrics']> = {
  serif: { unitsPerEm: 1000, ascender: 900, descender: -260, xHeight: 450, capHeight: 670 },
  sans: { unitsPerEm: 1000, ascender: 950, descender: -240, xHeight: 520, capHeight: 700 },
  slab: { unitsPerEm: 1000, ascender: 930, descender: -250, xHeight: 500, capHeight: 700 },
  script: { unitsPerEm: 1000, ascender: 980, descender: -320, xHeight: 380, capHeight: 650 },
  mono: { unitsPerEm: 1000, ascender: 1020, descender: -300, xHeight: 540, capHeight: 730 },
  display: { unitsPerEm: 1000, ascender: 900, descender: -200, xHeight: 560, capHeight: 720 },
  symbol: { unitsPerEm: 1000, ascender: 900, descender: -200, xHeight: null, capHeight: null },
};

/** The OpenType features each class usually ships. */
const FEATURE_TAGS: Record<Classification, string[]> = {
  serif: ['kern', 'liga', 'onum', 'smcp'],
  sans: ['ccmp', 'kern', 'liga', 'ss01'],
  slab: ['kern', 'liga'],
  script: ['kern', 'liga'],
  mono: ['kern'],
  display: ['kern'],
  symbol: [],
};

const FORMATS: Record<string, FontFormat> = {
  ttf: 'ttf',
  otf: 'otf',
  woff: 'woff',
  woff2: 'woff2',
};

const compact = (s: string) => s.replace(/[^A-Za-z0-9]/g, '');

/** A plausible, stable file size for a face of family `i`. */
const sizeOf = (f: WallFamily, i: number, k: number) =>
  Math.round(
    (f.cyrl || f.grek ? 180_000 : f.viet ? 120_000 : 60_000) + ((i * 7919 + k * 104_729) % 90_000),
  );

/** Everything browser mode needs about the wall. */
export interface WallLibrary {
  faces: FaceDetail[];
  /** Face id → fixture path under fixtures/fonts/. */
  fixtures: Map<FaceId, string>;
  /** Family id → the characters it maps. */
  chars: Map<FamilyId, Set<string>>;
  /** Family id → its class. */
  classes: Map<FamilyId, Classification>;
}

/**
 * Builds the wall's faces; `features` names the feature tags as the mock does. Face ids
 * run from 1 (skipping the opt-in faces' 8 and 9) in table order, styles sorted by
 * weight then slant.
 */
export function wallLibrary(
  features: (...tags: string[]) => FeatureInfo[] = (...tags) =>
    tags.map((tag) => ({ tag, name: null, automatic: false, defaultOn: false })),
): WallLibrary {
  const faces: FaceDetail[] = [];
  const fixtures = new Map<FaceId, string>();
  const chars = new Map<FamilyId, Set<string>>();
  const classes = new Map<FamilyId, Classification>();
  let next = 1;
  const nextId = () => {
    while (RESERVED_FACE_IDS.has(next)) next++;
    return next++;
  };
  WALL_FAMILIES.forEach((f, i) => {
    const family = WALL_FAMILY_BASE + i;
    const set = charsOf(f);
    chars.set(family, new Set(set));
    classes.set(family, f.class);
    const ext = f.file.split('.').pop()!;
    const variable = !!f.axes?.length;
    const scripts = ['latn', ...(f.cyrl ? ['cyrl'] : []), ...(f.grek ? ['grek'] : [])];
    const rank = (s: string) => {
      const x = styleOf(s);
      return x.weight * 10 + (x.slant === 'italic' ? 1 : 0) + x.width * 0.1;
    };
    [...f.styles]
      .sort((a, b) => rank(a) - rank(b))
      .forEach((style, k) => {
        const id = nextId();
        const { weight, slant, width } = styleOf(style);
        const ps = `${compact(f.name)}-${compact(style)}`;
        const wght = f.axes?.find((a) => a.tag === 'wght');
        const axisNote = variable ? `[${f.axes!.map((a) => a.tag).join(',')}]` : '';
        const fileName = variable
          ? `${compact(f.name)}${slant === 'italic' ? '-Italic' : ''}${axisNote}.${ext}`
          : `${ps}.${ext}`;
        fixtures.set(id, f.file);
        faces.push({
          face: {
            id,
            family,
            familyName: f.name,
            style,
            postscriptName: ps,
            weight: wght?.default ?? weight,
            width,
            slant,
            format: FORMATS[ext] ?? 'ttf',
            isVariable: variable,
            active: null,
            health: { kind: 'ok' },
          },
          path: `/Users/demo/Fonts/${f.foundry}/${f.name}/${fileName}`,
          collectionIndex: 0,
          fileSize: sizeOf(f, i, k),
          hash: (id * 2654435761).toString(16).padStart(64, 'a').slice(-64),
          version: `Version ${1 + (i % 3)}.${String((i * 37) % 1000).padStart(3, '0')}`,
          copyright: `Copyright ${f.year} The ${f.name} Project Authors`,
          designer: f.designer,
          vendorId: null,
          license: structuredClone(LICENSES[f.license]),
          metrics: { ...METRICS[f.class] },
          axes: structuredClone(f.axes ?? []),
          instances: wght
            ? WEIGHTS.filter(([, v]) => v >= wght.min! && v <= wght.max!).map(([name, value]) => ({
                name,
                coordinates: [{ tag: 'wght', value }],
              }))
            : [],
          features: features(...FEATURE_TAGS[f.class]),
          scripts,
          tags: [],
          favorite: false,
          rating: null,
          notes: null,
          legacyFamily: f.name,
          legacyStyle: style,
          foundry: f.foundry,
          italicAngle: slant === 'italic' ? -12 : 0,
          codePointCount: set.length,
        });
      });
  });
  return { faces, fixtures, chars, classes };
}

/** Face id → fixture path, for the @screens spec that serves `/fmfont/<id>`. */
export function wallFixtures(): Map<FaceId, string> {
  return wallLibrary().fixtures;
}

/** The first face of the wall family named `name` (tests, duplicates). */
function faceOf(lib: WallLibrary, name: string, style = 'Regular'): FaceDetail {
  const f = lib.faces.find((d) => d.face.familyName === name && d.face.style === style);
  if (!f) throw new Error(`The wall has no ${name} ${style}.`);
  return f;
}

/**
 * The wall's duplicates: two exact copies (Montserrat in Downloads, Poppins on the
 * Desktop) and an older JetBrains Mono that shares the PostScript name (near).
 */
export function wallDuplicates(lib: WallLibrary): DuplicateGroup[] {
  const exact = (d: FaceDetail, other: string): DuplicateGroup => ({
    kind: 'exact',
    postscriptName: d.face.postscriptName,
    copies: [
      {
        path: d.path,
        size: d.fileSize,
        hash: d.hash,
        face: d.face.id,
        version: d.version,
        source: 1,
        inLibraryFolder: true,
        keep: true,
      },
      {
        path: other,
        size: d.fileSize,
        hash: d.hash,
        face: d.face.id,
        version: d.version,
        source: 1,
        inLibraryFolder: false,
        keep: false,
      },
    ],
  });
  const mono = faceOf(lib, 'JetBrains Mono');
  const near = exact(mono, '/Users/demo/Downloads/old/JetBrainsMono-Regular.otf');
  near.kind = 'near';
  near.copies[1]!.hash = 'b7'.repeat(32);
  near.copies[1]!.size = mono.fileSize - 4_212;
  return [
    exact(faceOf(lib, 'Montserrat'), '/Users/demo/Downloads/Montserrat-Regular.woff2'),
    exact(faceOf(lib, 'Poppins'), '/Users/demo/Desktop/Poppins-Regular.woff2'),
    near,
  ];
}

/**
 * The wall's Problems entries beyond its faces: the three duplicates and two broken
 * files (a truncated Agdasima, as `fixtures/fonts/derived/agdasima/agdasima-truncated.ttf`
 * is, and a Teko without a `head` table).
 */
export function wallProblems(lib: WallLibrary): Problem[] {
  const dupes = wallDuplicates(lib).map((g): Problem => ({
    kind: g.kind === 'exact' ? 'duplicateFile' : 'duplicateFace',
    face: g.copies[1]!.face,
    path: g.copies[1]!.path,
    other: g.copies[0]!.path,
    message:
      g.kind === 'exact'
        ? `The same file is also at ${g.copies[0]!.path}.`
        : `The same font (${g.postscriptName}) is also at ${g.copies[0]!.path}.`,
  }));
  return [
    {
      kind: 'broken',
      face: null,
      path: '/Users/demo/Fonts/Agdasima/Agdasima-Bold.ttf',
      other: null,
      message: "The file is truncated: the 'glyf' table ends before its declared length.",
    },
    {
      kind: 'broken',
      face: null,
      path: '/Users/demo/Downloads/Teko-SemiBold.ttf',
      other: null,
      message: "The file has no 'head' table.",
    },
    ...dupes,
  ];
}

/** The Vietnamese preset the wall adds to the mock's languages, so its gaps can be seen. */
export const WALL_VIETNAMESE: LanguagePreset = {
  code: 'vi',
  name: 'Vietnamese',
  script: 'Latn',
  pangram: 'Tôi có thể ăn thủy tinh mà không hại gì',
};
