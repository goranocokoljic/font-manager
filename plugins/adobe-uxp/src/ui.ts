// The plugin's two UI pieces in a UXP host: a message that does not wait
// for the user and a yes/no question. UXP dialogs are HTML `<dialog>`s.

import type { PluginUi } from '@fm/plugin-shared';

/** A UXP dialog: `uxpShowModal` waits for the user, `show` does not. */
type UxpDialog = HTMLDialogElement & { uxpShowModal?: () => Promise<string> };

function dialog(doc: Document, message: string, buttons: [string, string][]): UxpDialog {
  const d = doc.createElement('dialog') as UxpDialog;
  const form = doc.createElement('form');
  form.method = 'dialog';
  const p = doc.createElement('p');
  p.textContent = message;
  form.append(p);
  // The buttons in a row of their own, so `index.html` can lay them out.
  const row = doc.createElement('div');
  row.className = 'fm-dialog-buttons';
  for (const [value, label] of buttons) {
    const b = doc.createElement('button');
    b.type = 'button';
    b.value = value;
    b.textContent = label;
    b.addEventListener('click', () => d.close(value));
    row.append(b);
  }
  form.append(row);
  d.append(form);
  doc.body.append(d);
  d.addEventListener('close', () => d.remove());
  return d;
}

export function uxpUi(doc: Document): PluginUi {
  return {
    notify(message) {
      // Shown and left there: nothing waits for the click.
      const status = doc.getElementById('status');
      if (status) status.textContent = message;
      const d = dialog(doc, message, [['ok', 'OK']]);
      if (typeof d.show === 'function') d.show();
    },
    async confirm(message) {
      const d = dialog(doc, message, [
        ['no', 'Keep them'],
        ['yes', 'Deactivate'],
      ]);
      const closed = new Promise<string>((resolve) =>
        d.addEventListener('close', () => resolve(d.returnValue)),
      );
      if (d.uxpShowModal) return (await d.uxpShowModal()) === 'yes';
      d.showModal();
      return (await closed) === 'yes';
    },
  };
}
