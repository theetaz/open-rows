// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import { runBoardWidget } from '@openrows/ipc';
import type {
  BoardDto,
  BoardRun,
  BoardRunReply,
  MonitorTarget,
  ParamValue,
} from '@openrows/ipc';

export type WidgetRun =
  | { status: 'running' }
  | { status: 'done'; reply: BoardRunReply; atMs: number }
  | { status: 'error'; message: string };

export type BoardRuns = {
  runs: Record<string, WidgetRun>;
  /** Interval refresh is live after an explicit run, until stopped. */
  live: boolean;
  note: string | null;
  run: () => void;
  stop: () => void;
};

function message(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error)
    return String(error.message);
  return String(error);
}

function runWidget(request: BoardRun): Promise<WidgetRun> {
  return runBoardWidget(request).then(
    (reply) => ({ status: 'done', reply, atMs: Date.now() }),
    (error: unknown) => ({ status: 'error', message: message(error) }),
  );
}

/**
 * Widgets run only on an explicit Run; an approved board with an interval
 * then refreshes until stopped, closed, switched, edited or locked.
 */
export function useBoardRuns(
  board: BoardDto | null,
  target: MonitorTarget,
  values: ParamValue[],
  active: boolean,
): BoardRuns {
  const [runs, setRuns] = useState<Record<string, WidgetRun>>({});
  const [live, setLive] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const generation = useRef(0);
  const { profileId, database } = target;
  const stop = useCallback(() => {
    generation.current += 1;
    setLive(false);
  }, []);
  const runAll = useCallback(
    async (automatic: boolean): Promise<void> => {
      if (!board?.reviewed || !active) return;
      const current = generation.current;
      for (const widget of board.board.widgets) {
        if (generation.current !== current) return;
        setRuns((all) => ({ ...all, [widget.id]: { status: 'running' } }));
        const request = {
          target: { profileId, database },
          boardId: board.id,
          widgetId: widget.id,
          values,
          automatic,
        };
        const result = await runWidget(request);
        if (generation.current !== current) return;
        setRuns((all) => ({ ...all, [widget.id]: result }));
        if (result.status === 'done' && result.reply.readOnly !== 'enforced') {
          setNote(
            'This engine cannot enforce read-only execution: interval refresh is off; run manually.',
          );
          setLive(false);
        }
      }
    },
    [board, active, profileId, database, values],
  );
  const scope = [
    board?.id,
    board?.updatedAtMs,
    board?.reviewed,
    active,
    profileId,
    database,
  ].join('|');
  const [seen, setSeen] = useState(scope);
  if (seen !== scope) {
    setSeen(scope);
    setLive(false);
    setRuns({});
    setNote(null);
  }
  useEffect(() => {
    generation.current += 1;
  }, [scope]);
  const seconds = board?.board.refreshSecs ?? null;
  useEffect(() => {
    if (!live || !seconds || !active) return;
    const timer = window.setInterval(() => {
      void runAll(true);
    }, seconds * 1_000);
    return () => {
      window.clearInterval(timer);
    };
  }, [live, seconds, active, runAll]);
  const run = useCallback(() => {
    generation.current += 1;
    setNote(null);
    setLive(Boolean(seconds));
    void runAll(false);
  }, [runAll, seconds]);
  return { runs, live, note, run, stop };
}
