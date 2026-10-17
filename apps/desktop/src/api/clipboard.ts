// The system clipboard, behind one function so components do not reach for
// browser APIs themselves and tests can replace it.

/** Puts `text` on the clipboard; rejects when the webview refuses. */
export function copyText(text: string): Promise<void> {
  if (!navigator.clipboard) return Promise.reject(new Error('No clipboard in this webview.'));
  return navigator.clipboard.writeText(text);
}
