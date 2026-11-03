/** 16 px, 1.5 stroke line icons (Lucide shapes). Each entry is the inner SVG markup. */
export const ICONS = {
  list: '<path d="M3 4h10M3 8h10M3 12h10"/>',
  table: '<rect x="2.5" y="3" width="11" height="10" rx="1.5"/><path d="M2.5 6.5h11M6.5 6.5V13"/>',
  grid: '<rect x="2.5" y="2.5" width="4.5" height="4.5" rx="1"/><rect x="9" y="2.5" width="4.5" height="4.5" rx="1"/><rect x="2.5" y="9" width="4.5" height="4.5" rx="1"/><rect x="9" y="9" width="4.5" height="4.5" rx="1"/>',
  waterfall: '<path d="M3 3.5h10M3 6.5h8M3 9.5h6M3 12.5h4"/>',
  layout: '<rect x="2.5" y="2.5" width="11" height="11" rx="1.5"/><path d="M2.5 7h11M7 7v6.5"/>',
  density: '<path d="M3 3.5h10M3 6h10M3 8.5h10M3 11h10M3 13.5h10"/>',
  compare:
    '<rect x="2" y="3" width="5" height="10" rx="1"/><rect x="9" y="3" width="5" height="10" rx="1"/>',
  inspector: '<rect x="2.5" y="3" width="11" height="10" rx="1.5"/><path d="M10 3v10"/>',
} as const;
export type IconName = keyof typeof ICONS;
