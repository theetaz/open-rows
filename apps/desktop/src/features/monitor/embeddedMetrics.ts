// SPDX-License-Identifier: Apache-2.0
import type { Metric } from './metrics';
import { t } from './messages/lazyMetrics';
/** Own OpenRows request totals; other applications are never visible. */
const OWN_METRICS: readonly Metric[] = [
  {
    title: t('ownRequestsS'),
    names: ['requests'],
    unit: '/s',
    scope: 'Own requests',
  },
  {
    title: t('ownErrorsS'),
    names: ['request_errors'],
    unit: '/s',
    scope: 'Own requests',
  },
  {
    title: t('ownBusyTime'),
    names: ['request_ms'],
    unit: 'ms/s',
    scope: 'Own requests',
  },
  {
    title: t('rowsReturnedS'),
    names: ['rows_returned'],
    unit: '/s',
    scope: 'Own requests',
  },
];
/** SQLite/libSQL file: own requests plus cheap PRAGMA and file-size gauges. */
export const SQLITE_METRICS: readonly Metric[] = [
  ...OWN_METRICS,
  { title: t('fileSize'), names: ['file_bytes'], unit: 'B', scope: 'File' },
  { title: t('pages'), names: ['page_count'], unit: 'count', scope: 'File' },
  {
    title: t('freePages'),
    names: ['freelist_count'],
    unit: 'count',
    scope: 'File',
  },
  { title: t('walSize'), names: ['wal_bytes'], unit: 'B', scope: 'File' },
  {
    title: t('walFrames'),
    names: ['wal_frames'],
    unit: 'count',
    scope: 'File',
  },
  {
    title: t('checkpointBacklog'),
    names: ['checkpoint_backlog'],
    unit: 'frames',
    scope: 'File',
  },
];
/** DuckDB: own requests plus storage, memory and temp gauges of the owning session. */
export const DUCKDB_METRICS: readonly Metric[] = [
  ...OWN_METRICS,
  {
    title: t('databaseSize'),
    names: ['database_bytes'],
    unit: 'B',
    scope: 'Database',
  },
  {
    title: t('usedBlocks'),
    names: ['used_blocks'],
    unit: 'count',
    scope: 'Database',
  },
  {
    title: t('freeBlocks'),
    names: ['free_blocks'],
    unit: 'count',
    scope: 'Database',
  },
  { title: t('walSize'), names: ['wal_bytes'], unit: 'B', scope: 'Database' },
  { title: t('memory'), names: ['memory_bytes'], unit: 'B', scope: 'Engine' },
  {
    title: t('memoryLimit'),
    names: ['memory_limit_bytes'],
    unit: 'B',
    scope: 'Engine',
  },
  {
    title: t('tempFiles'),
    names: ['temp_files'],
    unit: 'count',
    scope: 'Engine',
  },
  { title: t('tempSize'), names: ['temp_bytes'], unit: 'B', scope: 'Engine' },
  { title: t('threads'), names: ['threads'], unit: 'count', scope: 'Engine' },
];
/** Remote libSQL over Hrana: own requests and a measured round trip only. */
export const LIBSQL_METRICS: readonly Metric[] = [
  {
    title: t('roundTrip'),
    names: ['round_trip_ms'],
    unit: 'ms',
    scope: 'Own requests',
  },
  ...OWN_METRICS,
];
