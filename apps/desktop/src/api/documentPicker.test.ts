import { afterEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from './client';

const open = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open }));

afterEach(() => open.mockReset());

describe('the picker of "Check a document\'s fonts…" (E32.2)', () => {
  it('opens the native picker for one file, filtered to the documents fm-core reads', async () => {
    open.mockResolvedValueOnce('/Users/demo/Brand brochure.indd');
    expect(await tauriClient.pickDocument('Pick', 'Design documents')).toBe(
      '/Users/demo/Brand brochure.indd',
    );
    expect(open).toHaveBeenCalledWith({
      directory: false,
      multiple: false,
      title: 'Pick',
      filters: [
        {
          name: 'Design documents',
          extensions: ['indd', 'ai', 'psd', 'sketch', 'fig', 'docx', 'pptx'],
        },
      ],
    });
  });

  it('is null when the user cancels', async () => {
    open.mockResolvedValueOnce(null);
    expect(await tauriClient.pickDocument('Pick', 'Design documents')).toBeNull();
  });
});
