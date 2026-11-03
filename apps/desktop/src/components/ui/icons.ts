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
  plus: '<path d="M8 3v10M3 8h10"/>',
  sparkles:
    '<path d="M7 2.5l1.3 3.2L11.5 7 8.3 8.3 7 11.5 5.7 8.3 2.5 7l3.2-1.3z"/><path d="M12.5 10.5v3M11 12h3"/>',
  rescan: '<path d="M13 8a5 5 0 1 1-1.5-3.5M13 2.5v2.5h-2.5"/>',
  pause: '<path d="M5.5 3.5v9M10.5 3.5v9"/>',
  play: '<path d="M5 3.5l7 4.5-7 4.5z"/>',
  trash: '<path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.5 8.5h6l.5-8.5"/>',
  chevron: '<path d="M6 3.5l4.5 4.5L6 12.5"/>',
  more: '<path d="M3.5 8h.01M8 8h.01M12.5 8h.01" stroke-width="2"/>',
  power: '<path d="M8 2.5v5M4.7 4.6a5 5 0 1 0 6.6 0"/>',
  similar: '<circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5M5.5 7h3M7 5.5v3"/>',
  import: '<path d="M8 2.5v7M5 6.5l3 3 3-3M3 12.5h10"/>',
} as const;
export type IconName = keyof typeof ICONS;
