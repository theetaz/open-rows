// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { QueryEditor } from './QueryEditor';
import type { EditorHandle } from './request';

it('mounts CodeMirror lazily, exposes text and cursor, and runs on ⌘↵ / ⇧⌘↵', async () => {
  const onRun = vi.fn();
  const onChange = vi.fn();
  const ready: { handle: EditorHandle | null } = { handle: null };
  const current = (): EditorHandle | null => ready.handle;
  const { unmount } = render(
    <QueryEditor
      doc={'SELECT 1;\nSELECT 2;'}
      label="SQL editor"
      dialect="postgres"
      onChange={onChange}
      onRun={onRun}
      onReady={(editor) => {
        ready.handle = editor;
      }}
    />,
  );
  const editor = await screen.findByRole('textbox', { name: 'SQL editor' });
  await waitFor(() => {
    expect(current()?.text()).toBe('SELECT 1;\nSELECT 2;');
  });
  expect(current()?.selection()).toBe('');

  const press = (shiftKey: boolean): void => {
    editor.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: 'Enter',
        metaKey: false,
        ctrlKey: true,
        shiftKey,
        bubbles: true,
        cancelable: true,
      }),
    );
  };
  press(false);
  press(true);
  expect(onRun.mock.calls).toEqual([['current'], ['all']]);
  unmount();
  expect(current()).toBeNull();
  expect(onChange).not.toHaveBeenCalled();
});
