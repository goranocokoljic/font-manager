// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import { uxpUi } from '../src/ui';

type UxpDialog = HTMLDialogElement & { uxpShowModal?: () => Promise<string> };

describe('uxpUi', () => {
  it('shows a message without waiting for the user', () => {
    document.body.innerHTML = '<p id="status"></p>';
    const show = vi.fn();
    HTMLDialogElement.prototype.show = show;
    uxpUi(document).notify('Font Manager is not running');
    expect(show).toHaveBeenCalledOnce();
    expect(document.getElementById('status')?.textContent).toBe('Font Manager is not running');
    expect(document.querySelector('dialog p')?.textContent).toBe('Font Manager is not running');
  });

  it.each([
    ['yes', true],
    ['no', false],
  ])('answers the question with the button the user pressed (%s)', async (value, answer) => {
    document.body.innerHTML = '';
    const proto = HTMLDialogElement.prototype as UxpDialog;
    proto.uxpShowModal = function (this: UxpDialog) {
      const button = this.querySelector<HTMLButtonElement>(`button[value="${value}"]`);
      return new Promise<string>((resolve) => {
        this.addEventListener('close', () => resolve(this.returnValue));
        button?.click();
      });
    };
    HTMLDialogElement.prototype.close = function (this: HTMLDialogElement, v?: string) {
      this.returnValue = v ?? '';
      this.dispatchEvent(new Event('close'));
    };
    const ui = uxpUi(document);
    await expect(ui.confirm('Deactivate 2 fonts used only by Poster.indd?')).resolves.toBe(answer);
    expect(document.querySelector('dialog')).toBeNull();
  });
});
