// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { Sheet } from './Sheet';

function renderSheet(): { dialog: HTMLElement; title: HTMLElement } {
  render(
    <Sheet title="Open database" onClose={() => undefined}>
      <p>Body</p>
    </Sheet>,
  );
  const dialog = screen.getByRole('dialog', { hidden: true });
  dialog.getBoundingClientRect = () => new DOMRect(300, 200, 400, 300);
  return { dialog, title: screen.getByText('Open database') };
}

it('opens without a drag offset and moves with the title row', () => {
  const { dialog, title } = renderSheet();
  expect(dialog.style.translate).toBe('');
  fireEvent.pointerDown(title, { button: 0, clientX: 500, clientY: 210 });
  fireEvent.pointerMove(window, { clientX: 450, clientY: 260 });
  expect(dialog.style.translate).toBe('-50px 50px');
  fireEvent.pointerUp(window);
  fireEvent.pointerMove(window, { clientX: 0, clientY: 0 });
  expect(dialog.style.translate).toBe('-50px 50px');
});

it('keeps the sheet inside the window', () => {
  const { dialog, title } = renderSheet();
  fireEvent.pointerDown(title, { button: 0, clientX: 500, clientY: 210 });
  fireEvent.pointerMove(window, { clientX: -2000, clientY: -2000 });
  expect(dialog.style.translate).toBe('-300px -200px');
});
