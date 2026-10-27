// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import manifestXml from '../CSXS/manifest.xml?raw';

/** The minimum Illustrator: 27.0 (2023) is the first with CEP 11. */
const MIN_ILLUSTRATOR = 27;

function parse(xml: string): Document {
  return new DOMParser().parseFromString(xml, 'application/xml');
}

const all = (root: ParentNode, selector: string) =>
  [...root.querySelectorAll(selector)].map((e) => e.textContent?.trim() ?? '');

/** What is wrong with a CSXS manifest; empty when it has what the extension needs. */
function problems(xml: string): string[] {
  const doc = parse(xml);
  if (doc.querySelector('parsererror')) return ['not XML'];
  const out: string[] = [];
  const root = doc.documentElement;
  if (root.tagName !== 'ExtensionManifest') out.push('the root is not ExtensionManifest');
  if (!root.getAttribute('ExtensionBundleId')) out.push('no ExtensionBundleId');

  const hosts = [...doc.querySelectorAll('HostList > Host')];
  const ilst = hosts.filter((h) => h.getAttribute('Name') === 'ILST');
  if (hosts.length !== 1 || ilst.length !== 1) out.push('the host is not ILST alone');
  const range = /^\[(\d+(?:\.\d+)?),\s*(\d+(?:\.\d+)?)\]$/.exec(
    ilst[0]?.getAttribute('Version') ?? '',
  );
  if (!range) out.push('ILST has no [min,max] version');
  else if (Number(range[1]) < MIN_ILLUSTRATOR) out.push(`ILST minimum below ${MIN_ILLUSTRATOR}`);

  const csxs = doc.querySelector('RequiredRuntime[Name="CSXS"]')?.getAttribute('Version');
  if (!csxs || Number.parseFloat(csxs) < 11) out.push('CSXS runtime below 11.0');

  const listed = [...doc.querySelectorAll('ExtensionList > Extension')].map((e) => e.id);
  const dispatched = [...doc.querySelectorAll('DispatchInfoList > Extension')].map((e) => e.id);
  if (listed.length !== 1 || dispatched.join() !== listed.join()) {
    out.push('ExtensionList and DispatchInfoList do not name the same one extension');
  }
  const flags = all(doc, 'CEFCommandLine > Parameter');
  if (!flags.includes('--enable-nodejs')) out.push('Node is not enabled');
  if (!flags.includes('--mixed-context')) out.push('no mixed context');
  if (all(doc, 'Resources > MainPath').join() !== './index.html') out.push('MainPath');
  if (all(doc, 'Resources > ScriptPath').join() !== './jsx/host.jsx') out.push('ScriptPath');
  if (all(doc, 'Lifecycle > StartOn > Event').length === 0)
    out.push('it does not start on its own');
  return out;
}

describe('CSXS/manifest.xml', () => {
  it('has the elements the extension needs', () => {
    expect(problems(manifestXml)).toEqual([]);
  });

  it('names ILST with a minimum version and Node enabled', () => {
    const doc = parse(manifestXml);
    const host = doc.querySelector('Host');
    expect(host?.getAttribute('Name')).toBe('ILST');
    expect(host?.getAttribute('Version')).toMatch(/^\[27\.0,/);
    expect(all(doc, 'CEFCommandLine > Parameter')).toContain('--enable-nodejs');
  });

  it.each<[string, string, string]>([
    ['another host', 'Name="ILST"', 'Name="PHXS"'],
    ['no minimum version', 'Version="[27.0,99.9]"', 'Version="27.0"'],
    ['an older Illustrator', '[27.0,99.9]', '[22.0,99.9]'],
    ['Node off', '<Parameter>--enable-nodejs</Parameter>', ''],
    ['an older runtime', 'Name="CSXS" Version="11.0"', 'Name="CSXS" Version="9.0"'],
    ['another script', './jsx/host.jsx', './host.jsx'],
    ['a broken file', '</ExtensionManifest>', ''],
  ])('refuses %s', (_, from, to) => {
    const edited = manifestXml.replace(from, to);
    expect(edited).not.toBe(manifestXml);
    expect(problems(edited)).not.toEqual([]);
  });
});
