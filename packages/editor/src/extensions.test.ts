// SPDX-License-Identifier: Apache-2.0
import { CompletionContext } from '@codemirror/autocomplete';
import type {
  CompletionResult,
  CompletionSource,
} from '@codemirror/autocomplete';
import { syntaxTree } from '@codemirror/language';
import { EditorState } from '@codemirror/state';
import { highlightTree, tags } from '@lezer/highlight';
import type { Tag } from '@lezer/highlight';
import { expect, it } from 'vitest';
import { sqlEditorExtensions } from './extensions';
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
  const sources = state.languageDataAt<CompletionSource>(
    'autocomplete',
    position,
  );
  const results = await Promise.all(
    sources.map(async (source) =>
      source(new CompletionContext(state, position, true)),
    ),
  );
  const labels = results
    .filter((result): result is CompletionResult => result !== null)
    .flatMap((result) => result.options.map((option) => option.label));
  expect(labels.map((label) => label.toLowerCase())).toContain('ilike');
});

it('places a key mode ahead of the default extensions', () => {
  const marker = EditorState.languageData.of(() => []);
  const extensions = sqlEditorExtensions({
    doc: '',
    label: 'SQL',
    keyMode: marker,
  });
  expect(Array.isArray(extensions) && extensions[0]).toBe(marker);
});
