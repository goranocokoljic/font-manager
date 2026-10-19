// The small Markdown subset the user docs are written in, rendered to HTML with
// no dependencies: `#`–`###` headings, paragraphs, `-` and `1.` lists, pipe
// tables, `>` notes, fenced code, and inline `code`, **bold**, _emphasis_ and
// [links](page.md) to other pages. Everything else is text: the source is escaped, so raw HTML
// in a page shows as written instead of running.

const ESCAPES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

/** Escapes text for HTML element content and attribute values. */
export function escapeHtml(text) {
  return text.replace(/[&<>"']/g, (c) => ESCAPES[c]);
}

/** A link target as the site serves it: `page.md#part` becomes `page.html#part`. */
export function siteHref(href) {
  const m = /^([a-z0-9-]+)\.md(#.*)?$/.exec(href);
  return m ? `${m[1]}.html${m[2] ?? ''}` : href;
}

/** A heading's anchor id: lower case, words joined by `-`. */
export function slugify(text) {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .trim()
    .replace(/\s+/g, '-');
}

/** Renders one line of inline Markdown. Code spans are taken out first, so their content stays literal. */
export function renderInline(text) {
  const codes = [];
  let s = text.replace(/`([^`]+)`/g, (_, code) => {
    codes.push(code);
    return `\u0000${codes.length - 1}\u0000`;
  });
  s = escapeHtml(s);
  s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, label, href) => {
    // Only links to other pages and anchors: the help window stays on the bundled,
    // offline pages (no web pages, no script URLs). Any other link is plain text.
    const target = siteHref(href.replace(/&amp;/g, '&'));
    if (!/^(?:[a-z0-9-]+\.html(?:#[\w-]*)?|#[\w-]+)$/.test(target)) return label;
    return `<a href="${escapeHtml(target)}">${label}</a>`;
  });
  s = s.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  s = s.replace(/(^|[\s(])_([^_]+)_(?=[\s).,;:!?]|$)/g, '$1<em>$2</em>');
  // eslint-disable-next-line no-control-regex
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${escapeHtml(codes[Number(i)])}</code>`);
}

const cells = (line) =>
  line
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split('|')
    .map((c) => c.trim());

const isTableRule = (line) => /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$/.test(line);

/**
 * Renders a page. Returns the body HTML and the text of its first `#` heading,
 * which is the page title; `null` when the page has none.
 */
export function renderMarkdown(source) {
  const lines = source.replace(/\r\n?/g, '\n').split('\n');
  const out = [];
  let title = null;
  let i = 0;
  const para = [];
  const flush = () => {
    if (para.length) out.push(`<p>${renderInline(para.join(' '))}</p>`);
    para.length = 0;
  };

  while (i < lines.length) {
    const line = lines[i];
    if (/^```/.test(line)) {
      flush();
      const code = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) code.push(lines[i++]);
      i++; // the closing fence (or the end of the page)
      out.push(`<pre><code>${escapeHtml(code.join('\n'))}</code></pre>`);
      continue;
    }
    const h = /^(#{1,3})\s+(.+?)\s*#*\s*$/.exec(line);
    if (h) {
      flush();
      const level = h[1].length;
      if (level === 1 && title === null) title = h[2];
      out.push(`<h${level} id="${slugify(h[2])}">${renderInline(h[2])}</h${level}>`);
      i++;
      continue;
    }
    if (/^\s*\|/.test(line) && i + 1 < lines.length && isTableRule(lines[i + 1])) {
      flush();
      const head = cells(line);
      i += 2;
      const rows = [];
      while (i < lines.length && /^\s*\|/.test(lines[i])) rows.push(cells(lines[i++]));
      const th = head.map((c) => `<th scope="col">${renderInline(c)}</th>`).join('');
      const tb = rows
        .map((r) => `<tr>${head.map((_, k) => `<td>${renderInline(r[k] ?? '')}</td>`).join('')}</tr>`)
        .join('\n');
      out.push(`<table>\n<thead><tr>${th}</tr></thead>\n<tbody>\n${tb}\n</tbody>\n</table>`);
      continue;
    }
    const list = /^(\s*)(-|\d+\.)\s+/.exec(line);
    if (list) {
      flush();
      const ordered = list[2] !== '-';
      const items = [];
      while (i < lines.length) {
        const m = /^\s*(-|\d+\.)\s+(.*)$/.exec(lines[i]);
        if (m && (m[1] !== '-') === ordered) items.push(m[2]);
        else if (items.length && /^\s{2,}\S/.test(lines[i])) items[items.length - 1] += ` ${lines[i].trim()}`;
        else break;
        i++;
      }
      const tag = ordered ? 'ol' : 'ul';
      out.push(`<${tag}>\n${items.map((t) => `<li>${renderInline(t)}</li>`).join('\n')}\n</${tag}>`);
      continue;
    }
    if (/^>\s?/.test(line)) {
      flush();
      const quote = [];
      while (i < lines.length && /^>\s?/.test(lines[i])) quote.push(lines[i++].replace(/^>\s?/, ''));
      out.push(`<aside class="note"><p>${renderInline(quote.join(' '))}</p></aside>`);
      continue;
    }
    if (line.trim() === '') flush();
    else para.push(line.trim());
    i++;
  }
  flush();
  return { html: out.join('\n'), title };
}
