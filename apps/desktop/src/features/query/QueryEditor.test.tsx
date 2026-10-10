// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, waitFor } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { createCursorStore } from './cursor';
import { defaultEditorSettings } from './editorSettings';
import { QueryEditor } from './QueryEditor';
import type { EditorHandle } from './request';

it('mounts CodeMirror lazily, exposes text and cursor, and hands keys to the app first', async () => {
  const keys: string[] = [];
  const onKey = vi.fn((event: KeyboardEvent) => {
    if (event.key !== 'Enter' || !event.metaKey) return false;
    keys.push(event.shiftKey ? 'all' : 'current');
    return true;
  });
  const onChange = vi.fn();
  const ready: { handle: EditorHandle | null } = { handle: null };
  const current = (): EditorHandle | null => ready.handle;
  const { unmount } = render(
    <QueryEditor
      doc={'SELECT 1;\nSELECT 2;'}
      label="SQL editor"
      dialect="postgres"
      settings={defaultEditorSettings}
      catalog={null}
      cursor={createCursorStore()}
      onChange={onChange}
      onKey={onKey}
      onReady={(editor) => {
        ready.handle = editor;
      }}
    />,
  );
  // Loading CodeMirror is real module I/O, separate from the mounted UI assertion.
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  const editor = await screen.findByRole('textbox', { name: 'SQL editor' });
  await waitFor(() => {
    expect(current()?.text()).toBe('SELECT 1;\nSELECT 2;');
  });
  expect(current()?.selection()).toBe('');

  const press = (shiftKey: boolean): void => {
    editor.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: 'Enter',
        metaKey: true,
        shiftKey,
        bubbles: true,
        cancelable: true,
      }),
    );
  };
  press(false);
  press(true);
  expect(keys).toEqual(['current', 'all']);
  expect(current()?.text()).toBe('SELECT 1;\nSELECT 2;');
  unmount();
  expect(current()).toBeNull();
  expect(onChange).not.toHaveBeenCalled();
});
