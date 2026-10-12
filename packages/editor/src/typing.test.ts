// SPDX-License-Identifier: Apache-2.0
import { acceptCompletion } from '@codemirror/autocomplete';
import { indentMore } from '@codemirror/commands';
import { EditorState } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import { afterEach, expect, it } from 'vitest';
import { sqlEditorExtensions } from './extensions';
import {
  defaultPreferences,
  preferenceExtensions,
  preferenceSlot,
} from './preferences';
import type { EditorPreferences } from './preferences';

let view: EditorView | undefined;
afterEach(() => {
  view?.destroy();
  view = undefined;
});

function mount(doc: string, change: Partial<EditorPreferences>): EditorView {
  view?.destroy();
  view = new EditorView({
    state: EditorState.create({
      doc,
      extensions: sqlEditorExtensions({
        doc,
        label: 'SQL',
        preferences: { ...defaultPreferences, ...change },
      }),
    }),
  });
  return view;
}

/** Commands bound to `key` across every keymap of the state. */
function bound(state: EditorState, key: string): unknown[] {
  return state
    .facet(keymap)
    .flat()
    .filter((binding) => binding.key === key)
    .map((binding) => binding.run);
}

it('indents with the configured width or with tabs', () => {
  const spaces = mount('SELECT 1', { indentWidth: 4 });
  indentMore(spaces);
  expect(spaces.state.doc.toString()).toBe('    SELECT 1');
  expect(spaces.state.tabSize).toBe(4);
  const tabs = mount('SELECT 1', { indentWith: 'tabs' });
  indentMore(tabs);
  expect(tabs.state.doc.toString()).toBe('\tSELECT 1');
});

it('binds Enter, Tab or both to accept a suggestion', () => {
  const accepts = (acceptKey: EditorPreferences['acceptKey']): string[] => {
    const { state } = mount('', { acceptKey });
    return ['Enter', 'Tab'].filter((key) =>
      bound(state, key).includes(acceptCompletion),
    );
  };
  expect(accepts('enter')).toEqual(['Enter']);
  expect(accepts('tab')).toEqual(['Tab']);
  expect(accepts('both')).toEqual(['Enter', 'Tab']);
});

it('reconfigures indentation and closing delimiters on an existing editor', () => {
  const editor = mount('SELECT 1', { closeDelimiters: true });
  editor.dispatch({ selection: { anchor: 8 } });
  const handlers = editor.state.facet(EditorView.inputHandler);
  const insert = () =>
    editor.state.update({ changes: { from: 8, insert: '(' } });
  expect(handlers.some((handle) => handle(editor, 8, 8, '(', insert))).toBe(
    true,
  );
  expect(editor.state.doc.toString()).toBe('SELECT 1()');
  editor.dispatch({
    effects: preferenceSlot.reconfigure(
      preferenceExtensions({
        ...defaultPreferences,
        indentWidth: 4,
        indentWith: 'tabs',
        closeDelimiters: false,
      }),
    ),
  });
  expect(editor.state.facet(EditorView.inputHandler)).toHaveLength(0);
  expect(editor.state.tabSize).toBe(4);
  indentMore(editor);
  expect(editor.state.doc.toString()).toBe('\tSELECT 1()');
});
