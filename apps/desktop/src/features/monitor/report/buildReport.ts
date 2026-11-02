// SPDX-License-Identifier: Apache-2.0
// Assemble report documents for the Monitor header and for a board.
import { monitorHistory, monitorIncidents } from '@openrows/ipc';
import type { BoardDto, MonitorTarget } from '@openrows/ipc';
import type { WidgetRun } from '../boards/useBoardRuns';
import type { MonitorData, Point } from '../monitorData';
import { boardRange, boardSections } from './boardReport';
import type { Connection } from './connection';
import { validateReport } from './validate';
import { REPORT_SCHEMA, REPORT_VERSION } from './model';
import type { Report, ReportSection } from './model';
import { monitorSections } from './monitorReport';
import type { MonitorPart, ReportRange } from './monitorReport';

function report(
  title: string,
  connection: Connection,
  range: Report['range'],
  sections: ReportSection[],
): Report {
  return validateReport({
    schema: REPORT_SCHEMA,
    version: REPORT_VERSION,
    title,
    generatedMs: Date.now(),
    connection,
    range,
    sections,
  });
}

const message = (failure: unknown): string =>
  failure instanceof Error ? failure.message : String(failure);

/** Stored aggregates of the last `hours`, read like the history view reads them. */
async function historyPoints(
  target: MonitorTarget,
  range: ReportRange,
): Promise<Point[]> {
  const hours = Math.round((range.toMs - range.fromMs) / 3_600_000);
  const observations = await monitorHistory(target, Math.min(24, hours * 2));
  return observations.flatMap((point) =>
    point.collectedMs === null || point.collectedMs < range.fromMs
      ? []
      : [{ ...point, collectedMs: point.collectedMs }],
  );
}

/** The live window, or `hours` of stored history, as a report. */
export async function monitorReport(request: {
  target: MonitorTarget;
  connection: Connection;
  data: MonitorData | null;
  hours: number;
  parts: ReadonlySet<MonitorPart>;
}): Promise<Report> {
  const { target, connection, data, hours } = request;
  const toMs = Date.now();
  const range: ReportRange = hours
    ? {
        fromMs: toMs - hours * 3_600_000,
        toMs,
        label: `History · ${String(hours)} h`,
        history: true,
      }
    : {
        fromMs: toMs - 600_000,
        toMs,
        label: 'Live · 10 minutes',
        history: false,
      };
  const points = hours
    ? await historyPoints(target, range)
    : (data?.points ?? []);
  const incidents = request.parts.has('incidents')
    ? await monitorIncidents(target).then(
        (reply) => ({ reply, error: null }),
        (failure: unknown) => ({ reply: null, error: message(failure) }),
      )
    : { reply: null, error: null };
  const sections = monitorSections(
    { engine: connection.engine, data, points, range, incidents },
    request.parts,
  );
  return report(
    `Monitor report · ${connection.database}`,
    connection,
    range,
    sections,
  );
}

/** The latest widget results of a board as a report. */
export function boardReport(request: {
  connection: Connection;
  board: BoardDto;
  runs: Readonly<Record<string, WidgetRun>>;
  include: ReadonlySet<string>;
}): Report {
  const { board, runs } = request;
  return report(
    `Board report · ${board.board.name}`,
    request.connection,
    { ...boardRange(runs), label: 'Latest widget runs' },
    boardSections(board, runs, request.include),
  );
}
