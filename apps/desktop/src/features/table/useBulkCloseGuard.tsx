// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { QueryTabs } from '../query/useQueryTabs';
import type { QueryRuns } from '../query/useQueryRuns';
import type { TableViews } from './useTableViews';

type Input = { tabs: QueryTabs; tables: TableViews; runs: QueryRuns };

/** One impact review for a whole tab group; commits prevent the entire close. */
export function useBulkCloseGuard(input: Input): React.JSX.Element | null {
  const { tabs, tables, runs } = input;
  const [closing, setClosing] = useState<readonly string[] | null>(null);
  useEffect(
    () =>
      tabs.installBulkCloseGuard?.((ids) => {
        if (ids.some((id) => impact(input, id).dirty)) {
          setClosing(ids);
          return false;
        }
        ids.forEach((id) => {
          tables.release(id);
          runs.release(id);
        });
        return true;
      }),
    [tabs, tables, runs, input],
  );
  if (!closing) return null;
  const affected = closing.map((id) => ({ id, ...impact(input, id) }));
  const pending = affected.some((tab) => tab.pending);
  const dismiss = (): void => {
    setClosing(null);
  };
  const confirm = (): void => {
    if (closing.some((id) => impact(input, id).pending)) return;
    closing.forEach((id) => {
      tables.release(id);
      runs.release(id);
    });
    tabs.closeMany?.(closing, { discard: true });
    dismiss();
  };
  return (
    <Sheet title="Review closing tabs" onClose={dismiss}>
      <p>
        Close {closing.length} tabs? Unsaved SQL and uncommitted changes listed
        below will be discarded. No database write occurs.
      </p>
      <ul>
        {affected.map((tab) => (
          <li key={tab.id}>
            {tab.title}:{' '}
            {tab.pending
              ? 'commit in progress'
              : tab.dirty
                ? 'unsaved changes'
                : 'no pending changes'}
          </li>
        ))}
      </ul>
      {pending && (
        <p role="alert">
          Wait for commits to finish before closing these tabs.
        </p>
      )}
      <SheetActions>
        <Button onClick={dismiss}>Keep tabs</Button>
        <Button variant="primary" disabled={pending} onClick={confirm}>
          Discard and close tabs
        </Button>
      </SheetActions>
    </Sheet>
  );
}

function impact(
  { tabs, tables }: Input,
  id: string,
): { title: string; dirty: boolean; pending: boolean } {
  const tab = tabs.state.tabs.find((item) => item.id === id);
  const { edits, schema } = tables.entryOf(id);
  const pending = edits.pending || schema.pending;
  const sqlDirty = tab?.kind === 'query' && tab.sql !== (tab.file?.saved ?? '');
  return {
    title: tab?.title ?? id,
    pending,
    dirty: Boolean(
      sqlDirty || pending || edits.current.length || schema.current.length,
    ),
  };
}
