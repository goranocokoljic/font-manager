// @vitest-environment jsdom
import { fontSwitchDropTarget, hostFromName, inDesignAdapter } from '@fm/font-switch';
import { describe, expect, it, vi } from 'vitest';
import indexHtml from '../index.html?raw';
import packageJson from '../package.json';
import main from '../src/main.ts?raw';

// The panel is the drop target of Font Switch (E34): what is dropped on its
// document goes to `@fm/font-switch`'s adapter for the host.

function drop(doc: Document, data: Record<string, string>): Event {
  const event = new Event('drop', { cancelable: true });
  Object.assign(event, { dataTransfer: { getData: (type: string) => data[type] ?? '' } });
  doc.dispatchEvent(event);
  return event;
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('Font Switch in the panel', () => {
  it('applies a font dropped on the panel to the selected InDesign text', async () => {
    const font = { postscriptName: 'Inter-Regular', fontFamily: 'Inter', fontStyleName: 'Regular' };
    const words = { constructorName: 'Word', appliedFont: 'Minion' as unknown };
    const app = { selection: [words], fonts: [font] };
    const report = vi.fn();
    fontSwitchDropTarget(
      document,
      () => hostFromName('InDesign'),
      { indesign: inDesignAdapter(app) },
      report,
    );

    const over = new Event('dragover', { cancelable: true });
    document.dispatchEvent(over);
    expect(over.defaultPrevented).toBe(true);

    const payload = { postscriptName: 'Inter-Regular', family: 'Inter', style: 'Regular' };
    const event = drop(document, {
      'application/x-fm-font-switch': JSON.stringify({ ...payload, fileUrl: 'file:///f.otf' }),
    });
    await settle();
    expect(event.defaultPrevented).toBe(true);
    expect(words.appliedFont).toBe(font);
    expect(report).not.toHaveBeenCalled();

    app.selection = [];
    drop(document, { 'text/plain': 'Inter-Regular' });
    await settle();
    expect(report).toHaveBeenCalledWith('Select some text first');
  });

  it('is wired from @fm/font-switch, not copied', () => {
    expect(packageJson.dependencies).toHaveProperty('@fm/font-switch', 'workspace:*');
    expect(main).toContain("from '@fm/font-switch'");
    expect(main).toContain('fontSwitchDropTarget(');
    expect(main).toContain('inDesignAdapter(');
    expect(main).toContain('photoshopAdapter(');
    expect(indexHtml).toContain('Drop a font from Font Manager here');
  });
});
