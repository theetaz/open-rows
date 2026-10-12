// SPDX-License-Identifier: Apache-2.0
import type { Metric } from './metrics';
/** Own OpenRows request totals; other applications are never visible. */
const OWN_METRICS: readonly Metric[] = [
  {
    title: 'Own requests/s',
    names: ['requests'],
    unit: '/s',
    scope: 'Own requests',
  },
  {
    title: 'Own errors/s',
    names: ['request_errors'],
    unit: '/s',
    scope: 'Own requests',
  },
  {
    title: 'Own busy time',
    names: ['request_ms'],
    unit: 'ms/s',
    scope: 'Own requests',
  },
  {
    title: 'Rows returned/s',
    names: ['rows_returned'],
    unit: '/s',
    scope: 'Own requests',
  },
];
/** SQLite/libSQL file: own requests plus cheap PRAGMA and file-size gauges. */
export const SQLITE_METRICS: readonly Metric[] = [
  ...OWN_METRICS,
  { title: 'File size', names: ['file_bytes'], unit: 'B', scope: 'File' },
  { title: 'Pages', names: ['page_count'], unit: 'count', scope: 'File' },
  {
    title: 'Free pages',
    names: ['freelist_count'],
    unit: 'count',
    scope: 'File',
  },
  { title: 'WAL size', names: ['wal_bytes'], unit: 'B', scope: 'File' },
  { title: 'WAL frames', names: ['wal_frames'], unit: 'count', scope: 'File' },
  {
    title: 'Checkpoint backlog',
    names: ['checkpoint_backlog'],
    unit: 'frames',
    scope: 'File',
  },
];
/** Remote libSQL over Hrana: own requests and a measured round trip only. */
export const LIBSQL_METRICS: readonly Metric[] = [
  {
    title: 'Round trip',
    names: ['round_trip_ms'],
    unit: 'ms',
    scope: 'Own requests',
  },
  ...OWN_METRICS,
];
