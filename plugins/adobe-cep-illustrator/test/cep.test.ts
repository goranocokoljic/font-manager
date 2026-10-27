import { describe, expect, it } from 'vitest';
import {
  type CepNative,
  EVAL_SCRIPT_ERROR,
  cepExtendScript,
  scriptCall,
  scriptString,
} from '../src/cep';
import { extendScriptUi } from '../src/ui';

function cepAnswering(answer: (script: string) => string) {
  const scripts: string[] = [];
  const cep: CepNative = {
    evalScript: (script, callback) => {
      scripts.push(script);
      callback(answer(script));
    },
    addEventListener: () => {},
  };
  return { cep, scripts };
}

describe('cepExtendScript', () => {
  it("resolves with the script's answer", async () => {
    const { cep } = cepAnswering(() => '[]');
    await expect(cepExtendScript(cep).evalScript('fmOpenDocuments()')).resolves.toBe('[]');
  });

  it('rejects when the script threw', async () => {
    const { cep } = cepAnswering(() => EVAL_SCRIPT_ERROR);
    await expect(cepExtendScript(cep).evalScript('nope()')).rejects.toThrow(/ExtendScript/);
  });
});

describe('scriptString', () => {
  it('is an ES3 string literal of any text', () => {
    const text = 'a"b\\c\nd\u2028e\u2029f';
    const literal = scriptString(text);
    expect(literal).not.toMatch(/[\u2028\u2029\n]/);
    expect(new Function(`return ${literal};`)()).toBe(text);
    expect(scriptCall('fmAsk', 'x', 'y')).toBe('fmAsk("x","y")');
  });
});

describe('extendScriptUi', () => {
  it.each([
    ['yes', true],
    ['no', false],
    [EVAL_SCRIPT_ERROR, false],
  ])('answers the question with fmAsk (%s)', async (answer, expected) => {
    const { cep, scripts } = cepAnswering(() => answer);
    const ui = extendScriptUi(cepExtendScript(cep));
    await expect(ui.confirm('Deactivate 1 font used only by A.ai?')).resolves.toBe(expected);
    expect(scripts).toEqual(['fmAsk("Deactivate 1 font used only by A.ai?")']);
  });

  it('shows a message with fmTell and does not wait for it', () => {
    const { cep, scripts } = cepAnswering(() => EVAL_SCRIPT_ERROR);
    extendScriptUi(cepExtendScript(cep)).notify('Font Manager is not running');
    expect(scripts).toEqual(['fmTell("Font Manager is not running")']);
  });
});
