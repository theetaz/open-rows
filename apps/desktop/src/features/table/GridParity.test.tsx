// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { QuickFilter } from './filters/QuickFilter';
import { quickOperators } from './filters/quickFilters';
import { TemporalPicker } from './inline/TemporalPicker';
import { TableSearch } from './TableSearch';
import type { TableTabState } from './useTableTab';
import { filterActions } from './filters/filterActions';
import { initialView } from './model';

it('applies a header quick filter with Return and hides the value for IS NULL', async () => {
  const user = userEvent.setup();
  const apply = vi.fn();
  const close = vi.fn();
  render(
    <QuickFilter
      name="email"
      x={10}
      y={10}
      operators={quickOperators('text', 'postgres')}
      onApply={apply}
      onClose={close}
    />,
  );
  const dialog = screen.getByRole('dialog', { name: 'Filter email' });
  expect(screen.getByRole('combobox', { name: 'Operator' })).toHaveValue(
    'contains',
  );
  await user.type(
    screen.getByRole('textbox', { name: 'Value of email' }),
    'ann{Enter}',
  );
  expect(apply).toHaveBeenCalledWith('contains', 'ann');
  expect(close).toHaveBeenCalled();
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Operator' }),
    'isNull',
  );
  expect(screen.queryByRole('textbox')).toBeNull();
  await user.keyboard('{Escape}');
  expect(dialog).toBeInTheDocument();
  expect(close).toHaveBeenCalledTimes(2);
});

it('offers typed operators without BETWEEN or raw SQL in quick filters', () => {
  const values = quickOperators('integer', 'postgres').map(
    (item) => item.value,
  );
  expect(values).toContain('greater');
  expect(values).not.toContain('between');
  expect(values).not.toContain('raw');
});

function searchState(search = vi.fn()) {
  return {
    view: { search: '' },
    actions: { search },
  } as unknown as TableTabState;
}

it('searches every shown column from the footer and clears with Escape', async () => {
  const user = userEvent.setup();
  const search = vi.fn();
  render(<TableSearch state={searchState(search)} />);
  const field = screen.getByRole('textbox', { name: 'Search all columns' });
  await user.type(field, 'ann{Enter}');
  expect(search).toHaveBeenLastCalledWith('ann');
  await user.keyboard('{Escape}');
  expect(search).toHaveBeenLastCalledWith('');
  expect(field).toHaveValue('');
});

it('fills a timestamp from the system date picker, keeping the zone', () => {
  const pick = vi.fn();
  const { container } = render(
    <TemporalPicker
      kind="datetime"
      value="2024-05-01 13:45:00+02"
      name="created_at"
      onPick={pick}
    />,
  );
  fireEvent.click(
    screen.getByRole('button', { name: 'Pick date for created_at' }),
  );
  const native = container.querySelector('input[type="datetime-local"]');
  if (!(native instanceof HTMLInputElement)) throw new Error('no picker');
  expect(native.value).toBe('2024-05-01T13:45');
  fireEvent.change(native, { target: { value: '2024-06-02T08:30' } });
  expect(pick).toHaveBeenCalledWith('2024-06-02 08:30:00+02');
});

it('leaves unchanged picker values as unstaged text', () => {
  const pick = vi.fn();
  render(
    <TemporalPicker
      kind="datetime"
      value="2024-05-01 13:45:10.123456+05:30"
      name="created_at"
      onPick={pick}
    />,
  );
  fireEvent.change(screen.getByLabelText('Choose datetime for created_at'), {
    target: { value: '2024-05-01T13:45:10.123' },
  });
  expect(pick).toHaveBeenCalledExactlyOnceWith(null);
});

it('clears the search field when the filter bar clears applied search', () => {
  const search = vi.fn();
  const state = searchState(search);
  const view = render(<TableSearch state={state} />);
  const field = screen.getByRole('textbox', { name: 'Search all columns' });
  fireEvent.change(field, { target: { value: 'ann' } });
  view.rerender(
    <TableSearch
      state={{ ...state, view: { ...state.view, search: 'ann' } }}
    />,
  );
  expect(field).toHaveValue('ann');
  view.rerender(<TableSearch state={state} />);
  expect(field).toHaveValue('');
});

it('keeps composition confirmation from applying a header quick filter', () => {
  const apply = vi.fn();
  const close = vi.fn();
  render(
    <QuickFilter
      name="name"
      x={10}
      y={10}
      operators={quickOperators('text', 'postgres')}
      onApply={apply}
      onClose={close}
    />,
  );
  const field = screen.getByRole('textbox', { name: 'Value of name' });
  fireEvent.change(field, { target: { value: '名前' } });
  fireEvent.keyDown(field, { key: 'Enter', isComposing: true });
  fireEvent.keyDown(field, { key: 'Escape', isComposing: true });
  expect(apply).not.toHaveBeenCalled();
  expect(close).not.toHaveBeenCalled();
  fireEvent.keyDown(field, { key: 'Enter' });
  expect(apply).toHaveBeenCalledExactlyOnceWith('contains', '名前');
});

it('keeps composition confirmation from submitting or clearing table search', () => {
  const search = vi.fn();
  render(<TableSearch state={searchState(search)} />);
  const field = screen.getByRole('textbox', { name: 'Search all columns' });
  fireEvent.change(field, { target: { value: '名前' } });
  fireEvent.keyDown(field, { key: 'Enter', isComposing: true });
  fireEvent.keyDown(field, { key: 'Escape', isComposing: true });
  expect(search).not.toHaveBeenCalled();
  expect(field).toHaveValue('名前');
  fireEvent.keyDown(field, { key: 'Enter' });
  expect(search).toHaveBeenCalledExactlyOnceWith('名前');
});

it('clears pending search typing before its debounce can apply', () => {
  vi.useFakeTimers();
  try {
    const search = vi.fn();
    const state = searchState(search);
    let view = initialView;
    const rendered = render(<TableSearch state={{ ...state, view }} />);
    const field = screen.getByRole('textbox', { name: 'Search all columns' });
    fireEvent.change(field, { target: { value: 'ann' } });
    filterActions(
      (change) => {
        view = change(view);
      },
      ['name'],
    ).clear();
    rendered.rerender(<TableSearch state={{ ...state, view }} />);
    expect(field).toHaveValue('');
    act(() => {
      vi.advanceTimersByTime(400);
    });
    expect(search).not.toHaveBeenCalled();
  } finally {
    vi.useRealTimers();
  }
});
