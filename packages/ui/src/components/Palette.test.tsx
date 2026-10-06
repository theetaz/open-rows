// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import axe from 'axe-core';
import { Palette } from './Palette';
const choose = vi.fn();
function Harness(): React.JSX.Element {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        type="button"
        onClick={() => {
          setOpen(true);
        }}
      >
        Open
      </button>
      {open && (
        <Palette
          title="Find object"
          items={[
            {
              id: 'orders',
              label: 'orders',
              detail: 'Tables · public · 2.4M',
              glyph: '▦',
            },
            {
              id: 'customers',
              label: 'customers',
              detail: 'Tables · public · 412K',
              glyph: '▦',
            },
          ]}
          footer={
            <label>
              <input type="checkbox" />
              System
            </label>
          }
          onClose={() => {
            setOpen(false);
          }}
          onChoose={(id) => {
            choose(id);
            setOpen(false);
          }}
        />
      )}
    </>
  );
}
function mount(): HTMLInputElement {
  vi.useFakeTimers();
  render(<Harness />);
  const opener = screen.getByRole('button', { name: 'Open' });
  opener.focus();
  fireEvent.click(opener);
  return screen.getByRole('combobox');
}
function finish(milliseconds = 160): void {
  act(() => {
    vi.advanceTimersByTime(milliseconds);
  });
}
afterEach(() => {
  vi.useRealTimers();
});
it('supports combobox navigation, selection and retains the closing state for 160 ms', () => {
  const input = mount();
  expect(input).toHaveFocus();
  expect(input).toHaveAttribute('aria-expanded', 'true');
  expect(input).toHaveAttribute(
    'aria-controls',
    screen.getByRole('listbox').id,
  );
  fireEvent.keyDown(input, { key: 'ArrowUp' });
  expect(screen.getByRole('option', { name: /customers/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  fireEvent.keyDown(input, { key: 'ArrowDown' });
  expect(input).toHaveAttribute(
    'aria-activedescendant',
    screen.getByRole('option', { name: /orders/ }).id,
  );
  fireEvent.keyDown(input, { key: 'End' });
  fireEvent.keyDown(input, { key: 'Home' });
  fireEvent.keyDown(input, { key: 'ArrowDown' });
  fireEvent.keyDown(input, { key: 'Enter' });
  expect(screen.getByRole('dialog')).toHaveAttribute('data-state', 'closing');
  finish(159);
  expect(choose).not.toHaveBeenCalled();
  finish(1);
  expect(choose).toHaveBeenCalledWith('customers');
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(screen.getByRole('button', { name: 'Open' })).toHaveFocus();
});
it('filters/highlights, handles empty results and ignores IME navigation/selection', () => {
  const input = mount();
  fireEvent.change(input, { target: { value: 'odr' } });
  expect(screen.getAllByRole('option')).toHaveLength(1);
  expect(screen.getByRole('option').querySelectorAll('mark')).toHaveLength(3);
  fireEvent.keyDown(input, { key: 'Enter', isComposing: true });
  expect(screen.getByRole('dialog')).toHaveAttribute('data-state', 'open');
  fireEvent.change(input, { target: { value: 'xyz' } });
  expect(screen.getByRole('status')).toHaveTextContent('No matches');
  expect(input).not.toHaveAttribute('aria-activedescendant');
  fireEvent.keyDown(input, { key: 'ArrowDown' });
  fireEvent.keyDown(input, { key: 'Enter' });
  expect(choose).not.toHaveBeenCalled();
  fireEvent.keyDown(input, { key: 'Escape' });
  finish();
  expect(screen.getByRole('button', { name: 'Open' })).toHaveFocus();
  expect(screen.getByRole('button', { name: 'Open' })).not.toHaveAttribute(
    'inert',
  );
});
it('traps both Tab directions with a footer control and supports click choice', () => {
  mount();
  const first = screen.getByRole('button', { name: 'Close palette' });
  const last = screen.getByRole('checkbox');
  last.focus();
  fireEvent.keyDown(last, { key: 'Tab' });
  expect(first).toHaveFocus();
  fireEvent.keyDown(first, { key: 'Tab', shiftKey: true });
  expect(last).toHaveFocus();
  fireEvent.click(screen.getByRole('option', { name: /orders/ }));
  finish();
  expect(choose).toHaveBeenCalledWith('orders');
});
it('uses the reduced-motion state and dismisses with zero animation delay', () => {
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  );
  const input = mount();
  expect(screen.getByRole('dialog')).toHaveAttribute('data-motion', 'reduced');
  fireEvent.keyDown(input, { key: 'Escape' });
  finish(0);
  expect(screen.queryByRole('dialog')).toBeNull();
});
it('has no axe violations for dialog, combobox, listbox and footer semantics', async () => {
  render(<Harness />);
  fireEvent.click(screen.getByRole('button', { name: 'Open' }));
  const dialog = screen.getByRole('dialog');
  expect(within(dialog).getAllByRole('option')).toHaveLength(2);
  const result = await axe.run(dialog, {
    rules: { 'color-contrast': { enabled: false } },
  });
  expect(result.violations).toEqual([]);
});
