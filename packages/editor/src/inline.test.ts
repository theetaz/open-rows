// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { undo } from '@codemirror/commands';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { mountSqlEditor, parseFully } from './index';
import type { EditorDialect, InlineGenerate, SqlEditor } from './index';
import { inlineRange } from './inline';
let editor: SqlEditor | undefined;
afterEach(() => {
  editor?.destroy();
  editor = undefined;
  document.body.replaceChildren();
});
function mount(
  doc: string,
  generate: InlineGenerate,
  dialect: EditorDialect = 'postgres',
): SqlEditor {
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, {
    doc,
    label: 'SQL',
    dialect,
    onInline: { generate },
  });
  parseFully(editor.view, 1000);
  return editor;
}
function open(): void {
  if (!editor) throw new Error('Editor missing');
  fireEvent.keyDown(editor.view.contentDOM, { key: 'i', metaKey: true });
}
async function prompt(): Promise<void> {
  const input = await screen.findByRole('textbox', {
    name: 'Inline SQL prompt',
  });
  fireEvent.input(input, { target: { value: 'Make it better' } });
  fireEvent.click(screen.getByRole('button', { name: 'Generate proposal' }));
}
it('uses the selection and accepts literal SQL as one undoable edit, without execution', async () => {
  const generate = vi.fn<InlineGenerate>().mockImplementation(({ delta }) => {
    delta('streaming');
    return Promise.resolve('SELECT $1, \u0024{literal};');
  });
  const { view } = mount('SELECT 1;\nSELECT 2;', generate);
  view.dispatch({ selection: { anchor: 10, head: 19 } });
  open();
  await prompt();
  const accept = await screen.findByRole('button', { name: 'Accept proposal' });
  expect(generate.mock.calls[0]?.[0].sql).toBe('SELECT 2;');
  expect(view.state.doc.toString()).toBe('SELECT 1;\nSELECT 2;');
  expect(view.dom.querySelector('.cm-inline-original')).not.toBeNull();
  fireEvent.click(accept);
  expect(view.state.doc.toString()).toBe(
    'SELECT 1;\nSELECT $1, \u0024{literal};',
  );
  undo(view);
  expect(view.state.doc.toString()).toBe('SELECT 1;\nSELECT 2;');
});
it('Escape cancels the stream and ignores late proposals', async () => {
  let resolve: (sql: string) => void = () => undefined;
  const generate = vi.fn<InlineGenerate>().mockImplementation(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const { view } = mount('SELECT 1;', generate);
  open();
  await prompt();
  fireEvent.keyDown(view.contentDOM, { key: 'Escape' });
  expect(generate.mock.calls[0]?.[0].signal.aborted).toBe(true);
  resolve('DELETE FROM items;');
  await Promise.resolve();
  expect(screen.queryByRole('button', { name: 'Accept proposal' })).toBeNull();
  expect(view.state.doc.toString()).toBe('SELECT 1;');
});
it('editing or destroying the editor cancels and invalidates its pending proposal', async () => {
  const generate = vi
    .fn<InlineGenerate>()
    .mockImplementation(() => new Promise(() => {}));
  const { view } = mount('SELECT 1;', generate);
  open();
  await prompt();
  view.dispatch({ changes: { from: 0, insert: '-- changed\n' } });
  expect(generate.mock.calls[0]?.[0].signal.aborted).toBe(true);
  expect(screen.queryByRole('button', { name: 'Accept proposal' })).toBeNull();
  open();
  await prompt();
  editor?.destroy();
  editor = undefined;
  expect(generate.mock.calls[1]?.[0].signal.aborted).toBe(true);
});
it('rejects without changing the document and supports an empty insert point', async () => {
  const { view } = mount('', () => Promise.resolve('SELECT 1;'));
  expect(inlineRange(view)).toEqual({ from: 0, to: 0 });
  open();
  await prompt();
  await screen.findByRole('button', { name: 'Accept proposal' });
  fireEvent.click(screen.getByRole('button', { name: 'Reject proposal' }));
  expect(view.state.doc.toString()).toBe('');
  const escaped = vi.fn();
  window.addEventListener('keydown', escaped, { once: true });
  fireEvent.keyDown(view.contentDOM, { key: 'Escape' });
  expect(escaped).toHaveBeenCalled();
});
it.each([
  'postgres',
  'mysql',
  'mariadb',
  'sqlite',
  'sqlserver',
  'clickhouse',
] as EditorDialect[])(
  'selects the current statement in %s',
  async (dialect) => {
    const { view } = mount(
      'SELECT 1;\nSELECT 2;\n',
      () => Promise.resolve('SELECT 3;'),
      dialect,
    );
    view.dispatch({ selection: { anchor: 13 } });
    expect(
      view.state.sliceDoc(inlineRange(view).from, inlineRange(view).to),
    ).toBe('SELECT 2;');
    view.dispatch({ selection: { anchor: view.state.doc.length } });
    expect(inlineRange(view)).toEqual({
      from: view.state.doc.length,
      to: view.state.doc.length,
    });
    open();
    await waitFor(() => {
      expect(
        screen.getByRole('textbox', { name: 'Inline SQL prompt' }),
      ).toBeTruthy();
    });
  },
);

it('opens from menu actions and read-only editors cannot offer proposals', async () => {
  const mounted = mount('SELECT 1;', () => Promise.resolve('SELECT 2;'));
  mounted.promptInline();
  await screen.findByRole('textbox', { name: 'Inline SQL prompt' });
  mounted.destroy();
  editor = undefined;
  document.body.replaceChildren();
  const parent = document.createElement('div');
  document.body.append(parent);
  editor = mountSqlEditor(parent, {
    doc: 'SELECT 1;',
    label: 'SQL',
    readOnly: true,
    onInline: { generate: () => Promise.resolve('DELETE FROM items;') },
  });
  editor.promptInline();
  expect(
    screen.queryByRole('textbox', { name: 'Inline SQL prompt' }),
  ).toBeNull();
});

it('app shortcut overrides retain precedence over the default inline key', () => {
  const parent = document.createElement('div');
  document.body.append(parent);
  const key = vi.fn(() => true);
  editor = mountSqlEditor(parent, {
    doc: '',
    label: 'SQL',
    onKeyDown: key,
    onInline: { generate: () => Promise.resolve('SELECT 1;') },
  });
  open();
  expect(key).toHaveBeenCalled();
  expect(
    screen.queryByRole('textbox', { name: 'Inline SQL prompt' }),
  ).toBeNull();
});

it('accepts at an empty caret without modifying preceding SQL', async () => {
  const generate = vi.fn<InlineGenerate>().mockResolvedValue('SELECT 2;');
  const { view } = mount('SELECT 1;\n', generate);
  view.dispatch({ selection: { anchor: view.state.doc.length } });
  open();
  await prompt();
  fireEvent.click(
    await screen.findByRole('button', { name: 'Accept proposal' }),
  );
  expect(generate.mock.calls[0]?.[0].sql).toBe('');
  expect(view.state.doc.toString()).toBe('SELECT 1;\nSELECT 2;');
  undo(view);
  expect(view.state.doc.toString()).toBe('SELECT 1;\n');
});

it.each(['', 'x'.repeat(16385)])(
  'refuses empty or oversized proposals',
  async (proposal) => {
    const { view } = mount('SELECT 1;', () => Promise.resolve(proposal));
    open();
    await prompt();
    await screen.findByRole('alert');
    expect(
      screen.queryByRole('button', { name: 'Accept proposal' }),
    ).toBeNull();
    expect(view.state.doc.toString()).toBe('SELECT 1;');
  },
);

it('shows the setup reason and action instead of a prompt while blocked', async () => {
  const parent = document.createElement('div');
  document.body.append(parent);
  const setup = vi.fn();
  const generate = vi.fn<InlineGenerate>();
  editor = mountSqlEditor(parent, {
    doc: 'SELECT 1;',
    label: 'SQL',
    onInline: {
      generate,
      blocked: () => ({
        reason: 'No AI provider is set up.',
        action: { label: 'Open AI settings', run: setup },
      }),
    },
  });
  editor.promptInline();
  expect((await screen.findByRole('alert')).textContent).toBe(
    'No AI provider is set up.',
  );
  expect(
    screen.queryByRole('textbox', { name: 'Inline SQL prompt' }),
  ).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: 'Open AI settings' }));
  expect(setup).toHaveBeenCalledOnce();
  expect(generate).not.toHaveBeenCalled();
  expect(screen.queryByRole('alert')).toBeNull();
  expect(editor.view.state.doc.toString()).toBe('SELECT 1;');
});

