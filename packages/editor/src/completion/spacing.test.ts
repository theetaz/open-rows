// SPDX-License-Identifier: Apache-2.0
import { pickedCompletion } from '@codemirror/autocomplete';
import { EditorState } from '@codemirror/state';
import { expect, it } from 'vitest';
import { spaceAfterCompletion } from './spacing';

function accept(doc: string, insert: string, type: string): EditorState {
  const state = EditorState.create({
    doc,
    selection: { anchor: doc.length },
    extensions: spaceAfterCompletion,
  });
  return state.update({
    changes: { from: doc.length, insert },
    selection: { anchor: doc.length + insert.length },
    annotations: pickedCompletion.of({ label: insert, type }),
  }).state;
}

it('types a space after an accepted keyword, table or column', () => {
  for (const type of ['keyword', 'table', 'column']) {
    const state = accept('SEL', 'ECT', type);
    expect(state.doc.toString()).toBe('SELECT ');
    expect(state.selection.main.head).toBe(7);
  }
});

it('adds no space after a schema or function, or before an existing separator', () => {
  expect(accept('pub', 'lic', 'schema').doc.toString()).toBe('public');
  expect(accept('cou', 'nt', 'function').doc.toString()).toBe('count');
  const state = EditorState.create({
    doc: 'SEL FROM t',
    selection: { anchor: 3 },
    extensions: spaceAfterCompletion,
  }).update({
    changes: { from: 3, insert: 'ECT' },
    selection: { anchor: 6 },
    annotations: pickedCompletion.of({ label: 'SELECT', type: 'keyword' }),
  }).state;
  expect(state.doc.toString()).toBe('SELECT FROM t');
});

it('leaves ordinary typing alone', () => {
  const state = EditorState.create({
    doc: 'SEL',
    selection: { anchor: 3 },
    extensions: spaceAfterCompletion,
  }).update({
    changes: { from: 3, insert: 'E' },
    selection: { anchor: 4 },
  }).state;
  expect(state.doc.toString()).toBe('SELE');
});
