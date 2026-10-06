// SPDX-License-Identifier: Apache-2.0
import { memo, useMemo, useState } from 'react';
import { Button, Tabs } from '@openrows/ui';
import type { WorkspaceTab } from './data/content';
import { rows, addedRow, discardedRows } from './data/rows';
import type { OrderRow } from './data/rows';
import { DataGrid } from './DataGrid';
import { QueryView, TableView } from './TableViews';
import { CommitReview } from './CommitReview';
import styles from './Content.module.css';
import { TableToolbar } from './TableToolbar';
import { CommitCapsule, ContentFooter } from './ContentControls';

type Props = {
  tabs: readonly WorkspaceTab[];
  active: string;
  selected: number;
  onSelect: (index: number, row: OrderRow) => void;
  onTab: (id: string) => void;
  onClose: (id: string) => void;
  onAdd: () => void;
};
/** Content state is independent of panel resize and disclosure controls. */
export const TableContent = memo(function TableContent(
  props: Props,
): React.JSX.Element {
  const [view, setView] = useState('Data');
  const [filter, setFilter] = useState(true);
  const [pending, setPending] = useState(true);
  const [added, setAdded] = useState(false);
  const [review, setReview] = useState(false);
  const tab = props.tabs.find((item) => item.id === props.active);
  const data = useMemo(
    () => [...(pending ? rows : discardedRows()), ...(added ? [addedRow] : [])],
    [pending, added],
  );
  function discard(): void {
    setPending(false);
    setAdded(false);
    setReview(false);
  }
  return (
    <main className={styles.content}>
      <Tabs
        tabs={props.tabs}
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
        <QueryView key={tab.id} sql={tab.sql} />
      ) : (
        <>
          <TableToolbar
            view={view}
            onView={setView}
            filter={filter}
            onFilter={setFilter}
            onAdd={() => {
              setAdded(true);
            }}
          />
          {view === 'Data' ? (
            <DataGrid
              rows={data}
              selected={props.selected}
              pending={pending}
              onSelect={props.onSelect}
            />
          ) : (
            <TableView view={view} />
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
      )}
      <ContentFooter />
      {review && (
        <CommitReview
          onClose={() => {
            setReview(false);
          }}
          onDiscard={discard}
        />
      )}
    </main>
  );
});
