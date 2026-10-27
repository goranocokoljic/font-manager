// The "N fonts missing" panel (E30.4): the missing fonts of the active
// document, each as the app sees it, with Activate buttons. Plain DOM, so
// any host can mount it; the matching, the conflicts and the activation are
// the app's (`fonts_needed`, `conflicts`).

import type { ConflictPolicy, FaceId, NeededFont } from './api';
import { FmApiError, type FmClient, NotRunningError } from './client';
import { PANEL_STRINGS, type PanelStrings } from './panel-strings';
import { type HostDocument, fontLabel } from './session';

/** The active document and the fonts the host reports missing in it. */
export interface PanelDocument {
  doc: HostDocument;
  missing: NeededFont[];
}

/** What the host tells the panel. */
export interface PanelSource {
  /** The active document, or `null` when none is open. */
  activeDocument(): PanelDocument | null;
}

/** How the app sees one missing font. */
export type MissingState = 'library' | 'conflict' | 'absent';

interface Row {
  key: string;
  font: NeededFont;
  label: string;
  state: MissingState;
  face?: FaceId;
}

type Choice = 'keep' | 'replace';

function fontKey(font: NeededFont): string {
  return JSON.stringify([font.postscriptName, font.fullName, font.family, font.style]);
}

let panels = 0;

/** One per panel: call `refresh()` whenever the host's documents change. */
export class MissingFontsPanel {
  private rows: Row[] = [];
  private document: HostDocument | null = null;
  private status = '';
  private message = '';
  private busy = false;
  // A refresh makes every answer still on its way stale.
  private generation = 0;
  private answer: ((choice: Choice) => void) | null = null;

  private readonly root: HTMLElement;
  private readonly count: HTMLElement;
  private readonly note: HTMLElement;
  private readonly all: HTMLButtonElement;
  private readonly list: HTMLUListElement;
  private readonly question: HTMLElement;
  private readonly questionText: HTMLElement;
  private readonly keepButton: HTMLButtonElement;

