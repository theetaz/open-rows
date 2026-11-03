// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { transferTabs } from '@openrows/ipc';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { useMoveCommand } from '../commands';
import type { QueryTabs } from '../query/useQueryTabs';
import type { QueryRuns } from '../query/useQueryRuns';
import type { TableViews } from '../table/useTableViews';
import { liveText } from '../query/liveText';
import { moveImpact, transferPayload } from './transfer';
import type { MoveImpact } from './transfer';
import { t, tn } from './messages';

type Input = { tabs: QueryTabs; tables: TableViews; runs: QueryRuns };

/** A move waiting for the user to confirm what happens to staged work. */
type Request = { id: string; target: string | null; impact: MoveImpact };

/** Moving tabs to another window, and the sheet that confirms it. */
type TabMoves = {
  /** Move a tab to window `target`, or to a new window when `null`. */
  move: (id: string, target: string | null) => void;
  sheet: React.JSX.Element | null;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * Move tabs between windows (FR-APP-03). A tab with a running commit stays;
 * staged changes are explained first. Table-row edits stay in the local
 * journal, which the table offers to restore in the other window.
 */
export function useTabMoves(
  input: Input,
  connectionId: string | undefined,
  database: string,
): TabMoves {
  const active =
    connectionId && database
      ? input.tabs.activeFor({ connectionId, database })?.id
      : undefined;
  const [request, setRequest] = useState<Request | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const send = (id: string, target: string | null): void => {
    const tab = input.tabs.state.tabs.find((item) => item.id === id);
    if (!tab) return;
    const sql = liveText(id) ?? tab.sql;
    transferTabs(target, transferPayload([{ ...tab, sql }])).then(
      () => {
        // One batch: the tab unmounts with its views, so the journal stays.
        input.tables.release(id);
        input.runs.release(id);
        input.tabs.close(id, { discard: true });
      },
      (error: unknown) => {
        setFailure(message(error));
      },
    );
  };
  const move = (id: string, target: string | null): void => {
    const tab = input.tabs.state.tabs.find((item) => item.id === id);
    if (!tab) return;
    const impact = moveImpact(tab, input.tables.entryOf(id));
    if (impact.pending || impact.journaled || impact.lost)
      setRequest({ id, target, impact });
    else send(id, target);
  };
  useMoveCommand(() => {
    if (active) move(active, null);
  });
  const dismiss = (): void => {
    setRequest(null);
    setFailure(null);
  };
  if (failure !== null)
    return {
      move,
      sheet: (
        <Sheet title={t('tabWasNot')} size="alert" onClose={dismiss}>
          <p role="alert">{failure}</p>
          <SheetActions>
            <Button variant="primary" onClick={dismiss}>
              {t('ok')}
            </Button>
          </SheetActions>
        </Sheet>
      ),
    };
  return {
    move,
    sheet: request && (
      <MoveSheet
        impact={request.impact}
        onKeep={dismiss}
        onMove={() => {
          setRequest(null);
          send(request.id, request.target);
        }}
      />
    ),
  };
}

function MoveSheet(props: {
  impact: MoveImpact;
  onKeep: () => void;
  onMove: () => void;
}): React.JSX.Element {
  const { impact } = props;
  return (
    <Sheet
      title={impact.pending ? t('commitProgress') : t('moveTabChanges')}
      size="alert"
      onClose={props.onKeep}
    >
      <p>{moveText(impact)}</p>
      <SheetActions>
        <Button onClick={props.onKeep}>{t('keepTabHere')}</Button>
        <Button
          variant="primary"
          disabled={impact.pending}
          onClick={props.onMove}
        >
          {t('moveTab')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}

/** What the move keeps and what it discards; no database write happens. */
export function moveText(impact: MoveImpact): string {
  if (impact.pending) return t('keepTabHereUntil');
  const parts: string[] = [];
  if (impact.journaled) parts.push(tn('journaledEdits', impact.journaled));
  if (impact.lost) parts.push(tn('lostEdits', impact.lost));
  return [...parts, t('databaseNotChanged')].join(' ');
}
