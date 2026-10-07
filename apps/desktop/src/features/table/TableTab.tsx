// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import { Button } from '@openrows/ui';
import type { ResultMeta } from '../query/run';
import type { TableObject } from '../query/tabs';
import { ColumnsPanel } from './ColumnsPanel';
import { FilterBar } from './FilterBar';
import { StructureView } from './StructureView';
import { TableData } from './TableData';
import { TableFooter } from './TableFooter';
import { readRow } from './row';
import type { SelectedRow } from './row';
import { useTableKeys } from './useTableKeys';
import { useTableTab } from './useTableTab';
import type { TableViews } from './useTableViews';
import styles from './Table.module.css';

type TableTabProps = {
  tabId: string;
  table: TableObject;
  profileId: string;
  /** The session pages run on, or `null` when they cannot run here. */
  sessionId: string | null;
  /** Why rows cannot load even though the connection is open. */
  notice: string | null;
  views: TableViews;
  onRow: (row: SelectedRow) => void;
  onConnect: () => void;
};

/** A TablePlus-style table view: filters, paged canvas grid, structure, footer. */
export function TableTab(props: TableTabProps): React.JSX.Element {
  const { table, sessionId } = props;
  const root = useRef<HTMLElement>(null);
  const state = useTableTab(props);
  const { view, actions, columns } = state;
  useTableKeys(root, actions);
  const showRow = (result: ResultMeta, row: number): void => {
    readRow(result, row).then(
      (fields) => {
        props.onRow({
          table: table.name,
          number: view.offset + row + 1,
          fields,
        });
      },
      () => undefined,
    );
  };
  return (
    <section
      ref={root}
      className={styles.table}
      aria-label={`${table.name} table`}
    >
      <Banner {...props} />
      {view.mode === 'data' && view.filtersOpen && (
        <FilterBar
          view={view}
          columns={columns}
          query={state.query}
          actions={actions}
        />
      )}
      {view.mode === 'data' ? (
        sessionId && <TableData state={state} onRow={showRow} />
      ) : (
        <StructureView structure={state.entry.structure} show={view.mode} />
      )}
      {view.columnsOpen && (
        <ColumnsPanel
          columns={columns}
          hidden={view.hidden}
          actions={actions}
        />
      )}
      <TableFooter state={state} ready={sessionId !== null} />
    </section>
  );
}

/** Why rows are not shown: another database's tab, or no open session. */
function Banner(props: TableTabProps): React.JSX.Element | null {
  if (props.notice)
    return (
      <p className={styles.banner} role="note">
        {props.notice}
      </p>
    );
  if (props.sessionId !== null) return null;
  return (
    <p className={styles.banner}>
      Not connected.{' '}
      <Button size="sm" variant="ghost" onClick={props.onConnect}>
        Connect to browse rows
      </Button>
    </p>
  );
}
