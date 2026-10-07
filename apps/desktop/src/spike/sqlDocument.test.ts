// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { DOCUMENT_LINES, generateSqlDocument } from './sqlDocument';
import { POSITIONS, TYPED_TEXT, keystrokes } from './editorScript';

it('generates exactly 5 000 deterministic PostgreSQL lines', () => {
  const doc = generateSqlDocument();
  expect(doc.split('\n')).toHaveLength(DOCUMENT_LINES);
  expect(generateSqlDocument()).toBe(doc);
  for (const feature of ['$$', '::jsonb', '@>', 'ILIKE', "E'", 'OVER (']) {
    expect(doc).toContain(feature);
  }
  expect(doc.length).toBeGreaterThan(150_000);
});

it('honours smaller sizes and pads without splitting statements', () => {
  const lines = generateSqlDocument(12).split('\n');
  expect(lines).toHaveLength(12);
  expect(lines.at(-1)).toMatch(/^SELECT \d+ AS filler; -- padding$/);
});

it('scripts sixty inserts and twenty deletes per edit site', () => {
  expect(POSITIONS.map((position) => position.line)).toEqual([2, 2_500, 5_000]);
  expect(TYPED_TEXT).toMatch(/^[\x20-\x7e]{60}$/);
  const sql = keystrokes('sql');
  expect(sql.filter((stroke) => stroke.kind === 'insert')).toHaveLength(60);
  expect(sql.slice(60)).toEqual(
    Array.from({ length: 20 }, () => ({ kind: 'delete', key: 'Backspace' })),
  );
  const vim = keystrokes('vim');
  expect(vim[0]).toEqual({ kind: 'mode', key: 'A', mode: 'insert' });
  expect(vim[61]).toEqual({ kind: 'mode', key: 'Escape', mode: 'normal' });
  expect(vim.slice(62).every((stroke) => stroke.key === 'x')).toBe(true);
  expect(vim).toHaveLength(82);
});
