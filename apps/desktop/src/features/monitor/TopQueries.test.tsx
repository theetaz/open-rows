// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { TopQueries } from './TopQueries';
import { decodeTable } from './monitorData';
import { monitorBytes } from './monitorFixture';
vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

function renderTopQueries(explain: boolean): void {
  const table = decodeTable(monitorBytes('statements'));
  render(
    <TopQueries
      table={table}
      target={{ profileId: 'p', database: 'db' }}
      production={false}
      now={16000}
      revision="1"
      activity={undefined}
      explain={explain}
      onSession={vi.fn()}
      onEvidence={vi.fn()}
      onOpenSql={vi.fn()}
    />,
  );
}

it('offers monitor Explain only where the PostgreSQL diagnostic session exists', () => {
  renderTopQueries(false);
  expect(screen.queryByRole('button', { name: 'Explain' })).toBeNull();
  expect(
    screen.getByText(/Explain from the Monitor is PostgreSQL only/),
  ).toBeInTheDocument();
});

it('keeps monitor Explain for PostgreSQL', () => {
  renderTopQueries(true);
  expect(screen.getByRole('button', { name: 'Explain' })).toBeInTheDocument();
});
