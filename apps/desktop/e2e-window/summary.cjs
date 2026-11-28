const R = require(process.argv[2] ?? './results/windows-2026-11-28.json');
for (const r of R) {
  console.log('==', r.run, JSON.stringify(r.setup));
  const t = r.tooltip, p = r.popover, m = r.textFieldMenu, d = r.dragGhost;
  console.log(' tooltip gap', t?.gap?.toFixed(1), 'overlapH', t?.overlapH?.toFixed(1), 'btn', JSON.stringify([t?.button.left, t?.button.top, t?.button.width]), 'tip', JSON.stringify([t?.tip.left, t?.tip.top, t?.tip.width]));
  console.log(' popover below', p?.below?.toFixed(1), 'nearH', p?.nearH?.toFixed(1), 'btn', JSON.stringify([p?.button.left, p?.button.bottom]), 'menu', JSON.stringify([p?.menu.left, p?.menu.top]));
  console.log(' tfmenu dx', m?.dx?.toFixed(1), 'dy', m?.dy?.toFixed(1), JSON.stringify(m?.pointer), JSON.stringify([m?.menu.left, m?.menu.top]));
  console.log(' drag', JSON.stringify(d?.ghosts?.map((g) => g && g.offset)), 'spread', d?.spread);
  console.log(' list', r.list?.ok, JSON.stringify(r.list?.steps.filter((s) => s.worst > 1 || (!s.covered && !s.atEnd)).slice(0, 3)), 'last', JSON.stringify(r.list?.steps.at(-1)));
  console.log(' paper', JSON.stringify(r.pdf?.paper));
}
