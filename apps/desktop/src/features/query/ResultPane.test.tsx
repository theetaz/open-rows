// SPDX-License-Identifier: Apache-2.0
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ResultPane } from './ResultPane';
import { idleRun } from './run';
import type { ResultMeta, RunState } from './run';
import type { ResultView } from './editorSettings';

const result = (id: string, statement: number): ResultMeta => ({
  id,
  statement,
  columns: [],
  rows: 0,
  done: false,
  truncated: false,
});

const run: RunState = {
  ...idleRun,
  status: 'running',
  queryId: 'q1',
  results: [result('r1', 0), result('r2', 1)],
};

function show(view: ResultView): void {
  render(
    <ResultPane
      run={run}
      view={view}
      runDefault="current"
      engine="postgres"
      onCell={() => undefined}
    />,
  );
}

it('stacks every result of a multi-statement run with a caption each', () => {
  show('stacked');
  expect(screen.queryByRole('tablist')).toBeNull();
  const first = screen.getByRole('region', { name: 'Result 1 · statement 1' });
  const second = screen.getByRole('region', { name: 'Result 2 · statement 2' });
  expect(within(first).getByRole('status')).toHaveTextContent('Receiving rows');
  expect(within(second).getByRole('status')).toHaveTextContent(
    'Receiving rows',
  );
});

it('keeps one tab per result in the tab view', () => {
  show('tabs');
  expect(screen.getAllByRole('tab')).toHaveLength(2);
  expect(screen.queryByRole('region', { name: /Result 1/ })).toBeNull();
});

it('keeps the plan beside stacked results and shows notices from every visible result', async () => {
  render(
    <ResultPane
      run={{
        ...run,
        messages: [
          { statement: 0, level: 'info', text: 'first notice', code: null },
          { statement: 1, level: 'warning', text: 'second notice', code: null },
        ],
      }}
      view="stacked"
      runDefault="current"
      engine="postgres"
      onCell={vi.fn()}
      extra={{
        id: 'plan',
        label: 'Plan',
        content: <p>Plan content</p>,
        onClose: vi.fn(),
      }}
    />,
  );
  expect(screen.getByText('Plan content')).toBeVisible();
  const user = userEvent.setup();
  await user.click(screen.getByRole('tab', { name: 'Results' }));
  expect(
    screen.getByRole('region', { name: 'Result 1 · statement 1' }),
  ).toBeVisible();
  expect(
    screen.getByRole('region', { name: 'Result 2 · statement 2' }),
  ).toBeVisible();
  expect(screen.getByRole('list', { name: 'Messages' })).toHaveTextContent(
    'first notice',
  );
  expect(screen.getByRole('list', { name: 'Messages' })).toHaveTextContent(
    'second notice',
  );
});
