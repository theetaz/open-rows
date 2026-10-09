// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import type { Mock } from 'vitest';
import { profile } from '../../test/ipcStub';
import { QueryTab } from './QueryTab';
import { idleRun } from './run';
import type { QueryTab as Tab } from './tabs';
import type { QueryRuns } from './useQueryRuns';
import type { QueryTabs } from './useQueryTabs';

/** Query runs that record calls; `runOf` is idle unless overridden. */
export function fakeRuns(): QueryRuns {
  return {
    runOf: () => idleRun,
    execute: vi.fn(() => Promise.resolve()),
    confirm: vi.fn(),
    dismiss: vi.fn(),
    cancel: vi.fn(),
    release: vi.fn(),
  };
}

/** Tab-set actions of a query tab, recording calls. */
export function fakeTabs(): {
  edit: Mock<QueryTabs['edit']>;
  open: Mock<QueryTabs['open']>;
  bindFile: Mock<QueryTabs['bindFile']>;
  setParams: Mock<QueryTabs['setParams']>;
} {
  return {
    edit: vi.fn<QueryTabs['edit']>(),
    open: vi.fn<QueryTabs['open']>(),
    bindFile: vi.fn<QueryTabs['bindFile']>(),
    setParams: vi.fn<QueryTabs['setParams']>(),
  };
}

const firstTab: Tab = {
  id: 'query-1',
  kind: 'query',
  title: 'SQL Query 1',
  sql: 'select 1',
  connectionId: 'id-local',
  database: 'orders',
};

/** Render one query tab on the `local` profile with an open session. */
export function renderTab(
  runs: QueryRuns = fakeRuns(),
  options: { tab?: Partial<Tab>; tabs?: ReturnType<typeof fakeTabs> } = {},
): {
  user: ReturnType<typeof userEvent.setup>;
  unmount: () => void;
  tabs: ReturnType<typeof fakeTabs>;
} {
  const user = userEvent.setup();
  const tabs = options.tabs ?? fakeTabs();
  const { unmount } = render(
    <QueryTab
      tab={{ ...firstTab, ...options.tab }}
      connection={profile('local')}
      sessionId="session-1"
      notice={null}
      catalog={null}
      runs={runs}
      tabs={tabs}
      onCell={vi.fn()}
      onConnect={vi.fn()}
    />,
  );
  return { user, unmount, tabs };
}

/** Open a bar menu and click the row whose text matches `item`. */
export async function choose(
  user: ReturnType<typeof userEvent.setup>,
  button: string,
  item: RegExp,
): Promise<void> {
  await user.click(screen.getByRole('button', { name: button }));
  const menu = screen.getByRole('menu', { name: button });
  const row = Array.from(
    menu.querySelectorAll<HTMLElement>('[role^="menuitem"]'),
  ).find((element) => item.test(element.textContent));
  if (!row) throw new Error(`no ${String(item)} in ${button}`);
  await user.click(row);
}

/** Open a bar menu's submenu and pick one of its choices. */
export async function chooseIn(
  user: ReturnType<typeof userEvent.setup>,
  button: string,
  submenu: string,
  item: RegExp,
): Promise<void> {
  await user.click(screen.getByRole('button', { name: button }));
  await user.click(screen.getByRole('menuitem', { name: new RegExp(submenu) }));
  const menu = screen.getByRole('menu', { name: submenu });
  const row = Array.from(
    menu.querySelectorAll<HTMLElement>('[role^="menuitem"]'),
  ).find((element) => item.test(element.textContent));
  if (!row) throw new Error(`no ${String(item)} in ${submenu}`);
  await user.click(row);
}
