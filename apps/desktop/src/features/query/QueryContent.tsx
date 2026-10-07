// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { Button, Tabs } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
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
  /** Browsed database; empty for the connection's own database. */
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
            sessionId={runnable(status, tab.database)}
            notice={elsewhere(status, tab.database)}
            runs={runs}
            onSql={tabs.edit}
            onCell={props.onCell}
            onConnect={props.connections.connect}
          />
        </Suspense>
      ) : (
        <NoTabs connection={connection} onOpen={open} />
      )}
    </main>
  );
}

function NoTabs(props: {
  connection: ProfileDto;
  onOpen: () => void;
}): React.JSX.Element {
  return (
    <div className={styles.empty}>
      <p>No query tabs on {props.connection.name}.</p>
      <Button onClick={props.onOpen}>New query tab (⌘T)</Button>
    </div>
  );
}

/**
 * The session a tab on `database` runs on: only tabs of the connection's own
 * database run until per-database query sessions exist.
 */
function runnable(status: SessionStatus, database: string): string | null {
  return status.state === 'connected' && !database ? status.sessionId : null;
}

/** Why a tab of another database cannot run on the connection's session. */
function elsewhere(status: SessionStatus, database: string): string | null {
  if (!database) return null;
  const own = status.state === 'connected' ? status.server.database : '';
  return `Runs on ${own || 'the connection database'} only; ${database} is browse-only for now`;
}
