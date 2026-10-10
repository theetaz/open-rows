// SPDX-License-Identifier: Apache-2.0
import {
  acceptCompletion,
  currentCompletions,
  startCompletion,
} from '@codemirror/autocomplete';
import { syntaxTreeAvailable } from '@codemirror/language';
import { afterEach, expect, it, vi } from 'vitest';
import { defaultPreferences, mountSqlEditor, parseFully } from './index';
import type { SqlEditor } from './index';
import { vimMode, vimModeOf } from './vim';

let editor: SqlEditor | undefined;
afterEach(() => {
  editor?.destroy();
  editor = undefined;
  document.body.replaceChildren();
  vi.useRealTimers();
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

it('runs application keys before the default Mod-Enter binding', () => {
  const parent = document.createElement('div');
  document.body.append(parent);
  const runs: string[] = [];
  editor = mountSqlEditor(parent, {
    doc: 'SELECT 1;',
    label: 'SQL',
    onKeyDown: (event) => {
      if (event.key !== 'Enter' || !event.ctrlKey) return false;
      runs.push('current');
      return true;
    },
  });

  const event = new KeyboardEvent('keydown', {
    key: 'Enter',
    metaKey: false,
    ctrlKey: true,
    bubbles: true,
    cancelable: true,
  });
  editor.view.contentDOM.dispatchEvent(event);
  expect(runs).toEqual(['current']);
  expect(event.defaultPrevented).toBe(true);
  expect(editor.view.state.doc.toString()).toBe('SELECT 1;');
});

it('read-only viewers keep their text and are not editable', () => {
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, {
    doc: 'CREATE TABLE t ();',
    label: 'DDL',
    readOnly: true,
  });
  const { view } = editor;
  expect(view.state.readOnly).toBe(true);
  expect(view.contentDOM.getAttribute('contenteditable')).toBe('false');
  expect(view.state.doc.toString()).toBe('CREATE TABLE t ();');
});

it('shows completions with kind icons and muted details', async () => {
  vi.useFakeTimers();
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, {
    doc: 'SELECT * FROM us',
    label: 'SQL',
    catalog: {
      schemas: () => Promise.resolve([{ name: 'public', system: false }]),
      objects: () =>
        Promise.resolve([{ schema: 'public', name: 'users', kind: 'table' }]),
      objectsIn: () => Promise.resolve([]),
      columns: () => Promise.resolve([]),
    },
  });
  const { view } = editor;
  view.focus();
  view.dispatch({ selection: { anchor: view.state.doc.length } });
  startCompletion(view);
  // CodeMirror debounces explicit completion by 50 ms; resolve the catalog too.
  await vi.advanceTimersByTimeAsync(50);
  expect(currentCompletions(view.state).length).toBeGreaterThan(0);
  const row = document.querySelector('.cm-tooltip-autocomplete li');
  if (!row) throw new Error('no completion row');
  expect(row.querySelector('svg.cm-sqlIcon-table')).not.toBeNull();
  expect(row.querySelector('.cm-completionLabel')?.textContent).toBe('users');
  expect(row.querySelector('.cm-completionDetail')?.textContent).toBe('public');
  // Acceptance is ignored for 75 ms after opening, independent of machine load.
  expect(acceptCompletion(view)).toBe(false);
  await vi.advanceTimersByTimeAsync(75);
  expect(acceptCompletion(view)).toBe(true);
  expect(view.state.doc.toString()).toBe('SELECT * FROM users');
});

it('switches preferences and key modes without losing the text', () => {
  const { view, setPreferences, setKeyMode } = mount('SELECT 1;\nSELECT 2;');
  setPreferences({
    ...defaultPreferences,
    wrap: true,
    highlightQuery: true,
    fontSize: 16,
  });
  expect(view.contentDOM.classList.contains('cm-lineWrapping')).toBe(true);
  expect(view.dom.querySelectorAll('.cm-currentQuery')).toHaveLength(1);
  expect(view.dom.querySelector('.cm-activeLine')).toBeNull();
  setKeyMode(vimMode());
  expect(vimModeOf(view)).toBe('normal');
  setKeyMode(null);
  expect(vimModeOf(view)).toBeNull();
  expect(view.state.doc.toString()).toBe('SELECT 1;\nSELECT 2;');
});
