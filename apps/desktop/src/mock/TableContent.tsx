// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, memo, useMemo, useState } from 'react';
import { Button, Tabs } from '@openrows/ui';
import type { WorkspaceTab, TableState } from './data/content';
import { rows, addedRow, discardedRows } from './data/rows';
import type { OrderRow } from './data/rows';
import { DataGrid } from './DataGrid';
const QueryView = lazy(() =>
  import('./TableViews').then((module) => ({ default: module.QueryView })),
);
const TableView = lazy(() =>
  import('./TableViews').then((module) => ({ default: module.TableView })),
);
const CommitReview = lazy(() =>
  import('./CommitReview').then((module) => ({ default: module.CommitReview })),
);
import styles from './Content.module.css';
import { TableToolbar } from './TableToolbar';
import { CommitCapsule, ContentFooter } from './ContentControls';

type Props = {
  tabs: readonly WorkspaceTab[];
  active: string;
  database: string;
  selected: number;
  onSelect: (index: number, row: OrderRow) => void;
  onTab: (id: string) => void;
  onClose: (id: string) => void;
  onAdd: () => void;
  onSql: (sql: string) => void;
  onTable: (state: TableState) => void;
};
/** Content state is independent of panel resize and disclosure controls. */
export const TableContent = memo(function TableContent(
  props: Props,
): React.JSX.Element {
  const [review, setReview] = useState(false);
  const tabs = useTabTitles(props);
  const tab = props.tabs.find((item) => item.id === props.active);
  const { view, filter, pending, setView, setFilter, setAdded, data, discard } =
    useTableData(props, tab, setReview);
  return (
    <main className={styles.content}>
      <Tabs
        tabs={tabs}
        active={props.active}
        onSelect={props.onTab}
        onClose={props.onClose}
        onAdd={props.onAdd}
      />
      {!tab ? (
        <div className={styles.view}>
          <p>No open tabs</p>
          <Button onClick={props.onAdd}>New SQL query</Button>
        </div>
      ) : tab.kind === 'query' ? (
        <Suspense fallback={null}>
          <QueryView
            key={`${props.database}:${tab.id}`}
            sql={tab.sql}
            onChange={props.onSql}
          />
        </Suspense>
      ) : (
        <TableData
          {...{
            view,
            setView,
            filter,
            setFilter,
            added: setAdded,
            data,
            pending,
            discard,
            setReview,
            owner: props,
          }}
        />
      )}
      <ContentFooter />
      {review && (
        <Suspense fallback={null}>
          <CommitReview
            onClose={() => {
              setReview(false);
            }}
            onDiscard={discard}
          />
        </Suspense>
      )}
    </main>
  );
});

function useTabTitles(props: Props): Props['tabs'] {
  return useMemo(
    () =>
      props.tabs.map((tab) => ({
        ...tab,
        title: `${props.database}.${tab.schema ?? 'public'}.${tab.label}`,
      })),
    [props.tabs, props.database],
  );
}

function TableData({
  view,
  setView,
  filter,
  setFilter,
  added,
  data,
  pending,
  discard,
  setReview,
  owner,
}: {
  view: string;
  setView: (value: string) => void;
  filter: boolean;
  setFilter: (value: boolean) => void;
  added: (value: boolean) => void;
  data: readonly OrderRow[];
  pending: boolean;
  discard: () => void;
  setReview: (value: boolean) => void;
  owner: Props;
}): React.JSX.Element {
  return (
    <>
      <TableToolbar
        view={view}
        onView={setView}
        filter={filter}
        onFilter={setFilter}
        onAdd={() => {
          added(true);
        }}
      />
      {view === 'Data' ? (
        <DataGrid
          rows={data}
          selected={owner.selected}
          pending={pending}
          onSelect={owner.onSelect}
        />
      ) : (
        <Suspense fallback={null}>
          <TableView view={view} />
        </Suspense>
      )}
      {pending && view === 'Data' && (
        <CommitCapsule
          onDiscard={discard}
          onReview={() => {
            setReview(true);
          }}
        />
      )}
    </>
  );
}

function useTableData(
  props: Props,
  tab: WorkspaceTab | undefined,
  setReview: (value: boolean) => void,
): {
  view: string;
  filter: boolean;
  pending: boolean;
  data: readonly OrderRow[];
  setView: (value: string) => void;
  setFilter: (value: boolean) => void;
  setAdded: (value: boolean) => void;
  discard: () => void;
} {
  const state = tab?.tableState ?? {
    view: 'Data',
    filter: true,
    pending: true,
    added: false,
  };
  const { view, filter, pending, added } = state;
  const setView = (view: string): void => {
    props.onTable({ ...state, view });
  };
  const setFilter = (filter: boolean): void => {
    props.onTable({ ...state, filter });
  };
  const setAdded = (added: boolean): void => {
    props.onTable({ ...state, added });
  };
  const data = useMemo(
    () => [...(pending ? rows : discardedRows()), ...(added ? [addedRow] : [])],
    [pending, added],
  );
  function discard(): void {
    props.onTable({ ...state, pending: false, added: false });
    setReview(false);
  }
  return { view, filter, pending, setView, setFilter, setAdded, data, discard };
}
