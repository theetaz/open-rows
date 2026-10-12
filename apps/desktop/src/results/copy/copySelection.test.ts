// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { copySelection } from './copySelection';
import { copySettings, parseCopySettings } from './copySettings';
import { displayWindow } from '../displayWindow';

vi.mock('../displayWindow', () => ({ displayWindow: vi.fn() }));
afterEach(() => {
  vi.unstubAllGlobals();
});

const defaultCopySettings = parseCopySettings(null);

const source = {
  result: {
    id: 'r',
    statement: 0,
    rows: 1,
    done: true,
    truncated: false,
    columns: [{ name: 'note', sourceType: 'text' }],
  },
  ranges: [{ r0: 0, r1: 0, c0: 0, c1: 0 }],
};
const target = { engine: 'postgres' as const, table: null };

function clipboard() {
  const write = vi.fn(() => Promise.resolve());
  const writeText = vi.fn(() => Promise.resolve());
  vi.stubGlobal('navigator', { clipboard: { write, writeText } });
  vi.mocked(displayWindow).mockResolvedValue({
    columns: 1,
    convertMs: 0,
    cells: [{ text: '<img src=x>', state: 'normal' }],
  });
  copySettings.set(defaultCopySettings);
  return { write, writeText };
}

it('writes escaped HTML and a plain text fallback in the same clipboard item', async () => {
  const { write, writeText } = clipboard();
  const items = vi.fn<(values: Record<string, Promise<Blob>>) => void>();
  vi.stubGlobal(
    'ClipboardItem',
    class {
      readonly types: string[];
      constructor(values: Record<string, Promise<Blob>>) {
        this.types = Object.keys(values);
        items(values);
      }
    },
  );
  const copying = copySelection('html', source, target);
  expect(write).toHaveBeenCalledOnce();
  await copying;
  expect(writeText).not.toHaveBeenCalled();
  const values = items.mock.calls[0]?.[0];
  expect(Object.keys(values ?? {})).toEqual(['text/html', 'text/plain']);
  expect((await values?.['text/html'])?.type).toBe('text/html');
  expect((await values?.['text/plain'])?.type).toBe('text/plain');
});

it('copies HTML markup as text on clipboards without HTML support', async () => {
  const { writeText } = clipboard();
  vi.stubGlobal('ClipboardItem', undefined);
  await copySelection('html', source, target);
  expect(writeText).toHaveBeenCalledWith(
    expect.stringContaining('&lt;img src=x&gt;'),
  );
});

it('rejects imported ambiguous copy conventions before reading cells or writing', async () => {
  const { write, writeText } = clipboard();
  vi.mocked(displayWindow).mockClear();
  copySettings.set({ ...defaultCopySettings, decimal: 'comma' });
  await expect(copySelection('csv', source, target)).rejects.toThrow(
    'cannot both be a comma',
  );
  expect(displayWindow).not.toHaveBeenCalled();
  expect(write).not.toHaveBeenCalled();
  expect(writeText).not.toHaveBeenCalled();
});
