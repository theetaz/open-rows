// SPDX-License-Identifier: Apache-2.0
import { syntaxTreeAvailable } from '@codemirror/language';
import { afterEach, expect, it } from 'vitest';
import { mountSqlEditor, parseFully } from './index';
import type { SqlEditor } from './index';
import { vimMode, vimModeOf } from './vim';

let editor: SqlEditor | undefined;
afterEach(() => {
  editor?.destroy();
  editor = undefined;
  document.body.replaceChildren();
});

function mount(doc: string, vim = false): SqlEditor {
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, {
    doc,
    label: 'SQL',
    ...(vim ? { keyMode: vimMode() } : {}),
  });
  return editor;
}

function press(target: HTMLElement, key: string): void {
  target.dispatchEvent(
    new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }),
  );
}

it('mounts, edits and removes its DOM on destroy', () => {
  const { view } = mount('SELECT 1;\nSELECT 2;');
  expect(view.state.doc.lines).toBe(2);
  expect(view.contentDOM.getAttribute('aria-label')).toBe('SQL');
  view.dispatch({ changes: { from: 0, insert: '-- x\n' } });
  expect(view.state.doc.line(1).text).toBe('-- x');
  editor?.destroy();
  editor = undefined;
  expect(document.querySelector('.cm-editor')).toBeNull();
});

it('notifies document changes synchronously but not selection moves', () => {
  const parent = document.createElement('div');
  document.body.append(parent);
  const changes: number[] = [];
  editor = mountSqlEditor(parent, {
    doc: 'SELECT 1;',
    label: 'SQL',
    onDocChange: () => changes.push(editor?.view.state.doc.length ?? -1),
  });
  editor.view.dispatch({ selection: { anchor: 3 } });
  expect(changes).toEqual([]);
  editor.view.dispatch({ changes: { from: 9, insert: ' ' } });
  expect(changes).toEqual([10]);
});

it('parses a large document fully on request', () => {
  const doc = Array.from(
    { length: 5_000 },
    (_, line) => `SELECT ${String(line)} AS value; -- row`,
  ).join('\n');
  const { view } = mount(doc);
  expect(parseFully(view, 5_000)).toBe(true);
  expect(syntaxTreeAvailable(view.state, view.state.doc.length)).toBe(true);
});

it('reports no Vim mode when Vim is not installed', () => {
  expect(vimModeOf(mount('SELECT 1;').view)).toBeNull();
});

it('drives Vim normal and insert modes through key events', () => {
  const { view } = mount('SELECT 1;', true);
  view.focus();
  expect(vimModeOf(view)).toBe('normal');
  press(view.contentDOM, 'x');
  expect(view.state.doc.toString()).toBe('ELECT 1;');
  press(view.contentDOM, 'i');
  expect(vimModeOf(view)).toBe('insert');
  press(view.contentDOM, 'Escape');
  expect(vimModeOf(view)).toBe('normal');
});
