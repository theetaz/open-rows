// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { EnginePicker } from './EnginePicker';

function mount() {
  const onCreate = vi.fn();
  const onCancel = vi.fn();
  render(<EnginePicker onCreate={onCreate} onCancel={onCancel} />);
  return { onCreate, onCancel, user: userEvent.setup() };
}

it('creates the selected engine and refuses types without a driver', async () => {
  const { onCreate, user } = mount();
  expect(screen.getByRole('option', { selected: true })).toHaveTextContent(
    'PostgreSQL',
  );
  await user.click(screen.getByRole('option', { name: /MySQL/ }));
  await user.click(screen.getByRole('button', { name: 'Create' }));
  expect(onCreate).toHaveBeenLastCalledWith('mysql');
  await user.click(screen.getByRole('option', { name: /Oracle/ }));
  expect(screen.getByRole('status')).toHaveTextContent(
    'Oracle connections are coming soon.',
  );
  expect(screen.getByRole('button', { name: 'Create' })).toBeDisabled();
  await user.dblClick(screen.getByRole('option', { name: /SQLite/ }));
  expect(onCreate).toHaveBeenLastCalledWith('sqlite');
});

it('filters as you type and moves with the arrow keys', async () => {
  const { onCreate, user } = mount();
  await user.type(screen.getByRole('searchbox'), 'maria');
  expect(screen.getAllByRole('option')).toHaveLength(1);
  await user.click(screen.getByRole('button', { name: 'Create' }));
  expect(onCreate).toHaveBeenLastCalledWith('mariadb');
  await user.clear(screen.getByRole('searchbox'));
  await user.type(screen.getByRole('searchbox'), 'zzz');
  expect(screen.getByText('No connection type matches.')).toBeInTheDocument();
});

it('arrows in the grid move the outlined tile', async () => {
  const { user } = mount();
  screen.getByRole('listbox').focus();
  await user.keyboard('{ArrowRight}{ArrowRight}');
  expect(screen.getByRole('option', { selected: true })).toHaveTextContent(
    'MySQL',
  );
});
