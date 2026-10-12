// SPDX-License-Identifier: Apache-2.0
import { CompletionContext } from '@codemirror/autocomplete';
import { EditorState } from '@codemirror/state';
import { afterEach, expect, it } from 'vitest';
import { mountSqlEditor, parseFully } from './index';
import type { SqlEditor } from './index';
import { snippetPreview, snippetSource } from './snippets';
import type { SqlSnippet } from './snippets';

const SNIPPETS: SqlSnippet[] = [
  { name: 'Select rows', prefix: 'sel', body: 'SELECT ${1:*} FROM ${2:t};' },
  { name: 'Count', prefix: 'cnt', body: 'SELECT count(*) FROM ${1:t};' },
];

let editor: SqlEditor | undefined;
afterEach(() => {
  editor?.destroy();
  editor = undefined;
  document.body.replaceChildren();
});

it('offers snippets whose prefix starts with the typed word', async () => {
  const source = snippetSource(() => SNIPPETS);
  const state = EditorState.create({ doc: 'se' });
  const result = await source(new CompletionContext(state, 2, false));
  expect(result?.from).toBe(0);
  expect(result?.options.map((option) => option.label)).toEqual(['sel']);
  expect(result?.options[0]?.detail).toBe('Select rows');
  const none = await source(
    new CompletionContext(EditorState.create({ doc: 'x' }), 1, false),
  );
  expect(none).toBeNull();
});

it('inserts a snippet over the selection with its first field selected', () => {
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, { doc: 'xx', label: 'SQL' });
  const { view } = editor;
  view.dispatch({ selection: { anchor: 0, head: 2 } });
  editor.insertSnippet('SELECT ${1:cols} FROM ${2:t};');
  expect(view.state.doc.toString()).toBe('SELECT cols FROM t;');
  const { from, to } = view.state.selection.main;
  expect(view.state.sliceDoc(from, to)).toBe('cols');
});

it('previews a body without field markers', () => {
  expect(snippetPreview('SELECT ${1:*} FROM ${table} \\{x\\} ${}')).toBe(
    'SELECT * FROM table {x} ',
  );
});

it('reports where Run Current starts', () => {
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, {
    doc: 'SELECT 1;\nSELECT 2;',
    label: 'SQL',
  });
  expect(parseFully(editor.view, 50)).toBe(true);
  editor.view.dispatch({ selection: { anchor: 14 } });
  expect(editor.runStart()).toBe(10);
  editor.view.dispatch({ selection: { anchor: 2, head: 5 } });
  expect(editor.runStart()).toBe(2);
});

it('expands saved query keywords as literal SQL without snippet field interpretation', async () => {
  const body = "SELECT :id, $1, '${1:literal}';";
  const source = snippetSource(() => [
    { name: 'reports/top.sql', prefix: 'top', body, literal: true },
  ]);
  const result = await source(
    new CompletionContext(EditorState.create({ doc: 'to' }), 2, false),
  );
  expect(result?.options[0]?.apply).toBe(body);
  expect(result?.options[0]?.detail).toBe('reports/top.sql');
});
