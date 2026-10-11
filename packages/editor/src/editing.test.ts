// SPDX-License-Identifier: Apache-2.0
import { CompletionContext } from '@codemirror/autocomplete';
import { foldable, ensureSyntaxTree } from '@codemirror/language';
import { selectNextOccurrence } from '@codemirror/search';
import { EditorState } from '@codemirror/state';
import { afterEach, expect, it } from 'vitest';
import { sqlCompletionSource } from './completion/source';
import { sqlEditorExtensions } from './extensions';
import { blockFold } from './folding';
import { mountSqlEditor } from './index';
import type { SqlEditor } from './index';
import { addsCaret } from './multiCursor';
import { defaultPreferences } from './preferences';
import type { EditorPreferences } from './preferences';
import { snippetSource } from './snippets';

let editor: SqlEditor | undefined;
afterEach(() => {
  editor?.destroy();
  editor = undefined;
  document.body.replaceChildren();
});

function stateOf(
  doc: string,
  change: Partial<EditorPreferences> = {},
): EditorState {
  const state = EditorState.create({
    doc,
    extensions: sqlEditorExtensions({
      doc,
      label: 'SQL',
      preferences: { ...defaultPreferences, ...change },
    }),
  });
  ensureSyntaxTree(state, state.doc.length, 1000);
  return state;
}

it('adds carets with ⌥-click and selects the next occurrence with ⌘D', () => {
  expect(addsCaret(new MouseEvent('mousedown', { altKey: true }))).toBe(true);
  expect(addsCaret(new MouseEvent('mousedown'))).toBe(false);
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, {
    doc: 'SELECT id, id FROM t',
    label: 'SQL',
  });
  const { view } = editor;
  view.dispatch({ selection: { anchor: 7, head: 9 } });
  selectNextOccurrence(view);
  expect(view.state.selection.ranges.map((range) => range.from)).toEqual([
    7, 11,
  ]);
});

it('folds multi-line statements and bracketed blocks', () => {
  const doc = 'SELECT a,\n  b\nFROM t\nWHERE id IN (\n  1,\n  2\n);';
  const state = stateOf(doc);
  expect(foldable(state, 0, state.doc.line(1).to)).not.toBeNull();
  const line = state.doc.line(4);
  const block = blockFold(state, line.from, line.to);
  expect(block && state.sliceDoc(block.from, block.to)).toBe('\n  1,\n  2\n');
  expect(blockFold(state, state.doc.line(1).from, state.doc.line(1).to)).toBe(
    null,
  );
});

it('turns line numbers and folding off', () => {
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, {
    doc: 'SELECT 1',
    label: 'SQL',
    preferences: { ...defaultPreferences, lineNumbers: false, folding: false },
  });
  expect(parent.querySelector('.cm-lineNumbers')).toBeNull();
  expect(parent.querySelector('.cm-foldGutter')).toBeNull();
  editor.setPreferences(defaultPreferences);
  expect(parent.querySelector('.cm-lineNumbers')).not.toBeNull();
  expect(parent.querySelector('.cm-foldGutter')).not.toBeNull();
});

it('leaves out the suggestion kinds the user turned off', async () => {
  const catalog = {
    schemas: () => Promise.resolve([{ name: 'public', system: false }]),
    objects: () =>
      Promise.resolve([
        { schema: 'public', name: 'users', kind: 'table' as const },
      ]),
    objectsIn: () => Promise.resolve([]),
    columns: () => Promise.resolve([]),
  };
  const labels = async (
    change: Partial<EditorPreferences>,
  ): Promise<string[]> => {
    const state = stateOf('SELECT * FROM u', change);
    const result = await sqlCompletionSource(
      'postgres',
      catalog,
    )(new CompletionContext(state, state.doc.length, true));
    return result?.options.map((option) => option.label) ?? [];
  };
  expect(await labels({})).toContain('users');
  expect(await labels({ completeTables: false })).not.toContain('users');
  const snippets = snippetSource(() => [
    { name: 'Select', prefix: 'sel', body: 'SELECT ${1}' },
  ]);
  const typed = (state: EditorState): unknown =>
    snippets(new CompletionContext(state, state.doc.length, false));
  expect(typed(stateOf('sel'))).not.toBeNull();
  expect(typed(stateOf('sel', { completeSnippets: false }))).toBeNull();
});

it('inserts assistant SQL literally over the selection without snippet expansion', () => {
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, { doc: 'replace', label: 'SQL' });
  editor.view.dispatch({ selection: { anchor: 0, head: 7 } });
  const sql = "SELECT $1, '${literal}' FROM public.items";
  editor.insertText(sql);
  expect(editor.view.state.doc.toString()).toBe(sql);
  expect(editor.view.state.selection.main.head).toBe(sql.length);
});
