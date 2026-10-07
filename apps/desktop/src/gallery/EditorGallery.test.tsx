// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { EditorGallery } from './EditorGallery';

it('lazily mounts a labelled SQL editor and remounts it with Vim', async () => {
  const { container, unmount } = render(<EditorGallery />);
  const editor = await screen.findByRole('textbox', {
    name: 'SQL editor specimen',
  });
  expect(editor.textContent).toContain('WITH recent AS');
  expect(container.querySelector('.cm-vimMode')).toBeNull();
  fireEvent.click(screen.getByRole('switch', { name: 'Vim mode' }));
  await waitFor(() => {
    expect(container.querySelector('.cm-vimMode')).not.toBeNull();
  });
  expect(container.querySelectorAll('.cm-editor')).toHaveLength(1);
  unmount();
  expect(container.querySelector('.cm-editor')).toBeNull();
});

it('keeps Escape inside the editor so the gallery stays open', async () => {
  const escape = vi.fn();
  window.addEventListener('keydown', escape);
  render(<EditorGallery />);
  const editor = await screen.findByRole('textbox', {
    name: 'SQL editor specimen',
  });
  fireEvent.keyDown(editor, { key: 'Escape' });
  fireEvent.keyDown(document.body, { key: 'Escape' });
  window.removeEventListener('keydown', escape);
  expect(escape).toHaveBeenCalledOnce();
});
