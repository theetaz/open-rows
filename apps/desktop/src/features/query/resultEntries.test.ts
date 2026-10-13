// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { emptyEntry } from '../table/entries';
import { resultEntry, resultEntryIds } from './resultEntries';
it('includes every result in its parent close guard without mixing other query tabs', () => {
  const pending = {
    ...emptyEntry,
    edits: { ...emptyEntry.edits, pending: true },
  };
  const entries = {
    'query:result:one': emptyEntry,
    'query:result:two': pending,
    'other:result:one': emptyEntry,
  };
  expect(resultEntry(entries, 'query')?.edits.pending).toBe(true);
  expect(resultEntry(entries, 'other')?.edits.pending).toBe(false);
  expect(resultEntryIds(entries, 'query')).toEqual([
    'query',
    'query:result:one',
    'query:result:two',
  ]);
  expect(resultEntry(entries, 'query:result:one')).toBe(emptyEntry);
});
