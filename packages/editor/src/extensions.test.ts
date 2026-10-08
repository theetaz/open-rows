// SPDX-License-Identifier: Apache-2.0
import { CompletionContext } from '@codemirror/autocomplete';
import { syntaxTree } from '@codemirror/language';
import { EditorState } from '@codemirror/state';
import { highlightTree, tags } from '@lezer/highlight';
import type { Tag } from '@lezer/highlight';
import { expect, it } from 'vitest';
import { sqlCompletionSource } from './completion/source';
import { sqlEditorExtensions } from './extensions';
import { keyModeSlot } from './preferences';
import { sqlHighlightStyle } from './theme';

function stateOf(doc: string): EditorState {
  return EditorState.create({
    doc,
    extensions: sqlEditorExtensions({ doc, label: 'SQL' }),
  });
}

function classAt(state: EditorState, text: string): string {
  const from = state.doc.toString().indexOf(text);
  let found = '';
  highlightTree(syntaxTree(state), sqlHighlightStyle, (start, end, classes) => {
    if (start <= from && from + text.length <= end) found = classes;
  });
  return found;
}

function classFor(tag: Tag): string {
  return sqlHighlightStyle.style([tag]) ?? 'missing';
}

it('highlights PostgreSQL dollar quoting, casts and comments', () => {
  const state = stateOf(
    "-- note\nSELECT $$it's raw$$::text, E'a\\'b', 42 FROM t;",
  );
  expect(classAt(state, 'SELECT')).toBe(classFor(tags.keyword));
  expect(classAt(state, "$$it's raw$$")).toBe(classFor(tags.string));
  expect(classAt(state, 'text')).toBe(classFor(tags.typeName));
  expect(classAt(state, '-- note')).toBe(classFor(tags.lineComment));
  expect(classAt(state, '42')).toBe(classFor(tags.number));
});

it('offers PostgreSQL keyword completion while typing', async () => {
  const state = stateOf('SELECT * FROM t WHERE name IL');
  const position = state.doc.length;
  const result = await sqlCompletionSource(
    'postgres',
    null,
  )(new CompletionContext(state, position, false));
  const labels = result?.options.map((option) => option.label) ?? [];
  expect(labels).toContain('ILIKE');
});

it('places a key mode ahead of the default extensions', () => {
  const marker = EditorState.languageData.of(() => []);
  const extensions = sqlEditorExtensions({
    doc: '',
    label: 'SQL',
    keyMode: marker,
  });
  const state = EditorState.create({ extensions });
  expect(keyModeSlot.get(state)).toBe(marker);
  expect(Array.isArray(extensions) && extensions[0]).toEqual(
    keyModeSlot.of(marker),
  );
});
