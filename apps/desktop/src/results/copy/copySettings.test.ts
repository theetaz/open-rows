// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { formatTable } from './formats';
import type { CopyTable } from './formats';
import {
  copyConflict,
  copySettings,
  delimitedOptions,
  parseCopySettings,
  updateCopySettings,
} from './copySettings';

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
  expect(delimitedOptions('tsv', defaults)).toMatchObject({
    headers: false,
    nulls: 'text',
    quote: 'minimal',
    delimiter: '\t',
  });
  expect(delimitedOptions('csv', defaults)).toMatchObject({
    headers: true,
    nulls: 'empty',
    quote: 'minimal',
    delimiter: ',',
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
    ...defaults,
    quote: 'all',
  });
  expect(parseCopySettings({ delimiter: ':', decimal: 'comma' })).toEqual({
    ...defaults,
    decimal: 'comma',
  });
  expect(parseCopySettings('x')).toEqual(defaults);
});

it('writes the chosen delimiter, quote, newline and decimal separator', () => {
  const settings = parseCopySettings({
    delimiter: 'semicolon',
    quoteChar: 'single',
    newline: 'crlf',
    decimal: 'comma',
  });
  const decimals: CopyTable = {
    columns: [
      { name: 'price', kind: 'number' },
      { name: 'note', kind: 'text' },
    ],
    rows: [
      ['1.5', "it's; ok"],
      ['2', '3.5'],
    ],
  };
  expect(
    formatTable('csv', decimals, pg, delimitedOptions('csv', settings)),
  ).toBe("price;note\r\n1,5;'it''s; ok'\r\n2;3.5");
});

it('rejects a decimal comma inside comma-separated fields', () => {
  const ambiguous = parseCopySettings({ delimiter: 'comma', decimal: 'comma' });
  expect(copyConflict(ambiguous)).toMatch(/cannot both be a comma/);
  expect(copyConflict({ ...ambiguous, delimiter: 'semicolon' })).toBeNull();
  expect(updateCopySettings({ decimal: 'comma' })).toMatch(/comma/);
  expect(copySettings.get().decimal).toBe('point');
  expect(updateCopySettings({ delimiter: 'tab', decimal: 'comma' })).toBeNull();
  expect(copySettings.get()).toMatchObject({
    delimiter: 'tab',
    decimal: 'comma',
  });
});

it('copies an escaped HTML table with NULL handling', () => {
  const html = formatTable(
    'html',
    table,
    pg,
    delimitedOptions('html', defaults),
  );
  expect(html).toContain('<thead><tr><th>id</th><th>note</th></tr></thead>');
  expect(html).toContain(
    '<tr><td style="text-align:right">2</td><td>NULL</td></tr>',
  );
  const tags: CopyTable = {
    columns: [{ name: 'a<b', kind: 'text' }],
    rows: [['x & "y"']],
  };
  expect(formatTable('html', tags, pg)).toContain(
    '<th>a&lt;b</th></tr></thead>\n<tbody>\n<tr><td>x &amp; &quot;y&quot;</td></tr>',
  );
});

it('preserves line breaks as escaped HTML table content', () => {
  const lines: CopyTable = {
    columns: [{ name: 'note', kind: 'text' }],
    rows: [['<script>\r\n&next\nlast']],
  };
  expect(formatTable('html', lines, pg)).toContain(
    '<td>&lt;script&gt;<br>&amp;next<br>last</td>',
  );
});
