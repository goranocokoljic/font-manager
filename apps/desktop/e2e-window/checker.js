// E58.4 TEST BUILD ONLY (never merged): runs inside the real app window (FM_TEST_CHECKS).
// Input goes through WebView2's DevTools protocol (Input.dispatchMouseEvent and
// Input.dispatchDragEvent via the test_cdp command), so it takes the browser's input path,
// with the zoom, as a real pointer does. Where the page saw the pointer is read from its own
// mousemove events. The report goes to test_report, which quits the app.
(async () => {
  const invoke = window.__TAURI_INTERNALS__.invoke;
  const inv = (cmd, req) => invoke(cmd, req === undefined ? {} : { req });
  const cdp = async (method, params = {}) => JSON.parse(await invoke('test_cdp', { method, params: JSON.stringify(params) }));
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const rect = (el) => {
    const r = el.getBoundingClientRect();
    return { left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height };
  };
  async function waitFor(fn, what, ms = 90000) {
    const end = Date.now() + ms;
    for (;;) {
      try {
        const v = await fn();
        if (v) return v;
      } catch {
        /* not yet */
      }
      if (Date.now() > end) throw new Error(`timed out: ${what}`);
      await sleep(200);
    }
  }
  let seen = null;
  window.addEventListener('mousemove', (e) => (seen = { x: e.clientX, y: e.clientY }), true);
  const move = async (x, y) => {
    await cdp('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
    await sleep(60);
    return seen;
  };
  const press = (type, x, y, button = 'left') =>
    cdp('Input.dispatchMouseEvent', { type, x, y, button, buttons: type === 'mousePressed' ? (button === 'left' ? 1 : 2) : 0, clickCount: 1 });
  const key = async (k) => {
    await cdp('Input.dispatchKeyEvent', { type: 'keyDown', key: k, code: k, windowsVirtualKeyCode: k === 'Escape' ? 27 : 0 });
    await cdp('Input.dispatchKeyEvent', { type: 'keyUp', key: k, code: k, windowsVirtualKeyCode: k === 'Escape' ? 27 : 0 });
  };
  // The point to send for an element's centre: found by probing, so the check does not assume
  // how the protocol's coordinates relate to the page's (that relation is itself reported).
  const out = { mode: window.__fmTestInterface, errors: [] };
  const step = async (name, fn) => {
    try {
      out[name] = await fn();
    } catch (e) {
      out.errors.push(`${name}: ${e?.stack ?? e}`);
    }
  };

  await step('setup', async () => {
    const fixtures = window.__fmFixtures;
    const source = await inv('add_source', { path: fixtures, mode: { kind: 'indexInPlace' } });
    await inv('scan', { source: source.id }).catch(() => inv('scan', {}));
    await waitFor(async () => (await inv('list_families', { page: { offset: 0, limit: 1 } })).total > 50, 'the scan');
    const status = await inv('setup_status');
    for (const s of status.open) if (s !== 'downloadFolder') await inv('answer_setup_step', { step: s, answer: 'skipped' }).catch(() => {});
    await inv('create_collection', { name: 'Probe' });
    for (let i = 0; i < 10 && document.querySelector('[data-testid=setup]'); i++) {
      const next = document.querySelector('[data-testid=setup-continue]') ?? [...document.querySelectorAll('[data-testid=setup] button')].at(-1);
      next?.click();
      await sleep(800);
    }
    out.dom0 = { hash: location.hash, text: document.body.innerText.slice(0, 600), roles: [...new Set([...document.querySelectorAll('[role]')].map((e) => e.getAttribute('role')))], more: document.querySelectorAll('button.more').length, rows: document.querySelectorAll('[role=treeitem]').length, testids: [...new Set([...document.querySelectorAll('[data-testid]')].map((e) => e.dataset.testid))].slice(0, 60) };
    await waitFor(() => document.querySelector('[role=treeitem][data-key]') && document.querySelector('button.more'), 'the list and a collection', 20000);
    await sleep(1500);
    return { dpr: devicePixelRatio, innerWidth, innerHeight };
  });

  // How the protocol's x, y map to the page's clientX, clientY.
  await step('input', async () => {
    const a = await move(100, 100);
    const b = await move(300, 200);
    return { sent: [[100, 100], [300, 200]], seen: [a, b], scale: (b.x - a.x) / 200 };
  });
  const scale = out.input?.scale ?? 1;
  const send = (x, y) => [x / scale, y / scale];

  await step('tooltip', async () => {
    const button = document.querySelector('button.more');
    const b = rect(button);
    await move(...send(b.left + b.width / 2, b.top + b.height / 2));
    const tipEl = await waitFor(() => [...document.querySelectorAll('[role=tooltip]')].find((t) => t.getBoundingClientRect().width > 0), 'the tooltip', 5000);
    const t = rect(tipEl);
    const gap = Math.min(Math.abs(t.top - b.bottom), Math.abs(b.top - t.bottom));
    const overlapH = Math.min(t.right, b.right) - Math.max(t.left, b.left);
    await move(5, 5);
    await sleep(400);
    return { button: b, tip: t, gap, overlapH, ok: gap <= 16 && overlapH > 0 };
  });

  await step('popover', async () => {
    const button = document.querySelector('button.more');
    const b = rect(button);
    const [x, y] = send(b.left + b.width / 2, b.top + b.height / 2);
    await press('mousePressed', x, y);
    await press('mouseReleased', x, y);
    const menuEl = await waitFor(() => document.querySelector('[role=menu]'), 'the popover', 5000);
    await sleep(300);
    const m = rect(menuEl);
    const below = m.top - b.bottom;
    const above = b.top - m.bottom;
    const nearH = Math.min(Math.abs(m.left - b.left), Math.abs(m.right - b.right));
    await key('Escape');
    await sleep(300);
    return { button: b, menu: m, below, above, nearH, ok: (Math.abs(below) <= 12 || Math.abs(above) <= 12) && nearH <= 12 };
  });

  await step('textFieldMenu', async () => {
    const field = [...document.querySelectorAll('input[type=search], input[type=text], input:not([type])')].find((f) => f.getBoundingClientRect().width > 40);
    const f = rect(field);
    const [x, y] = send(f.left + 20, f.top + f.height / 2);
    const at = await move(x, y);
    await press('mousePressed', x, y, 'right');
    await press('mouseReleased', x, y, 'right');
    const menuEl = await waitFor(() => document.querySelector('[data-testid=text-field-menu]'), 'the text field menu', 5000);
    await sleep(200);
    const m = rect(menuEl);
    const dx = m.left - at.x;
    const dy = m.top - at.y;
    await key('Escape');
    await sleep(200);
    return { pointer: at, menu: m, dx, dy, ok: Math.abs(dx) <= 8 && Math.abs(dy) <= 8 };
  });

  await step('list', async () => {
    let sc = document.querySelector('[role=treeitem][data-key]');
    while (sc && !(sc.scrollHeight > sc.clientHeight + 4 && /auto|scroll/.test(getComputedStyle(sc).overflowY))) sc = sc.parentElement;
    const box = rect(sc);
    const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
    const cover = hit && !sc.contains(hit) ? hit.closest('[data-testid],[class]') : null;
    // A point the list owns: the first one, top to bottom, whose hit is inside it.
    let px = box.left + box.width / 2;
    let py = box.top + 10;
    for (let yy = box.top + 10; yy < box.bottom - 5; yy += 10) {
      const h = document.elementFromPoint(px, yy);
      if (h && sc.contains(h)) {
        py = yy;
        break;
      }
    }
    out.listProbe = { box, cover: cover && { testid: cover.dataset?.testid, cls: String(cover.className), text: cover.textContent.trim().slice(0, 80), rect: rect(cover) }, wheelAt: [px, py] };
    const [x, y] = send(px, py);
    await move(x, y);
    const steps = [];
    for (let i = 0; i < 12; i++) {
      await cdp('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX: 0, deltaY: 173 });
      await sleep(180);
      const rows = [...sc.querySelectorAll('[role=treeitem][data-key]')]
        .map((r) => r.getBoundingClientRect())
        .filter((r) => r.bottom > box.top && r.top < box.bottom)
        .sort((a, b) => a.top - b.top);
      let worst = 0;
      for (let k = 1; k < rows.length; k++) worst = Math.max(worst, Math.abs(rows[k].top - rows[k - 1].bottom));
      const covered = rows.length > 0 && rows[0].top <= box.top + 1 && rows.at(-1).bottom >= box.bottom - 1;
      steps.push({ scrollTop: sc.scrollTop, rows: rows.length, worst, covered, atEnd: sc.scrollTop + sc.clientHeight >= sc.scrollHeight - 1 });
    }
    sc.scrollTop = 0;
    await sleep(300);
    return { steps, moved: steps.at(-1).scrollTop > 0, ok: steps.at(-1).scrollTop > 0 && steps.every((s) => s.worst <= 1 && (s.covered || s.atEnd)) };
  });

  await step('dragGhost', async () => {
    const row = document.querySelectorAll('[role=treeitem][data-key]')[2];
    const r = rect(row);
    const start = { x: r.left + 60, y: r.top + r.height / 2 };
    await cdp('Input.setInterceptDrags', { enabled: true });
    const [sx, sy] = send(start.x, start.y);
    await move(sx, sy);
    await press('mousePressed', sx, sy);
    for (let i = 1; i <= 4; i++) await cdp('Input.dispatchMouseEvent', { type: 'mouseMoved', x: sx + i * 3, y: sy + i * 2, button: 'left', buttons: 1 });
    await sleep(200);
    const data = { items: [{ mimeType: 'text/plain', data: 'probe' }], dragOperationsMask: 1 };
    const ghosts = [];
    let first = true;
    for (const [mx, my] of [
      [40, 30],
      [120, -40],
      [220, -90],
    ]) {
      const [x, y] = send(start.x + mx, start.y + my);
      if (first) await cdp('Input.dispatchDragEvent', { type: 'dragEnter', x, y, data });
      first = false;
      await cdp('Input.dispatchDragEvent', { type: 'dragOver', x, y, data });
      await sleep(200);
      const g = document.querySelector('[data-testid=drag-ghost]');
      const gr = g ? rect(g) : null;
      ghosts.push(gr ? { pointer: [start.x + mx, start.y + my], ghost: [gr.left, gr.top], offset: [gr.left - (start.x + mx), gr.top - (start.y + my)] } : null);
    }
    await cdp('Input.dispatchDragEvent', { type: 'dragCancel', x: sx, y: sy, data });
    await press('mouseReleased', sx, sy);
    await cdp('Input.setInterceptDrags', { enabled: false });
    await key('Escape');
    const offs = ghosts.filter(Boolean).map((g) => g.offset);
    const spread = offs.length ? Math.max(...offs.map((o) => Math.hypot(o[0] - offs[0][0], o[1] - offs[0][1]))) : null;
    const grab = offs[0] ?? null;
    return {
      ghosts,
      spread,
      ok: offs.length === 3 && spread <= 2 && Math.abs(grab[0] + 60) <= 4 && grab[1] <= 0 && grab[1] >= -r.height,
    };
  });

  await step('pdf', async () => {
    location.hash = '#/studio';
    await waitFor(() => document.querySelector('[data-fm-paper]'), 'the Studio paper', 20000).catch(() => null);
    await sleep(2500);
    const paper = document.querySelector('[data-fm-paper]');
    const target = paper?.closest('[class]');
    target?.setAttribute('data-print-target', '');
    const res = await cdp('Page.printToPDF', { printBackground: true, paperWidth: 8.27, paperHeight: 11.69 });
    return { paper: paper ? rect(paper) : null, base64: res.data };
  });

  await invoke('test_report', { json: JSON.stringify(out) });
})();
