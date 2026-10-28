import { describe, expect, it, vi } from 'vitest';
import type { FontRequest, FontSwitchAdapter, FontSwitchHost } from '../src/adapter';
import {
  COPY_A_FONT,
  type DropTarget,
  FONT_SWITCH_MIME,
  type FontDropEvent,
  NO_ADAPTER,
  applyCopiedFont,
  fontFromDrop,
  fontFromText,
  fontSwitchDropTarget,
  handleFontDrop,
} from '../src/drop';
import { INTER } from './fixtures';

function dropEvent(data: Record<string, string> | null): FontDropEvent & { prevented: boolean } {
  const event = {
    prevented: false,
    dataTransfer: data && { getData: (type: string) => data[type] ?? '' },
    preventDefault: () => void (event.prevented = true),
  };
  return event;
}

function adapterFor(host: FontSwitchHost) {
  const apply = vi.fn(async (font: FontRequest) => ({
    applied: true as const,
    font: typeof font === 'string' ? font : font.postscriptName,
  }));
  const adapter: FontSwitchAdapter = { host, apply };
  return { adapter, apply };
}

describe('fontFromDrop', () => {
  it('reads the JSON payload first', () => {
    const data = { [FONT_SWITCH_MIME]: JSON.stringify(INTER), 'text/plain': 'Other-Font' };
    expect(fontFromDrop(dropEvent(data).dataTransfer)).toEqual(INTER);
  });

  it('falls back to the PostScript name of text/plain', () => {
    expect(fontFromDrop(dropEvent({ 'text/plain': ' Inter-Regular\n' }).dataTransfer)).toBe(
      'Inter-Regular',
    );
    // A payload that is not one is skipped, not applied.
    const broken = { [FONT_SWITCH_MIME]: '{"family":"Inter"}', 'text/plain': 'Inter-Regular' };
    expect(fontFromDrop(dropEvent(broken).dataTransfer)).toBe('Inter-Regular');
  });

  it('carries no font for text that is not a name, or no data', () => {
    expect(fontFromDrop(dropEvent({ 'text/plain': 'Hello there' }).dataTransfer)).toBeNull();
    expect(fontFromDrop(dropEvent({}).dataTransfer)).toBeNull();
    expect(fontFromDrop(null)).toBeNull();
  });

  it('treats a type the host refuses to give as absent', () => {
    const data = {
      getData: (type: string) => {
        if (type === FONT_SWITCH_MIME) throw new Error('not allowed');
        return 'Inter-Regular';
      },
    };
    expect(fontFromDrop(data)).toBe('Inter-Regular');
  });
});

describe('fontFromText', () => {
  it('reads the copied payload or a name', () => {
    expect(fontFromText(JSON.stringify(INTER))).toEqual(INTER);
    expect(fontFromText('Lora-Bold')).toBe('Lora-Bold');
    expect(fontFromText('')).toBeNull();
    expect(fontFromText(null)).toBeNull();
    expect(fontFromText('[1, 2]')).toBe(null);
  });
});

describe('handleFontDrop', () => {
  it('calls the adapter of the host the detector names', async () => {
    const indesign = adapterFor('indesign');
    const photoshop = adapterFor('photoshop');
    const detect = vi.fn(() => 'photoshop' as const);
    const event = dropEvent({ [FONT_SWITCH_MIME]: JSON.stringify(INTER) });
    const result = await handleFontDrop(event, detect, {
      indesign: indesign.adapter,
      photoshop: photoshop.adapter,
    });
    expect(result).toEqual({ applied: true, font: 'Inter-Regular' });
    expect(detect).toHaveBeenCalled();
    expect(photoshop.apply).toHaveBeenCalledWith(INTER);
    expect(indesign.apply).not.toHaveBeenCalled();
    expect(event.prevented).toBe(true);
  });

  it('passes a plain-text name on as it is', async () => {
    const sketch = adapterFor('sketch');
    await handleFontDrop(dropEvent({ 'text/plain': 'Lora-Bold' }), () => 'sketch', {
      sketch: sketch.adapter,
    });
    expect(sketch.apply).toHaveBeenCalledWith('Lora-Bold');
  });

  it('says so in an app without an adapter', async () => {
    const indesign = adapterFor('indesign');
    const event = dropEvent({ 'text/plain': 'Lora-Bold' });
    expect(await handleFontDrop(event, () => null, { indesign: indesign.adapter })).toEqual({
      applied: false,
      message: NO_ADAPTER,
    });
    expect(
      await handleFontDrop(event, () => 'illustrator', { indesign: indesign.adapter }),
    ).toEqual({ applied: false, message: NO_ADAPTER });
    expect(indesign.apply).not.toHaveBeenCalled();
  });

  it('leaves a drop without a font alone', async () => {
    const indesign = adapterFor('indesign');
    const detect = vi.fn(() => 'indesign' as const);
    const event = dropEvent({ 'text/plain': 'two words' });
    expect(await handleFontDrop(event, detect, { indesign: indesign.adapter })).toBeNull();
    expect(event.prevented).toBe(false);
    expect(indesign.apply).not.toHaveBeenCalled();
  });
});

describe('applyCopiedFont', () => {
  it('applies the copied payload, or asks for one', async () => {
    const sketch = adapterFor('sketch');
    const adapters = { sketch: sketch.adapter };
    expect((await applyCopiedFont(JSON.stringify(INTER), () => 'sketch', adapters)).applied).toBe(
      true,
    );
    expect(sketch.apply).toHaveBeenCalledWith(INTER);
    expect(await applyCopiedFont('', () => 'sketch', adapters)).toEqual({
      applied: false,
      message: COPY_A_FONT,
    });
  });
});

describe('fontSwitchDropTarget', () => {
  function target() {
    const listeners = new Map<string, (event: FontDropEvent) => void>();
    const t: DropTarget = { addEventListener: (type, l) => void listeners.set(type, l) };
    return { t, fire: (type: string, event: FontDropEvent) => listeners.get(type)?.(event) };
  }
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

  it('accepts the drag and reports only what went wrong', async () => {
    const { t, fire } = target();
    const report = vi.fn();
    const adapter: FontSwitchAdapter = {
      host: 'indesign',
      apply: vi.fn(async (font: FontRequest) =>
        font === 'Inter-Regular'
          ? { applied: true as const, font }
          : { applied: false as const, message: 'Select some text first' },
      ),
    };
    fontSwitchDropTarget(t, () => 'indesign', { indesign: adapter }, report);

    const over = dropEvent(null);
    fire('dragover', over);
    expect(over.prevented).toBe(true);

    fire('drop', dropEvent({ 'text/plain': 'Inter-Regular' }));
    await settle();
    expect(report).not.toHaveBeenCalled();

    fire('drop', dropEvent({ 'text/plain': 'Lora-Bold' }));
    await settle();
    expect(report).toHaveBeenCalledWith('Select some text first');
  });

  it('reports an adapter that threw', async () => {
    const { t, fire } = target();
    const report = vi.fn();
    const adapter: FontSwitchAdapter = {
      host: 'illustrator',
      apply: async () => {
        throw new Error('ExtendScript failed');
      },
    };
    fontSwitchDropTarget(t, () => 'illustrator', { illustrator: adapter }, report);
    fire('drop', dropEvent({ 'text/plain': 'Inter-Regular' }));
    await settle();
    expect(report).toHaveBeenCalledWith('Font Switch failed: Error: ExtendScript failed');
  });
});
