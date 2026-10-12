// SPDX-License-Identifier: Apache-2.0
// The two "Export report…" entry points: the Monitor header and a board.
import { useCallback, useState } from 'react';
import type { BoardDto, MonitorTarget } from '@openrows/ipc';
import type { WidgetRun } from '../boards/useBoardRuns';
import type { MonitorData } from '../monitorData';
import { boardReport, monitorReport } from './buildReport';
import type { Connection } from './connection';
import type { MonitorPart } from './monitorReport';
import { ReportDialog } from './ReportDialog';

const MONITOR_PARTS: readonly { value: MonitorPart; label: string }[] = [
  { value: 'summary', label: 'Summary and source status' },
  { value: 'metrics', label: 'Metric charts' },
  { value: 'queries', label: 'Top queries' },
  { value: 'incidents', label: 'Incidents' },
];

function monitorRanges(
  history: boolean,
  hours: number,
): { value: string; label: string }[] {
  const ranges = [
    { value: '0', label: 'Live · 10 minutes' },
    ...(history
      ? [1, 6, 24].map((value) => ({
          value: String(value),
          label: `History · ${String(value)} h`,
        }))
      : []),
  ];
  const current = ranges.find((range) => range.value === String(hours));
  return current ? [current, ...ranges.filter((r) => r !== current)] : ranges;
}

const isPart = (value: string): value is MonitorPart =>
  MONITOR_PARTS.some((part) => part.value === value);

/** Report of the live window or stored history, from the data shown at open. */
export function MonitorReportDialog(props: {
  target: MonitorTarget;
  connection: Connection;
  data: MonitorData | null;
  history: { enabled: boolean; hours: number };
  onClose: () => void;
}): React.JSX.Element {
  // A snapshot: parents re-render every tick; the report must not.
  const [{ target, connection, data }] = useState(props);
  const build = useCallback(
    (range: string, parts: ReadonlySet<string>) =>
      monitorReport({
        target,
        connection,
        data,
        hours: Number(range),
        parts: new Set([...parts].filter(isPart)),
      }),
    [target, connection, data],
  );
  return (
    <ReportDialog
      ranges={monitorRanges(props.history.enabled, props.history.hours)}
      parts={MONITOR_PARTS}
      build={build}
      onClose={props.onClose}
    />
  );
}

/** Report of a board's latest widget results, as shown at open. */
export function BoardReportDialog(props: {
  connection: Connection;
  board: BoardDto;
  runs: Readonly<Record<string, WidgetRun>>;
  onClose: () => void;
}): React.JSX.Element {
  const [{ board, connection, runs }] = useState(props);
  const build = useCallback(
    (_range: string, include: ReadonlySet<string>) =>
      Promise.resolve(boardReport({ connection, board, runs, include })),
    [connection, board, runs],
  );
  return (
    <ReportDialog
      parts={[
        { value: 'summary', label: 'Summary' },
        ...board.board.widgets.map((w) => ({ value: w.id, label: w.title })),
      ]}
      build={build}
      onClose={props.onClose}
    />
  );
}
