// SPDX-License-Identifier: Apache-2.0
import { Suspense } from 'react';
import { TabBody } from './TabBody';
import type { ProfileDto } from '@openrows/ipc';
import { useTableCloseGuard } from '../table/useTableCloseGuard';
import { Button, Icon } from '@openrows/ui';
import type { IconName } from '@openrows/ui';
import { openedDatabase } from '../connections';
import type { Connections } from '../connections';
import { useTableViews } from '../table';
import type { SelectedTable, ShowRow } from '../table';
import type { QueryTabs } from './useQueryTabs';
import { useQueryRuns } from './useQueryRuns';
import type { SelectedCell } from './cell';
import { ReconnectBar } from './ReconnectBar';
import { TabStrip } from './TabStrip';
import { SplitPanes } from './SplitPanes';
import { useSplit } from './useSplit';
import type { Split } from './useSplit';
import { usePaneCommands } from '../commands';
import type { PaneCommand } from '../commands';
import styles from './Query.module.css';
/** Inputs shared by the workspace and its lazily rendered tab bodies. */
export type QueryContentProps = {
  connections: Connections;
  tabs: QueryTabs;
  /** Active database of the connection; empty when none is open. */
  database: string;
  onCell: (cell: SelectedCell) => void;
  /** A table view's selected row, for the inspector's Details. */
  onRow: ShowRow;
  /** Table info for the inspector (Structure/Index modes); `null` clears it. */
  onTableInfo?: ((table: SelectedTable | null) => void) | undefined;
  /** Bumped by schema refresh; drops cached completion data. */
  catalogRevision?: number | undefined;
  /** Open a table of this database (e.g. a foreign key's target). */
  onOpenTable?:
    | ((table: { database: string; schema: string; name: string }) => void)
    | undefined;
  /** Open a trigger editor of a table: new (`name: null`), edit or drop. */
  onTrigger?:
    | ((
        table: { database: string; schema: string; name: string },
        name: string | null,
        drop: boolean,
      ) => void)
    | undefined;
  /** Bumped after an object editor changed the schema; table tabs re-read. */
  structureRevision?: number | undefined;
};

/**
 * Query tabs of the active connection and database; editor and grid load
 * with the first tab.
 */
export function QueryContent(props: QueryContentProps): React.JSX.Element {
  const runs = useQueryRuns();
  const tables = useTableViews();
  const closeGuard = useTableCloseGuard({ tables, runs, tabs: props.tabs });
  const connection = props.connections.active;
  const split = useSplitPanes(props.tabs, connection, props.database);
  if (!connection) return <NoConnection connections={props.connections} />;
  if (!props.database)
    return (
      <main className={styles.content}>
        <EmptyState
          icon="database"
          title="No database open"
          detail="Open a database with ⌘K to start a query tab."
        />
      </main>
    );
  const { tabs } = props;
  const shown = { connectionId: connection.id, database: props.database };
  const list = tabs.tabsFor(shown);
  const tab = tabs.activeFor(shown);
  const status = props.connections.statusOf(connection.id);
  const open = (): void => {
    tabs.open(shown);
  };
  return (
    <main className={styles.content}>
      {closeGuard}
      {list.length > 0 && (
        <ReconnectBar
          profile={connection}
          status={status}
          onConnect={props.connections.connect}
          database={openedDatabase(status, props.database)}
          onReopen={() => {
            props.connections.openDatabase(props.database);
          }}
        />
      )}
      <TabStrip
        tabs={split.stripTabs}
        list={list}
        active={tab?.id ?? ''}
        connection={connection.name}
        onAdd={open}
      />
      {tab ? (
        <Suspense fallback={<p className={styles.empty}>Loading…</p>}>
          <SplitPanes
            split={split}
            active={tab}
            render={(paneTab) => (
              <TabBody
                {...props}
                tab={paneTab}
                connection={connection}
                status={status}
                runs={runs}
                tables={tables}
              />
            )}
          />
        </Suspense>
      ) : (
        <NoTabs database={props.database} onOpen={open} />
      )}
    </main>
  );
}

/** The scope's split panes, the target of the split pane commands. */
function useSplitPanes(
  tabs: QueryTabs,
  connection: ProfileDto | undefined,
  database: string,
): Split {
  const scope =
    connection && database
      ? { connectionId: connection.id, database }
      : undefined;
  const split = useSplit(tabs, scope);
  usePaneCommands((id) => {
    paneCommand(split, id);
  });
  return split;
}

/** Split, turn, focus or close the editor panes. */
function paneCommand(split: Split, id: PaneCommand): void {
  if (id === 'split-right' || id === 'split-down')
    split.split(id === 'split-right' ? 'right' : 'down');
  else if (id === 'focus-pane') split.focusOther();
  else split.close();
}

function NoConnection(props: { connections: Connections }): React.JSX.Element {
  return (
    <main className={styles.content}>
      <EmptyState
        icon="database"
        title="No connection selected"
        detail="Choose a connection in the sidebar to open a query tab."
        action="New connection…"
        onAction={() => {
          props.connections.setSheet({ kind: 'new' });
        }}
      />
    </main>
  );
}

function NoTabs(props: {
  database: string;
  onOpen: () => void;
}): React.JSX.Element {
  return (
    <EmptyState
      icon="terminal"
      title={`No query tabs on ${props.database}.`}
      detail="Open a table from the sidebar, or start a new query."
      action="New query tab (⌘T)"
      onAction={props.onOpen}
    />
  );
}

/** Centred icon, title, hint and optional action for an empty content area. */
function EmptyState(props: {
  icon: IconName;
  title: string;
  detail: string;
  action?: string;
  onAction?: () => void;
}): React.JSX.Element {
  return (
    <div className={styles.empty}>
      <span className={styles.emptyIcon} aria-hidden="true">
        <Icon name={props.icon} />
      </span>
      <p className={styles.emptyTitle}>{props.title}</p>
      <p>{props.detail}</p>
      {props.action && (
        <Button size="sm" onClick={props.onAction}>
          {props.action}
        </Button>
      )}
    </div>
  );
}
