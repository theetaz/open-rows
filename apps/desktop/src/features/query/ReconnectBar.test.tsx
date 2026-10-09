// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import type { OpenedDatabase, SessionStatus } from '../connections';
import { profile, server } from '../../test/ipcStub';
import { ReconnectBar } from './ReconnectBar';

const connected = (database: OpenedDatabase): SessionStatus => ({
  state: 'connected',
  insecure: [],
  server: server('17.2'),
  databases: [database],
  generation: 1,
});

function show(database: OpenedDatabase, onReopen = vi.fn()) {
  return render(
    <ReconnectBar
      profile={profile('orders-local')}
      status={connected(database)}
      onConnect={vi.fn()}
      database={database}
      onReopen={onReopen}
    />,
  );
}

it('says the tab database is reconnecting and that queries wait', () => {
  show({
    name: 'orders',
    state: 'reconnecting',
    sessionId: 's',
    message: 'connection reset',
  });
  expect(screen.getByRole('status')).toHaveTextContent(
    'The connection to orders was lost. Reconnecting… Queries wait until it is back.',
  );
  expect(screen.queryByRole('button')).toBeNull();
});

it('offers to reopen a database whose connection could not be restored', async () => {
  const onReopen = vi.fn();
  show(
    {
      name: 'orders',
      state: 'failed',
      lost: true,
      message: 'could not be re-established after 5 attempts',
    },
    onReopen,
  );
  expect(screen.getByRole('status')).toHaveTextContent(
    'Lost the connection to orders: could not be re-established after 5 attempts',
  );
  await userEvent.click(screen.getByRole('button', { name: 'Reconnect' }));
  expect(onReopen).toHaveBeenCalledOnce();
});

it('stays hidden while the database session is up', () => {
  const { container } = show({ name: 'orders', state: 'open', sessionId: 's' });
  expect(container).toBeEmptyDOMElement();
});
