// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { Picker } from './Picker';
import { pickerRows, splitMatch } from './pickerModel';
import type { PickerSection } from './pickerModel';

const sections: readonly PickerSection[] = [
  { head: 'Recent', items: [{ id: 'orders', name: 'orders' }] },
  {
    head: 'All tables',
    items: [
      { id: 'agents', name: 'agents', status: 'on' },
      { id: 'orders-2', name: 'order_items', current: true },
      { id: 'users', name: 'users', sub: 'public' },
    ],
  },
];

function mount(): {
  onPick: ReturnType<typeof vi.fn>;
  onStep: ReturnType<typeof vi.fn>;
  input: HTMLElement;
} {
  const anchor = document.createElement('button');
  document.body.append(anchor);
  const onPick = vi.fn();
  const onStep = vi.fn();
  render(
    <Picker
      anchor={anchor}
      placeholder="Find a table"
      sections={sections}
      onPick={onPick}
      onStep={onStep}
      onClose={vi.fn()}
    />,
  );
  return { onPick, onStep, input: screen.getByRole('combobox') };
}

it('filters every section, keeps headings and highlights the match', () => {
  const { rows, total } = pickerRows(sections, 'ORD');
  expect(rows.map((row) => row.item.id)).toEqual(['orders', 'orders-2']);
  expect(rows.map((row) => row.head)).toEqual(['Recent', 'All tables']);
  expect(total).toBe(2);
  expect(splitMatch('order_items', 'item')).toEqual(['order_', 'item', 's']);
});

it('opens on the current item, focused, and picks with ↩ or ⌘↩', () => {
  const { onPick, input } = mount();
  expect(input).toHaveFocus();
  expect(screen.getByRole('option', { name: /order_items/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  fireEvent.keyDown(input, { key: 'ArrowDown' });
  fireEvent.keyDown(input, { key: 'Enter', metaKey: true });
  expect(onPick).toHaveBeenCalledWith(
    expect.objectContaining({ id: 'users' }),
    { newTab: true },
  );
});

it('steps to the neighbouring segment with ←→ only while the search is empty', () => {
  const { onStep, input } = mount();
  fireEvent.keyDown(input, { key: 'ArrowLeft' });
  expect(onStep).toHaveBeenCalledWith(-1);
  fireEvent.change(input, { target: { value: 'zz' } });
  fireEvent.keyDown(input, { key: 'ArrowRight' });
  expect(onStep).toHaveBeenCalledOnce();
  expect(screen.getByText('No matches for “zz”')).toBeInTheDocument();
});
