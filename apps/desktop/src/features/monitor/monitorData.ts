// SPDX-License-Identifier: Apache-2.0
import type { MonitorTable } from './monitorArrow';
export { decodeTable } from './monitorArrow';
export type { MonitorTable } from './monitorArrow';
import { monitorWindow, topStatements } from '@openrows/ipc';
import type { MonitorReply, MonitorTarget, Source } from '@openrows/ipc';

/** Decode one binary page outside the UI thread. */
type Decode = (bytes: ArrayBuffer) => Promise<MonitorTable>;
/** One scalar observation; unknown values retain their validity label. */
export type Point = {
  metric: string;
  collectedMs: number;
  value: number | null;
  state: string;
};
/** One atomically presented monitor window. */
export type MonitorData = {
  reply: MonitorReply;
  points: Point[];
  tables: Partial<Record<Source, MonitorTable>>;
};

/** Page every scalar series (64 × 300 maximum); large windows remain bounded. */
async function scalarWindow(
  target: MonitorTarget,
  decode: Decode,
): Promise<Point[]> {
  const points: Point[] = [];
  for (let offset = 0; offset < 19_200; offset += 1000) {
    const page = await decode(
      await monitorWindow({
        target,
        source: null,
        rowStart: offset,
        limit: 1000,
        mean: false,
      }),
    );
    for (const row of page.rows) {
      if (
        typeof row.metric === 'string' &&
        typeof row.collected_ms === 'number'
      )
        points.push({
          metric: row.metric,
          collectedMs: row.collected_ms,
          value: typeof row.value === 'number' ? row.value : null,
          state: String(row.state),
        });
    }
    if (page.rows.length < 1000) break;
  }
  return points;
}

/** Read only source tables whose sequence changed; all replies share a generation. */
export async function readMonitorData(
  target: MonitorTarget,
  reply: MonitorReply,
  previous: MonitorData | null,
  decode: Decode,
): Promise<MonitorData> {
  const tables = { ...(previous?.tables ?? {}) };
  for (const status of reply.sources) {
    const before = previous?.reply.sources.find(
      (source) => source.source === status.source,
    );
    if (status.state.kind !== 'ok') {
      Reflect.deleteProperty(tables, status.source);
      continue;
    }
    if (before?.sequence === status.sequence && tables[status.source]) continue;
    const request = {
      target,
      source: status.source,
      rowStart: 0,
      limit: 1000,
      mean: false,
    };
    tables[status.source] = await decode(
      await (status.source === 'statements'
        ? topStatements(request)
        : monitorWindow(request)),
    );
  }
  return { reply, tables, points: await scalarWindow(target, decode) };
}