it('keeps an accepted proposal separate from preceding typing in undo history', async () => {
  const { view } = mount('SELECT 1;', () => Promise.resolve('SELECT 2;'));
  view.dispatch({
    changes: { from: 9, insert: ' ' },
    selection: { anchor: 0, head: 9 },
    userEvent: 'input.type',
  });
  open();
  await prompt();
  fireEvent.click(
    await screen.findByRole('button', { name: 'Accept proposal' }),
  );
  expect(view.state.doc.toString()).toBe('SELECT 2; ');
  undo(view);
  expect(view.state.doc.toString()).toBe('SELECT 1; ');
});

it('marks the original range as removed while a proposal streams', async () => {
  let resolve: (sql: string) => void = () => undefined;
  const { view } = mount('SELECT 1;', ({ delta }) => {
    delta('SELECT');
    return new Promise((done) => {
      resolve = done;
    });
  });
  open();
  await prompt();
  expect((await screen.findByLabelText('Proposed SQL')).textContent).toBe(
    'SELECT',
  );
  expect(view.dom.querySelector('.cm-inline-original')?.textContent).toBe(
    'SELECT 1;',
  );
  fireEvent.click(screen.getByRole('button', { name: 'Cancel proposal' }));
  resolve('SELECT 2;');
  await Promise.resolve();
  expect(view.dom.querySelector('.cm-inline-original')).toBeNull();
  expect(view.state.doc.toString()).toBe('SELECT 1;');
});

it('targets the statement whose semicolon ends the caret line, not a blank line', () => {
  const { view } = mount('SELECT 1;\nSELECT 2;  \n\n', () =>
    Promise.resolve('SELECT 3;'),
  );
  for (const anchor of [19, 21]) {
    view.dispatch({ selection: { anchor } });
    const range = inlineRange(view);
    expect(view.state.sliceDoc(range.from, range.to)).toBe('SELECT 2;');
  }
  view.dispatch({ selection: { anchor: 22 } });
  expect(inlineRange(view)).toEqual({ from: 22, to: 22 });
});
