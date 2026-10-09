// SPDX-License-Identifier: Apache-2.0
import { screen, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { explainQuery } from '@openrows/ipc';
import { samplePlan } from '../../test/planStub';
import { choose, renderTab } from '../query/testQueryTab';
import { runTabCommand } from '../commands';
import { setExplain } from './explainStore';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('../query/QueryEditor', async () => ({
  QueryEditor: (await import('../query/testEditor')).TextareaEditor,
}));

const explain = vi.mocked(explainQuery);

afterEach(() => {
  setExplain('query-1', null);
  explain.mockReset();
});

it('explains from the editor bar into a closable Query Plan result tab', async () => {
  explain.mockResolvedValue({ kind: 'explained', plan: samplePlan() });
  const { user } = renderTab();
  await user.click(screen.getByRole('button', { name: 'Explain' }));
  expect(explain).toHaveBeenCalledWith(
    expect.objectContaining({
      sessionId: 'session-1',
      sql: 'select 1',
      analyze: false,
    }),
  );
  const tab = await screen.findByRole('tab', { name: 'Query Plan' });
  expect(tab).toHaveAttribute('aria-selected', 'true');
  expect(
    await screen.findByRole('tree', { name: 'Plan operators' }),
  ).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Close Query Plan' }));
  expect(
    screen.queryByRole('tab', { name: 'Query Plan' }),
  ).not.toBeInTheDocument();
});

it('runs Explain Analyze from the menu and the command after consent', async () => {
  explain
    .mockResolvedValueOnce({
      kind: 'confirmationRequired',
      token: 'tok',
      statement: 'delete from t',
    })
    .mockResolvedValueOnce({ kind: 'explained', plan: samplePlan() });
  const { user } = renderTab();
  await choose(user, 'Explain options', /Explain Analyze/);
  expect(explain).toHaveBeenLastCalledWith(
    expect.objectContaining({ analyze: true }),
  );
  const sheet = await screen.findByRole('dialog', { name: /Explain Analyze/ });
  expect(within(sheet).getByText('delete from t')).toBeInTheDocument();
  await user.click(
    within(sheet).getByRole('button', { name: 'Run and Roll Back' }),
  );
  expect(explain).toHaveBeenLastCalledWith(
    expect.objectContaining({ confirmToken: 'tok' }),
  );
  expect(
    await screen.findByRole('tab', { name: 'Explain Analyze' }),
  ).toBeInTheDocument();
  setExplain('query-1', null);
  explain.mockResolvedValueOnce({
    kind: 'blocked',
    message: 'This connection is read-only',
  });
  expect(runTabCommand('explain-analyze')).toBe(true);
  expect(await screen.findByRole('alert')).toHaveTextContent('read-only');
});
