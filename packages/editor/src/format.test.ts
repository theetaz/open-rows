// SPDX-License-Identifier: Apache-2.0
import { EditorState } from '@codemirror/state';
import { afterEach, expect, it } from 'vitest';
import { sqlEditorExtensions } from './extensions';
import { beautifySql, uglifySql } from './format';
import type { FormatOptions } from './format';
import { mountSqlEditor } from './index';
import type { SqlEditor } from './index';
import { currentStatement, editingRange } from './statement';

const options: FormatOptions = {
  dialect: 'postgres',
  indentWidth: 2,
  indentWith: 'spaces',
  keywordCase: 'upper',
  indentStyle: 'standard',
  linesBetweenQueries: 1,
  denseOperators: false,
};

let editor: SqlEditor | undefined;
afterEach(() => {
  editor?.destroy();
  editor = undefined;
  document.body.replaceChildren();
});

function mount(doc: string): SqlEditor {
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, { doc, label: 'SQL' });
  return editor;
}

it('uglifies onto one line, keeping literals and turning line comments into block comments', () => {
  const sql =
    "SELECT a,\n       'x  y' -- note\nFROM   t\n/* keep  me */ WHERE b = 1";
  expect(uglifySql(sql)).toBe(
    "SELECT a, 'x  y' /* note */ FROM t /* keep  me */ WHERE b = 1",
  );
});

it('beautifies with the chosen indent and keyword case', async () => {
  const pretty = await beautifySql('select id, name from users where id = 1', {
    ...options,
    indentWidth: 4,
  });
  expect(pretty).toBe(
    'SELECT\n    id,\n    name\nFROM\n    users\nWHERE\n    id = 1',
  );
  const tabs = await beautifySql('select a from t', {
    ...options,
    indentWith: 'tabs',
    keywordCase: 'lower',
  });
  expect(tabs).toBe('select\n\ta\nfrom\n\tt');
});

it('aligns keywords in a column with the tabular indent types', async () => {
  const sql = 'select a, b from t where a = 1';
  const left = await beautifySql(sql, {
    ...options,
    indentStyle: 'tabularLeft',
  });
  expect(left.split('\n')[0]).toBe('SELECT    a,');
  expect(left).toContain('\nFROM      t');
  const right = await beautifySql(sql, {
    ...options,
    indentStyle: 'tabularRight',
  });
  expect(right.split('\n')[0]).toBe('   SELECT a,');
  expect(right).toContain('\n     FROM t');
});

it('formats the whole document without a selection, as one undoable change', async () => {
  const { view, format } = mount('select 1;\nselect a,\n  b from t;');
  view.dispatch({ selection: { anchor: 14 } });
  await format('uglify', options);
  expect(view.state.doc.toString()).toBe('select 1; select a, b from t;');
  await format('beautify', options);
  expect(view.state.doc.toString()).toBe(
    'SELECT\n  1;\n\nSELECT\n  a,\n  b\nFROM\n  t;',
  );
});

it('separates statements and packs operators as configured', async () => {
  const pretty = await beautifySql('select 1; select a from t where a = 1', {
    ...options,
    linesBetweenQueries: 2,
    denseOperators: true,
  });
  expect(pretty).toBe('SELECT\n  1;\n\n\nSELECT\n  a\nFROM\n  t\nWHERE\n  a=1');
});

it('keeps parameter placeholders and comments when formatting', async () => {
  const cases: [FormatOptions['dialect'], string, string[]][] = [
    [
      'postgres',
      'select a from t where id = :id and b = $1 -- why',
      ['id = :id', 'b = $1', '-- why'],
    ],
    [
      'mysql',
      'select a from t /* note */ where id = :id and b = ?',
      ['id = :id', 'b = ?', '/* note */'],
    ],
    [
      'sqlite',
      'select a from t where a = ?1 and b = @b and c = $c and d = ?',
      ['a = ?1', 'b = @b', 'c = $c', 'd = ?'],
    ],
  ];
  for (const [dialect, sql, kept] of cases) {
    const pretty = await beautifySql(sql, { ...options, dialect });
    for (const text of kept) expect(pretty, dialect).toContain(text);
    const ugly = uglifySql(pretty);
    for (const text of kept.filter((item) => !item.startsWith('--')))
      expect(ugly, dialect).toContain(text);
  }
  expect(uglifySql('select :id -- why')).toBe('select :id /* why */');
});

it('formats the selection when there is one', async () => {
  const { view, format } = mount('select  1;  select  2;');
  view.dispatch({ selection: { anchor: 0, head: 10 } });
  await format('uglify', options);
  expect(view.state.doc.toString()).toBe('select 1;  select  2;');
});

it('finds the statement to run and the statement being typed', () => {
  const doc = 'SELECT 1;\n\nSELECT 2 FROM t;\nSEL';
  const state = EditorState.create({
    doc,
    extensions: sqlEditorExtensions({ doc, label: 'SQL' }),
  });
  expect(currentStatement(state, 3)).toEqual({ from: 0, to: 9 });
  expect(currentStatement(state, 10)).toEqual({ from: 0, to: 9 });
  expect(currentStatement(state, 15)).toEqual({ from: 11, to: 27 });
  expect(editingRange(state, 9)).toEqual({ from: 9, to: 9 });
  expect(editingRange(state, doc.length)).toEqual({
    from: 28,
    to: doc.length,
  });
});
