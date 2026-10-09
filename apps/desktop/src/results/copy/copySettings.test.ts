// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { formatTable } from './formats';
import type { CopyTable } from './formats';
import { delimitedOptions, parseCopySettings } from './copySettings';

const table: CopyTable = {
  columns: [
    { name: 'id', kind: 'number' },
    { name: 'note', kind: 'text' },
  ],
  rows: [
    ['1', 'a,b'],
    ['2', null],
  ],
};
const pg = { engine: 'postgres' as const, table: '"t"' };
const defaults = parseCopySettings(null);

it('keeps each format’s header and NULL convention by default', () => {
  expect(delimitedOptions('tsv', defaults)).toEqual({
    headers: false,
    nulls: 'text',
    quote: 'minimal',
  });
  expect(delimitedOptions('csv', defaults)).toEqual({
    headers: true,
    nulls: 'empty',
    quote: 'minimal',
  });
  expect(delimitedOptions('tsv', defaults, true).headers).toBe(true);
});

it('writes headers, NULL and quotes as the copy settings ask', () => {
  const settings = parseCopySettings({
    headers: 'always',
    nulls: 'empty',
    quote: 'all',
  });
  expect(formatTable('tsv', table, pg, delimitedOptions('tsv', settings))).toBe(
    '"id"\t"note"\n"1"\t"a,b"\n"2"\t',
  );
  const never = parseCopySettings({ headers: 'never', nulls: 'text' });
  expect(formatTable('csv', table, pg, delimitedOptions('csv', never))).toBe(
    '1,"a,b"\n2,NULL',
  );
});

it('validates stored copy settings field by field', () => {
  expect(parseCopySettings({ headers: 'sometimes', quote: 'all' })).toEqual({
    headers: 'format',
    nulls: 'format',
    quote: 'all',
  });
  expect(parseCopySettings('x')).toEqual(defaults);
});
