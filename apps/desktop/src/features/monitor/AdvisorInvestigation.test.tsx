// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import {
  cancelQuery,
  explainMonitorQuery,
  investigateStatement,
  proposeMonitorSql,
  validateMonitorIndex,
} from '@openrows/ipc';
import type { Suggestion, Investigation } from '@openrows/ipc';
import { AdvisorList } from './AdvisorList';
import { QueryReview } from './QueryReview';
import { QueryInvestigation } from './QueryInvestigation';
import { BlockingTree } from './BlockingTree';
import { TopQueries } from './TopQueries';
import { decodeTable } from './monitorData';
import { monitorBytes } from './monitorFixture';
vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
const target = { profileId: 'p', database: 'db' };
const card: Suggestion = {
  id: 'pg.selective-scan:q:0',
  ruleId: 'pg.selective-scan',
  version: 1,
  confidence: 'medium',
  impact: 'Reduce scanned rows',
  tradeOffs: 'Write amplification and locks',
  evidence: 'node 0: 1 of 200000 rows; 8000000 bytes',
  evidenceLink: 'plan:0',
  sql: 'CREATE INDEX CONCURRENTLY ON "public"."events" ("key");',
  statementId: 'q',
  collectedMs: 1000,
};
beforeEach(() => {
  localStorage.clear();
});
it('enqueues proposals without any execution, links evidence and isolates local dismissal', async () => {
  const onEvidence = vi.fn();
  const mounted = render(
    <AdvisorList
      target={target}
      suggestions={[card]}
      now={1000}
      production
      onEvidence={onEvidence}
    />,
  );
  expect(screen.getByText(/Production:/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'View evidence' }));
  expect(onEvidence).toHaveBeenCalledWith('statements:q');
  await userEvent.click(
    screen.getByRole('button', { name: 'Open SQL in editor' }),
  );
  expect(proposeMonitorSql).toHaveBeenCalledWith(target, card.id);
  expect(explainMonitorQuery).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
  expect(screen.queryByText('pg.selective-scan v1')).not.toBeInTheDocument();
  mounted.unmount();
  const second = render(
    <AdvisorList
      target={target}
      suggestions={[card]}
      now={2000}
      production={false}
      onEvidence={onEvidence}
    />,
  );
  expect(
    screen.queryByRole('button', { name: 'Open SQL in editor' }),
  ).not.toBeInTheDocument();
  second.unmount();
  render(
    <AdvisorList
      target={{ ...target, profileId: 'other' }}
      suggestions={[card]}
      now={2000}
      production={false}
      onEvidence={onEvidence}
    />,
  );
  expect(
    screen.getByRole('button', { name: 'Open SQL in editor' }),
  ).toBeInTheDocument();
  expect(localStorage.getItem('openrows-advisor-v1:p:db')).not.toContain(
    'CREATE INDEX',
  );
});
it('snoozes for one hour and exposes expired evidence errors without opening SQL', async () => {
  vi.mocked(proposeMonitorSql).mockRejectedValueOnce(new Error('expired'));
  const mounted = render(
    <AdvisorList
      target={target}
      suggestions={[card]}
      now={1000}
      production={false}
      onEvidence={vi.fn()}
    />,
  );
  await userEvent.click(
    screen.getByRole('button', { name: 'Open SQL in editor' }),
  );
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'Evidence changed',
  );
  await userEvent.click(screen.getByRole('button', { name: 'Snooze 1 hour' }));
  expect(
    screen.queryByRole('button', { name: 'Open SQL in editor' }),
  ).not.toBeInTheDocument();
  mounted.rerender(
    <AdvisorList
      target={target}
      suggestions={[card]}
      now={3_601_001}
      production={false}
      onEvidence={vi.fn()}
    />,
  );
  expect(
    screen.getByRole('button', { name: 'Open SQL in editor' }),
  ).toBeInTheDocument();
});
it('keeps HypoPG off until explicit review and opt-in, then shows estimated costs', async () => {
  render(
    <AdvisorList
      target={target}
      suggestions={[card]}
      now={1000}
      production={false}
      querySql="SELECT * FROM events WHERE key = ?"
      onEvidence={vi.fn()}
    />,
  );
  await userEvent.click(screen.getByRole('button', { name: 'HypoPG what-if' }));
  expect(
    await screen.findByRole('button', { name: 'Compare plans' }),
  ).toBeDisabled();
  expect(validateMonitorIndex).not.toHaveBeenCalled();
  await userEvent.clear(
    screen.getByRole('textbox', { name: 'Reviewed what-if SQL' }),
  );
  await userEvent.type(
    screen.getByRole('textbox', { name: 'Reviewed what-if SQL' }),
    'SELECT * FROM events WHERE key = 1',
  );
  await userEvent.click(
    screen.getByRole('switch', { name: 'What-if SQL reviewed' }),
  );
  await userEvent.click(screen.getByRole('switch', { name: 'Enable HypoPG' }));
  await userEvent.click(screen.getByRole('button', { name: 'Compare plans' }));
  expect(validateMonitorIndex).toHaveBeenCalledWith(
    expect.objectContaining({
      enabled: true,
      suggestionId: card.id,
      confirmToken: null,
    }),
  );
  expect(
    await screen.findByText(/Estimated total cost: 100 before/),
  ).toHaveTextContent('not a measured performance improvement');
});
it('requires the native policy token before a HypoPG plan comparison', async () => {
  vi.mocked(validateMonitorIndex).mockResolvedValueOnce({
    kind: 'confirmationRequired',
    token: 'test-only',
    statement: 'SELECT ?',
  });
  render(
    <AdvisorList
      target={target}
      suggestions={[card]}
      now={1000}
      production={false}
      querySql="SELECT * FROM events WHERE key = 1"
      onEvidence={vi.fn()}
    />,
  );
  await userEvent.click(screen.getByRole('button', { name: 'HypoPG what-if' }));
  await userEvent.click(
    await screen.findByRole('switch', { name: 'What-if SQL reviewed' }),
  );
  await userEvent.click(screen.getByRole('switch', { name: 'Enable HypoPG' }));
  await userEvent.click(screen.getByRole('button', { name: 'Compare plans' }));
  expect(await screen.findByRole('alertdialog')).toHaveTextContent(
    'confirmation',
  );
  await userEvent.click(
    screen.getByRole('button', { name: 'Confirm plan request' }),
  );
  expect(validateMonitorIndex).toHaveBeenLastCalledWith(
    expect.objectContaining({
      confirmToken: 'test-only',
    }),
  );
});
it('defaults to plan-only review and confirms every measured read with production warning', async () => {
  vi.mocked(explainMonitorQuery).mockResolvedValue({
    kind: 'confirmationRequired',
    token: 'test-only',
    statement: 'SELECT ?',
  });
  const mounted = render(
    <QueryReview
      target={target}
      sql="SELECT ?"
      statementId="q"
      production
      onPlan={vi.fn()}
      onClose={vi.fn()}
    />,
  );
  expect(
    screen.getByRole('button', { name: 'Explain plan only' }),
  ).toBeDisabled();
  expect(explainMonitorQuery).not.toHaveBeenCalled();
  await userEvent.clear(screen.getByRole('textbox', { name: 'Reviewed SQL' }));
  await userEvent.type(
    screen.getByRole('textbox', { name: 'Reviewed SQL' }),
    'SELECT 1',
  );
  await userEvent.click(
    screen.getByRole('switch', { name: 'Statement reviewed' }),
  );
  await userEvent.click(
    screen.getByRole('button', { name: 'Explain plan only' }),
  );
  expect(explainMonitorQuery).toHaveBeenLastCalledWith(
    expect.objectContaining({
      analyze: false,
      schema: 'public',
      confirmToken: null,
    }),
  );
  await userEvent.click(
    await screen.findByRole('button', { name: 'Cancel diagnostic' }),
  );
  await userEvent.click(
    screen.getByRole('button', { name: 'Explain analyse (runs SQL)' }),
  );
  expect(await screen.findByRole('alertdialog')).toHaveTextContent(
    'Production warning',
  );
  expect(explainMonitorQuery).toHaveBeenLastCalledWith(
    expect.objectContaining({ analyze: true, confirmToken: null }),
  );
  await userEvent.click(
    screen.getByRole('button', { name: 'Confirm measured execution' }),
  );
  expect(explainMonitorQuery).toHaveBeenLastCalledWith(
    expect.objectContaining({
      analyze: true,
      sql: 'SELECT 1',
      confirmToken: 'test-only',
    }),
  );
  mounted.unmount();
  expect(cancelQuery).toHaveBeenCalled();
});
it('renders real interval evidence with no invented percentiles and related waits/session navigation', async () => {
  const detail: Investigation = {
    statementId: 'q',
    query: 'UPDATE fixture SET value = ?',
    calls: 10,
    totalMs: 200,
    meanMs: 20,
    maxSinceResetMs: 50,
    regressions: [
      {
        previousMs: 1000,
        currentMs: 16000,
        previousMeanMs: 5,
        currentMeanMs: 20,
      },
    ],
    partial: true,
    intervals: [
      {
        statementId: 'q',
        collectedMs: 16000,
        durationMs: 15000,
        state: { kind: 'ok' },
        calls: 10,
        totalMs: 200,
        rows: 10,
        sharedHit: 2,
        sharedRead: 1,
        tempWritten: 5,
        tempRead: 2,
      },
    ],
  };
  vi.mocked(investigateStatement).mockResolvedValueOnce(detail);
  const onSession = vi.fn();
  render(
    <QueryInvestigation
      target={{ ...target, database: 'orders' }}
      statementId="q"
      revision="1"
      now={16000}
      production={false}
      activity={decodeTable(monitorBytes('activity'))}
      onSession={onSession}
      onEvidence={vi.fn()}
    />,
  );
  expect(await screen.findByText(/p95\/p99 unavailable/)).toBeInTheDocument();
  expect(
    screen.getByRole('table', { name: 'Query interval deltas' }),
  ).toHaveTextContent('200.00');
  expect(screen.getByText(/Regression at/)).toHaveTextContent('5 → 20 ms');
  await userEvent.click(screen.getByRole('button', { name: 'Session 8' }));
  expect(onSession).toHaveBeenCalledWith('8');
});
it('shows blocker statements and query/transaction ages, with no invented wait duration', async () => {
  const onSession = vi.fn();
  render(
    <BlockingTree
      table={decodeTable(monitorBytes('activity'))}
      onSession={onSession}
    />,
  );
  expect(
    screen.getByRole('list', { name: 'Blocking chains' }),
  ).toHaveTextContent('transaction age 80 ms');
  expect(
    screen.getByText(/lock-wait duration is unavailable/),
  ).toBeInTheDocument();
  expect(screen.getAllByText('UPDATE fixture SET value = ?')).toHaveLength(2);
  await userEvent.click(screen.getByRole('button', { name: 'Session 7' }));
  expect(onSession).toHaveBeenCalledWith('7');
  await waitFor(() => {
    expect(
      screen.getByRole('button', { name: 'Session 8' }),
    ).toBeInTheDocument();
  });
});
it('opens advisor query evidence even when its statement is outside the ranking page', async () => {
  vi.mocked(investigateStatement).mockResolvedValueOnce({
    statementId: 'outside-page',
    query: 'SELECT ?',
    calls: null,
    totalMs: null,
    meanMs: null,
    maxSinceResetMs: null,
    regressions: [],
    partial: true,
    intervals: [],
  });
  render(
    <TopQueries
      table={decodeTable(monitorBytes('statements'))}
      target={target}
      production={false}
      now={16000}
      revision="1"
      activity={undefined}
      initialStatementId="outside-page"
      onSession={vi.fn()}
      onEvidence={vi.fn()}
      onOpenSql={vi.fn()}
    />,
  );
  expect(
    await screen.findByRole('heading', {
      name: 'Query investigation · outside-page',
    }),
  ).toBeInTheDocument();
  expect(investigateStatement).toHaveBeenCalledWith(target, 'outside-page');
});