  constructor(
    container: HTMLElement,
    private readonly client: FmClient,
    private readonly source: PanelSource,
    private readonly strings: PanelStrings = PANEL_STRINGS,
  ) {
    const doc = container.ownerDocument;
    const id = `fm-missing-${++panels}`;
    const el = <K extends keyof HTMLElementTagNameMap>(tag: K, cls: string) => {
      const e = doc.createElement(tag);
      e.className = cls;
      return e;
    };

    this.root = el('section', 'fm-missing');
    this.root.setAttribute('aria-labelledby', `${id}-title`);
    const title = el('h2', 'fm-missing-title');
    title.id = `${id}-title`;
    title.textContent = strings.title;
    // The count is announced whenever it changes.
    this.count = el('p', 'fm-missing-count');
    this.count.setAttribute('role', 'status');
    this.count.setAttribute('aria-live', 'polite');
    this.count.setAttribute('aria-atomic', 'true');
    this.note = el('p', 'fm-missing-message');
    this.note.setAttribute('aria-live', 'polite');
    this.all = el('button', 'fm-missing-all');
    this.all.type = 'button';
    this.all.textContent = strings.activateAll;
    this.all.addEventListener('click', () => void this.activateAll());
    this.list = el('ul', 'fm-missing-list');
    this.list.setAttribute('aria-label', strings.listLabel);

    this.question = el('div', 'fm-missing-question');
    this.question.setAttribute('role', 'group');
    this.question.setAttribute('aria-labelledby', `${id}-question`);
    this.question.hidden = true;
    this.questionText = el('p', 'fm-missing-question-text');
    this.questionText.id = `${id}-question`;
    this.keepButton = el('button', 'fm-missing-keep');
    this.keepButton.type = 'button';
    this.keepButton.textContent = strings.keep;
    this.keepButton.addEventListener('click', () => this.answered('keep'));
    const replace = el('button', 'fm-missing-replace');
    replace.type = 'button';
    replace.textContent = strings.replace;
    replace.addEventListener('click', () => this.answered('replace'));
    this.question.append(this.questionText, this.keepButton, replace);
    this.question.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') this.answered('keep');
    });

    this.root.append(title, this.count, this.note, this.all, this.question, this.list);
    container.append(this.root);
    this.render();
  }

  /** Reads the active document again and asks the app about its missing fonts. */
  async refresh(): Promise<void> {
    const generation = ++this.generation;
    this.answered('keep');
    let active: PanelDocument | null;
    try {
      active = this.source.activeDocument();
    } catch {
      // A host that cannot say has no active document for us.
      active = null;
    }
    this.document = active?.doc ?? null;
    this.rows = [];
    this.message = '';
    if (!active) {
      this.status = this.strings.noDocument;
      return this.render();
    }
    const fonts: NeededFont[] = [];
    const seen = new Set<string>();
    for (const font of active.missing) {
      const key = fontKey(font);
      if (!seen.has(key)) {
        seen.add(key);
        fonts.push(font);
      }
    }
    if (fonts.length === 0) {
      this.status = this.strings.noneMissing;
      return this.render();
    }
    this.status = this.strings.checking;
    this.render();
    try {
      const found = await this.client.fontsNeeded({
        fonts,
        document: active.doc.key,
        activate: false,
      });
      const faces = [...new Set(found.matched.map((m) => m.face))];
      const clashes = faces.length > 0 ? await this.client.conflicts({ kind: 'faces', faces }) : [];
      if (generation !== this.generation) return;
      const byKey = new Map(found.matched.map((m) => [fontKey(m.font), m.face]));
      const clashing = new Set(clashes.map((c) => c.face));
      this.rows = fonts.map((font) => {
        const key = fontKey(font);
        const face = byKey.get(key);
        const state: MissingState =
          face === undefined ? 'absent' : clashing.has(face) ? 'conflict' : 'library';
        return { key, font, label: this.label(font), state, face };
      });
      this.status = '';
    } catch (e) {
      if (generation !== this.generation) return;
      this.status = this.errorText(e);
    }
    this.render();
  }

  /** Activates every missing font the library has; a conflict asks first. */
  activateAll(): Promise<void> {
    return this.activate(this.rows.filter((r) => r.state !== 'absent'));
  }

  private async activate(targets: Row[]): Promise<void> {
    const doc = this.document;
    if (this.busy || !doc || targets.length === 0) return;
    const generation = this.generation;
    const clashing = targets.filter((r) => r.state === 'conflict');
    let send = targets.filter((r) => r.state === 'library');
    let onConflict: ConflictPolicy = 'keepExisting';
    if (clashing.length > 0) {
      const choice = await this.ask(
        this.strings.conflictQuestion(clashing.length, clashing[0].label),
      );
      if (generation !== this.generation) return;
      if (choice === 'replace') {
        send = targets;
        onConflict = 'replace';
      }
    }
    if (send.length === 0) return;
    this.busy = true;
    this.message = '';
    this.render();
    try {
      const res = await this.client.fontsNeeded({
        fonts: send.map((r) => r.font),
        document: doc.key,
        activate: true,
        onConflict,
      });
      if (generation !== this.generation) return;
      const faceOf = new Map(res.matched.map((m) => [fontKey(m.font), m.face]));
      const outcomeOf = new Map(res.outcomes.map((o) => [o.face, o.outcome]));
      const notes: string[] = [];
      const activated = new Set<string>();
      for (const row of send) {
        const face = faceOf.get(row.key);
        const outcome = face === undefined ? undefined : outcomeOf.get(face);
        if (face === undefined) {
          // The library no longer has it.
          row.state = 'absent';
          row.face = undefined;
        } else if (outcome?.status === 'done' || outcome?.status === 'unchanged') {
          activated.add(row.key);
        } else if (outcome?.status === 'conflict') {
          row.state = 'conflict';
          if (onConflict === 'replace') notes.push(this.strings.stillConflicts(row.label));
        } else if (outcome?.status === 'skipped') {
          notes.push(this.strings.notActivated(row.label, outcome.reason));
        } else if (outcome?.status === 'failed') {
          notes.push(this.strings.notActivated(row.label, outcome.error.message));
        }
      }
      this.rows = this.rows.filter((r) => !activated.has(r.key));
      this.message = notes.join('\n');
    } catch (e) {
      if (generation !== this.generation) return;
      this.message = this.errorText(e);
    } finally {
      this.busy = false;
      this.render();
    }
  }

  private ask(text: string): Promise<Choice> {
    this.answered('keep');
    const answer = new Promise<Choice>((resolve) => {
      this.answer = resolve;
    });
    this.questionText.textContent = text;
    this.question.hidden = false;
    this.render();
    this.keepButton.focus();
    return answer;
  }

  private answered(choice: Choice): void {
    const answer = this.answer;
    if (!answer) return;
    this.answer = null;
    const focused = this.root.contains(this.root.ownerDocument.activeElement);
    this.question.hidden = true;
    this.render();
    if (focused) this.refocus();
    answer(choice);
  }

  private label(font: NeededFont): string {
    return fontLabel(font, this.strings.unknownFont);
  }

  private errorText(e: unknown): string {
    if (e instanceof NotRunningError) return this.strings.notRunning;
    if (e instanceof FmApiError) return e.message;
    return `Font Manager: ${String(e)}`;
  }

  private render(): void {
    const s = this.strings;
    const asking = this.answer !== null;
    const focused = this.root.contains(this.root.ownerDocument.activeElement);
    const count =
      this.status || (this.rows.length === 0 ? s.noneMissing : s.missing(this.rows.length));
    if (this.count.textContent !== count) this.count.textContent = count;
    if (this.note.textContent !== this.message) this.note.textContent = this.message;
    this.note.hidden = this.message === '';
    this.all.hidden = !this.rows.some((r) => r.state !== 'absent');
    this.all.disabled = this.busy || asking;
    this.list.setAttribute('aria-busy', String(this.busy));
    const doc = this.root.ownerDocument;
    const items = this.rows.map((row) => {
      const li = doc.createElement('li');
      li.className = 'fm-missing-row';
      li.dataset.state = row.state;
      const name = doc.createElement('span');
      name.className = 'fm-missing-name';
      name.textContent = row.label;
      const state = doc.createElement('span');
      state.className = 'fm-missing-state';
      state.textContent =
        row.state === 'library'
          ? s.inLibrary
          : row.state === 'conflict'
            ? s.conflict
            : s.notInLibrary;
      li.append(name, ' ', state);
      if (row.state !== 'absent') {
        const button = doc.createElement('button');
        button.type = 'button';
        button.className = 'fm-missing-activate';
        button.textContent = s.activate;
        button.setAttribute('aria-label', s.activateFont(row.label));
        button.dataset.key = row.key;
        button.disabled = this.busy || asking;
        button.addEventListener('click', () => void this.activate([row]));
        li.append(' ', button);
      }
      return li;
    });
    this.list.replaceChildren(...items);
    // Keyboard focus that sat on a button now gone stays in the panel.
    if (focused && !this.root.contains(doc.activeElement)) this.refocus();
  }

  private refocus(): void {
    const target = [this.all, ...this.list.querySelectorAll('button')].find(
      (b) => !b.hidden && !b.disabled,
    );
    if (target) {
      target.focus();
    } else {
      this.count.tabIndex = -1;
      this.count.focus();
    }
  }
}
