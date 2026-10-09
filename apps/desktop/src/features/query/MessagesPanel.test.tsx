// SPDX-License-Identifier: Apache-2.0
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ConfirmRun } from './ConfirmRun';
import { MessagesPanel } from './MessagesPanel';
import type { RunMessage } from './run';

const messages: RunMessage[] = [
  { statement: 0, level: 'info', text: 'table created', code: null },
  { statement: 1, level: 'warning', text: 'truncated', code: '01000' },
  { statement: 1, level: 'error', text: 'boom', code: '42P01' },
];

it('filters server messages by level and labels their statements', async () => {
  const user = userEvent.setup();
  render(<MessagesPanel messages={messages} statements={2} />);
  const list = screen.getByRole('list', { name: 'Messages' });
  expect(within(list).getAllByRole('listitem')).toHaveLength(3);
  expect(list).toHaveTextContent('#2WARNING 01000: truncated');
  await user.click(screen.getByRole('button', { name: 'Warnings 1' }));
  expect(within(list).getAllByRole('listitem')).toHaveLength(1);
  expect(screen.getByRole('button', { name: 'Warnings 1' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await user.click(screen.getByRole('button', { name: 'Info 1' }));
  expect(list).toHaveTextContent('INFO: table created');
});

it('asks before a multi-statement run of reads', () => {
  render(
    <ConfirmRun
      connection="orders-local"
      statements={[0, 1].map((index) => ({
        index,
        sql: `SELECT ${String(index)}`,
        kind: 'read',
        dangerous: false,
        verdict: 'allow',
      }))}
      onConfirm={vi.fn()}
      onCancel={vi.fn()}
    />,
  );
  expect(screen.getByText(/this run has 2 statements/)).toBeVisible();
  expect(
    within(
      screen.getByRole('list', { name: 'Statements to confirm' }),
    ).getAllByRole('listitem'),
  ).toHaveLength(2);
});
