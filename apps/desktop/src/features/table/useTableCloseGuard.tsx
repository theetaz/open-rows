// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { QueryTabs } from '../query/useQueryTabs';
import type { QueryRuns } from '../query/useQueryRuns';
import type { TableViews } from './useTableViews';
/** Guard mouse and keyboard tab closing against losing staged or committing edits. */
export function useTableCloseGuard({
  tabs,
  tables,
  runs,
}: {
  tabs: QueryTabs;
  tables: TableViews;
  runs: QueryRuns;
}): React.JSX.Element | null {
  const [closing, setClosing] = useState<string | null>(null);
  useEffect(
    () =>
      tabs.installCloseGuard?.((id) => {
        const { edits, schema } = tables.entryOf(id);
        const staged = edits.current.length + schema.current.length;
        if (staged || edits.pending || schema.pending) {
          setClosing(id);
          return false;
        }
        tables.release(id);
        runs.release(id);
        return true;
      }),
    [tabs, tables, runs],
  );
  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent): void => {
      if (!tables.hasChanges()) return;
      event.preventDefault();
    };
    window.addEventListener('beforeunload', beforeUnload);
    return () => {
      window.removeEventListener('beforeunload', beforeUnload);
    };
  }, [tables]);
  if (!closing) return null;
  const { edits, schema } = tables.entryOf(closing);
  const pending = edits.pending || schema.pending;
  const uncertain = edits.uncertain || schema.uncertain;
  return (
    <Sheet
      title={pending ? 'Commit in progress' : 'Discard staged changes?'}
      size="alert"
      onClose={() => {
        setClosing(null);
      }}
    >
      <p>
        {pending
          ? 'Keep this tab open until the commit finishes.'
          : uncertain
            ? 'The commit outcome is unknown. Closing this tab discards the staged copy; reload the table to check what was saved.'
            : 'Closing this tab will discard its uncommitted changes. Database data has not been changed.'}
      </p>
      <SheetActions>
        <Button
          onClick={() => {
            setClosing(null);
          }}
        >
          Keep tab
        </Button>
        <Button
          disabled={pending}
          variant="primary"
          onClick={() => {
            tables.release(closing);
            runs.release(closing);
            setClosing(null);
            tabs.close(closing, { discard: true });
          }}
        >
          Discard changes
        </Button>
      </SheetActions>
    </Sheet>
  );
}
