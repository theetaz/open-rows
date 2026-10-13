// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { useBulkCloseGuard } from './useBulkCloseGuard';
import { closeWindow, onWindowClose } from '@openrows/ipc';
import type { QueryTabs } from '../query/useQueryTabs';
import type { QueryRuns } from '../query/useQueryRuns';
import type { TableViews } from './useTableViews';

/** What is being closed: one tab, or the window (and with it the app). */
type Closing = { kind: 'tab'; id: string } | { kind: 'window' };

/**
 * Guard closing a tab or the window against losing staged or committing
 * edits. Table-tab edits are journaled locally; query-result edits are
 * held in memory. A commit in progress must finish before closing.
 */
export function useTableCloseGuard({
  tabs,
  tables,
  runs,
}: {
  tabs: QueryTabs;
  tables: TableViews;
  runs: QueryRuns;
}): React.JSX.Element | null {
  const bulkReview = useBulkCloseGuard({ tabs, tables, runs });
  const [closing, setClosing] = useState<Closing | null>(null);
  useEffect(
    () =>
      tabs.installCloseGuard?.((id, preview) => {
        const { edits, schema } = tables.entryOf(id);
        const staged = edits.current.length + schema.current.length;
        if (staged || edits.pending || schema.pending) {
          if (!preview) setClosing({ kind: 'tab', id });
          return false;
        }
        tables.release(id);
        runs.release(id);
        return true;
      }),
    [tabs, tables, runs],
  );
  useWindowGuard(tables, () => {
    setClosing({ kind: 'window' });
  });
  if (!closing) return bulkReview;
  const state =
    closing.kind === 'tab' ? tabState(tables, closing.id) : windowState(tables);
  const confirm = (): void => {
    setClosing(null);
    if (closing.kind === 'window') {
      closeWindow().catch(() => undefined);
      return;
    }
    tables.release(closing.id);
    runs.release(closing.id);
    tabs.close(closing.id, { discard: true });
  };
  return (
    <CloseSheet
      closing={closing}
      state={state}
      onKeep={() => {
        setClosing(null);
      }}
      onConfirm={confirm}
    />
  );
}

type GuardState = { pending: boolean; uncertain: boolean; staged: number };

function tabState(tables: TableViews, id: string): GuardState {
  const { edits, schema } = tables.entryOf(id);
  return {
    pending: edits.pending || schema.pending,
    uncertain: Boolean(edits.uncertain || schema.uncertain),
    staged: edits.current.length + schema.current.length,
  };
}

function windowState(tables: TableViews): GuardState {
  return tables.summary();
}

/** The native close button and ⌘W ask first while anything is staged or committing. */
function useWindowGuard(tables: TableViews, ask: () => void): void {
  const latest = useRef({ tables, ask });
  useEffect(() => {
    latest.current = { tables, ask };
  });
  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent): void => {
      if (latest.current.tables.hasChanges()) event.preventDefault();
    };
    window.addEventListener('beforeunload', beforeUnload);
    let unlisten: (() => void) | undefined;
    let alive = true;
    onWindowClose(() => {
      if (!latest.current.tables.hasChanges()) return true;
      latest.current.ask();
      return false;
    }).then(
      (stop) => {
        if (alive) unlisten = stop;
        else stop();
      },
      () => undefined,
    );
    return () => {
      alive = false;
      unlisten?.();
      window.removeEventListener('beforeunload', beforeUnload);
    };
  }, []);
}

function CloseSheet({
  closing,
  state,
  onKeep,
  onConfirm,
}: {
  closing: Closing;
  state: GuardState;
  onKeep: () => void;
  onConfirm: () => void;
}): React.JSX.Element {
  const tab = closing.kind === 'tab';
  const title = state.pending
    ? 'Commit in progress'
    : tab
      ? 'Discard staged changes?'
      : 'Quit with staged changes?';
  return (
    <Sheet title={title} size="alert" onClose={onKeep}>
      <p>{message(closing, state)}</p>
      <SheetActions>
        <Button onClick={onKeep}>{tab ? 'Keep tab' : 'Keep editing'}</Button>
        <Button disabled={state.pending} variant="primary" onClick={onConfirm}>
          {tab ? 'Discard changes' : 'Quit'}
        </Button>
      </SheetActions>
    </Sheet>
  );
}

function message(closing: Closing, state: GuardState): string {
  if (state.pending)
    return closing.kind === 'tab'
      ? 'Keep this tab open until the commit finishes.'
      : 'Wait until the commit finishes; quitting now could leave its outcome unknown.';
  if (state.uncertain)
    return closing.kind === 'tab'
      ? 'The commit outcome is unknown. Closing this tab discards the staged copy; reload the table to check what was saved.'
      : 'The commit outcome is unknown. Reload the table to check what was saved before quitting.';
  if (closing.kind === 'tab')
    return 'Closing this tab will discard its uncommitted changes. Database data has not been changed.';
  return `${String(state.staged)} staged ${state.staged === 1 ? 'change has' : 'changes have'} not been committed. Table-tab row edits are kept locally and offered for restore when the table is opened again; query-result edits and structure changes are lost.`;
}
