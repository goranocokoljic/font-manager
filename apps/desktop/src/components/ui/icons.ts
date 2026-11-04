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
  sidebar: '<rect x="2.5" y="3" width="11" height="10" rx="1.5"/><path d="M6 3v10"/>',
  inspector: '<rect x="2.5" y="3" width="11" height="10" rx="1.5"/><path d="M10 3v10"/>',
  close: '<path d="M4 4l8 8M12 4l-8 8"/>',
  plus: '<path d="M8 3v10M3 8h10"/>',
  minus: '<path d="M3 8h10"/>',
  sparkles:
    '<path d="M7 2.5l1.3 3.2L11.5 7 8.3 8.3 7 11.5 5.7 8.3 2.5 7l3.2-1.3z"/><path d="M12.5 10.5v3M11 12h3"/>',
  rescan: '<path d="M13 8a5 5 0 1 1-1.5-3.5M13 2.5v2.5h-2.5"/>',
  pause: '<path d="M5.5 3.5v9M10.5 3.5v9"/>',
  play: '<path d="M5 3.5l7 4.5-7 4.5z"/>',
  trash: '<path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.5 8.5h6l.5-8.5"/>',
  folder:
    '<path d="M2.5 4.5v7a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-5a1 1 0 0 0-1-1H8L6.8 3.5H3.5a1 1 0 0 0-1 1z"/>',
  layers: '<path d="M8 2.5l5.5 3L8 8.5l-5.5-3zM2.5 8L8 11l5.5-3M2.5 10.5L8 13.5l5.5-3"/>',
  tag: '<path d="M2.5 8.2V3.5a1 1 0 0 1 1-1h4.7l5.3 5.3a1 1 0 0 1 0 1.4l-4.3 4.3a1 1 0 0 1-1.4 0z"/><path d="M5.5 5.5h.01" stroke-width="2"/>',
  alert: '<path d="M8 2.8l5.5 9.4h-11zM8 6.8v2.4M8 11h.01"/>',
  box: '<path d="M2.5 5l5.5-2.5L13.5 5v6L8 13.5 2.5 11zM2.5 5L8 7.5 13.5 5M8 7.5v6"/>',
  globe:
    '<circle cx="8" cy="8" r="5.5"/><path d="M2.5 8h11M8 2.5c-2 2-2 9 0 11M8 2.5c2 2 2 9 0 11"/>',
  chevron: '<path d="M6 3.5l4.5 4.5L6 12.5"/>',
  chevronBack: '<path d="M10 3.5L5.5 8l4.5 4.5"/>',
  arrowLeft: '<path d="M13 8H3M7 4L3 8l4 4"/>',
  arrowRight: '<path d="M3 8h10M9 4l4 4-4 4"/>',
  folderPlus:
    '<path d="M2.5 4.5v7a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-5a1 1 0 0 0-1-1H8L6.8 3.5H3.5a1 1 0 0 0-1 1zM8 7.5v3M6.5 9h3"/>',
  more: '<path d="M3.5 8h.01M8 8h.01M12.5 8h.01" stroke-width="2"/>',
  power: '<path d="M8 2.5v5M4.7 4.6a5 5 0 1 0 6.6 0"/>',
  // A lens with "≈" inside: "find what looks like this", not zoom (a "+" read as zoom in).
  similar:
    '<circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5M5 6.2q1-.9 2 0t2 0M5 8.2q1-.9 2 0t2 0"/>',
  import: '<path d="M8 2.5v7M5 6.5l3 3 3-3M3 12.5h10"/>',
  export: '<path d="M8 9.5v-7M5 5.5l3-3 3 3M3 12.5h10"/>',
  star: '<path d="M8 2.2l1.7 3.5 3.8.5-2.8 2.7.7 3.8L8 10.8l-3.4 1.9.7-3.8-2.8-2.7 3.8-.5z"/>',
  starFilled:
    '<path fill="currentColor" d="M8 2.2l1.7 3.5 3.8.5-2.8 2.7.7 3.8L8 10.8l-3.4 1.9.7-3.8-2.8-2.7 3.8-.5z"/>',
  heart:
    '<path d="M8 13.3S2.5 10 2.5 6.1A2.9 2.9 0 0 1 8 5a2.9 2.9 0 0 1 5.5 1.1C13.5 10 8 13.3 8 13.3z"/>',
  heartFilled:
    '<path fill="currentColor" d="M8 13.3S2.5 10 2.5 6.1A2.9 2.9 0 0 1 8 5a2.9 2.9 0 0 1 5.5 1.1C13.5 10 8 13.3 8 13.3z"/>',
  sun: '<circle cx="8" cy="8" r="2.8"/><path d="M8 2v1.5M8 12.5V14M2 8h1.5M12.5 8H14M3.8 3.8l1 1M11.2 11.2l1 1M12.2 3.8l-1 1M4.8 11.2l-1 1"/>',
  sliders:
    '<path d="M3 5h5.5M11.5 5H13M3 11h1.5M7.5 11H13"/><circle cx="10" cy="5" r="1.5"/><circle cx="6" cy="11" r="1.5"/>',
  code: '<path d="M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5"/>',
  print:
    '<path d="M4.5 6V2.5h7V6M4.5 11.5h-2v-5h11v5h-2"/><rect x="4.5" y="9.5" width="7" height="4"/>',
  width: '<path d="M2.5 3v10M13.5 3v10M5.5 8h5M7.5 6L5.5 8l2 2M8.5 6l2 2-2 2"/>',
  info: '<circle cx="8" cy="8" r="5.5"/><path d="M8 7.5v3.2M8 5.4h.01"/>',
} as const;
export type IconName = keyof typeof ICONS;
