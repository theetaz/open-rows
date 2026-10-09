// SPDX-License-Identifier: Apache-2.0
import { vi } from 'vitest';
import type { ImportPreviewDto } from '@openrows/ipc';

/** A synthetic `people.csv`: `id` and `name`, the second id is not a number. */
export const samplePreview: ImportPreviewDto = {
  options: {
    format: 'delimited',
    delimiter: ',',
    quote: '"',
    header: true,
    encoding: 'utf-8',
    emptyAsNull: true,
  },
  columns: [
    {
      name: 'ID',
      category: 'text',
      nullable: false,
      suggestedType: 'text',
    },
    {
      name: 'name',
      category: 'text',
      nullable: true,
      suggestedType: 'text',
    },
  ],
  rows: [
    ['1', 'Ada'],
    ['x2', null],
  ],
  lines: [2, 3],
  invalid: [],
  errors: [],
  sampled: 2,
};

/** Import commands: one picked file, its preview, jobs that start. */
export function importStub(): Record<string, unknown> {
  return {
    pickImportFile: vi.fn(() =>
      Promise.resolve({
        status: 'picked',
        fileId: 'file-1',
        fileName: 'people.csv',
        sizeBytes: 64,
      }),
    ),
    previewImport: vi.fn(() => Promise.resolve(samplePreview)),
    startImport: vi.fn(() => Promise.resolve({ importId: 'import-1' })),
    cancelImport: vi.fn(() => Promise.resolve()),
  };
}
