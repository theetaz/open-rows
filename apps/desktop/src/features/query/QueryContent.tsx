// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { Button, Tabs } from '@openrows/ui';
import { sessionOf } from '../connections';
import type { Connections } from '../connections';
import type { QueryTabs } from './useQueryTabs';
import { useQueryRuns } from './useQueryRuns';
import type { SelectedCell } from './cell';
import styles from './Query.module.css';
type SessionStatus = ReturnType<Connections['statusOf']>;
const QueryTab = lazy(() =>
  import('./QueryTab').then((module) => ({ default: module.QueryTab })),
);

type Props = {
  connections: Connections;
  tabs: QueryTabs;
  /** Active database of the connection; empty when none is open. */
  database: string;
  onCell: (cell: SelectedCell) => void;
};

/**
 * Query tabs of the active connection and database; editor and grid load
 * with the first tab.
 */
export function QueryContent(props: Props): React.JSX.Element {
  const runs = useQueryRuns();
  const connection = props.connections.active;
  if (!connection)
    return (
      <main className={styles.content}>
        <p className={styles.empty}>
          Choose a connection in the rail to open a query tab.
        </p>
      </main>
    );
  if (!props.database)
    return (
      <main className={styles.content}>
        <p className={styles.empty}>
          Open a database with ⌘K to start a query tab.
        </p>
      </main>
    );
  const { tabs } = props;
  const scope = { connectionId: connection.id, database: props.database };
  const list = tabs.tabsFor(scope);
  const tab = tabs.activeFor(scope);
  const status = props.connections.statusOf(connection.id);
  const open = (): void => {
    tabs.open(scope);
  };
  return (
    <main className={styles.content}>
      <Tabs
        tabs={list.map((item) => ({
          id: item.id,
          label: item.title,
          kind: 'query' as const,
          title: `${connection.name} · ${item.title}`,
        }))}
        active={tab?.id ?? ''}
        onSelect={tabs.select}
        onClose={(id) => {
          runs.release(id);
          tabs.close(id);
        }}
        onAdd={open}
      />
      {tab ? (
        <Suspense fallback={<p className={styles.empty}>Loading editor…</p>}>
          <QueryTab
            key={tab.id}
            tab={tab}
            connection={connection}
            sessionId={sessionOf(status, tab.database)}
            notice={notice(status, tab.database)}
            runs={runs}
            onSql={tabs.edit}
            onCell={props.onCell}
            onConnect={props.connections.connect}
          />
        </Suspense>
      ) : (
        <NoTabs database={props.database} onOpen={open} />
      )}
    </main>
  );
}

function NoTabs(props: {
  database: string;
  onOpen: () => void;
}): React.JSX.Element {
  return (
    <div className={styles.empty}>
      <p>No query tabs on {props.database}.</p>
      <Button onClick={props.onOpen}>New query tab (⌘T)</Button>
    </div>
  );
}

/** Why a tab cannot run although its connection is up. */
function notice(status: SessionStatus, database: string): string | null {
  if (status.state !== 'connected') return null;
  const opened = status.databases.find((item) => item.name === database);
  if (!opened) return `${database} is closed; open it again with ⌘K`;
  if (opened.state === 'opening') return `Opening ${database}…`;
  return opened.state === 'failed' ? opened.message : null;
}
