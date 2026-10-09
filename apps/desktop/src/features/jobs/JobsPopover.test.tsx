// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { openExport, revealExport } from '@openrows/ipc';
import { JobsButton } from './JobsButton';
import { addJob, setJobsOpen } from './jobStore';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

function renderButton(): void {
  render(
    <ThemeProvider>
      <JobsButton />
    </ThemeProvider>,
  );
}

it('lists jobs of any kind with progress, cancel, outputs and details', async () => {
  const user = userEvent.setup();
  const cancel = vi.fn();
  const details = vi.fn();
  const exporting = addJob({
    kind: 'export',
    title: 'Export orders.xlsx',
    cancel,
  });
  const importing = addJob({
    kind: 'sync',
    title: 'Import people.csv',
    details,
  });
  renderButton();
  const button = screen.getByRole('button', {
    name: 'Background jobs (2 running)',
  });
  await user.click(button);
  const popover = await screen.findByRole('dialog', {
    name: 'Background jobs',
  });
  const exportRow = within(popover).getByRole('listitem', {
    name: 'Export orders.xlsx',
  });
  act(() => {
    exporting.update({ detail: '120 rows written', fraction: 0.25 });
  });
  expect(within(exportRow).getByText('120 rows written')).toBeInTheDocument();
  expect(within(exportRow).getByRole('progressbar')).toHaveAttribute(
    'value',
    '0.25',
  );
  await user.click(within(exportRow).getByRole('button', { name: 'Cancel' }));
  expect(cancel).toHaveBeenCalled();
  act(() => {
    exporting.finish('480 rows exported', '/tmp/orders.xlsx');
  });
  expect(
    within(exportRow).queryByRole('button', { name: 'Cancel' }),
  ).toBeNull();
  await user.click(within(exportRow).getByRole('button', { name: 'Open' }));
  expect(openExport).toHaveBeenCalledWith('/tmp/orders.xlsx');
  await user.click(
    within(exportRow).getByRole('button', { name: 'Show in Finder' }),
  );
  expect(revealExport).toHaveBeenCalledWith('/tmp/orders.xlsx');
  const importRow = within(popover).getByRole('listitem', {
    name: 'Import people.csv',
  });
  expect(within(importRow).getByText('sync')).toBeInTheDocument();
  await user.click(within(importRow).getByRole('button', { name: 'Details…' }));
  expect(details).toHaveBeenCalled();
  act(() => {
    importing.fail('Row 3: invalid date');
  });
  expect(within(importRow).getByRole('status')).toHaveTextContent(
    'Row 3: invalid date',
  );
  expect(
    screen.getByRole('button', { name: 'Background jobs' }),
  ).toBeInTheDocument();
  await user.click(
    within(popover).getByRole('button', { name: 'Remove Export orders.xlsx' }),
  );
  expect(
    within(popover).queryByRole('listitem', { name: 'Export orders.xlsx' }),
  ).toBeNull();
  await user.click(within(popover).getByRole('button', { name: 'Clear' }));
  expect(
    within(popover).getByText(/appear here while they run/),
  ).toBeInTheDocument();
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('dialog', { name: 'Background jobs' })).toBeNull();
});

it('opens from the Show Background Jobs command and closes on an outside press', async () => {
  const user = userEvent.setup();
  renderButton();
  act(() => {
    setJobsOpen(true);
  });
  expect(
    await screen.findByRole('dialog', { name: 'Background jobs' }),
  ).toBeInTheDocument();
  await user.click(document.body);
  expect(screen.queryByRole('dialog', { name: 'Background jobs' })).toBeNull();
});
