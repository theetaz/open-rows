// SPDX-License-Identifier: Apache-2.0
import { tableFromArrays, tableToIPC } from 'apache-arrow';
import type { MonitorReply, Source, SourceStatus } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';

/** Synthetic profile and source metadata used by monitor regression fixtures. */
export const monitorProfile = profile('monitor-test', {
  id: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
});
/** Complete permitted status fixture; tests replace state without inventing values. */
export function monitorReply(): MonitorReply {
  const sources: Source[] = [
    'activity',
    'database',
    'tables',
    'indexes',
    'replication',
    'statements',
    'io',
  ];
  return {
    generation: '1',
    capabilities: sources.map((source) => ({
      source,
      state: { kind: 'ok' },
      scope:
        source === 'activity' || source === 'replication'
          ? 'server'
          : 'database',
      intervalMs: source === 'activity' ? 5000 : 60000,
      guidance:
        source === 'statements' ? 'CREATE EXTENSION pg_stat_statements;' : null,
      coverage: 'Bounded fixture coverage',
    })),
    sources: sources.map((source): SourceStatus => ({
      source,
      state: { kind: 'ok' },
      collectedMs: 1000,
      lastSuccessMs: 1000,
      intervalMs: source === 'activity' ? 5000 : 60000,
      durationMs: 1,
      sequence: 1,
      partial: false,
    })),
  };
}
/** Arrow payloads exercise the actual decoder used by the monitor view. */
export function monitorBytes(source: Source | null): ArrayBuffer {
  const data: Record<string, (string | number | null)[]> =
    source === null
      ? {
          metric: [
            'sessions:active',
            'Database:1:xact_commit',
            'Database:1:xact_rollback',
            'Database:cache_hit_ratio',
          ],
          collected_ms: [1000, 1000, 1000, 1000],
          value: [2, null, null, null],
          state: ['ok', 'counterReset', 'gap', 'unknown'],
          unit: ['count', 'count/s', 'count/s', 'fraction'],
        }
      : source === 'activity'
        ? {
            pid: ['7', '8'],
            state: ['idle in transaction', 'active'],
            wait: [null, 'Lock:transactionid'],
            age_ms: [50, 30],
            transaction_ms: [80, 40],
            blockers: ['', '7'],
            query: [
              'UPDATE fixture SET value = ?',
              'UPDATE fixture SET value = ?',
            ],
            user: ['test', 'test'],
            database: ['orders', 'orders'],
            application: ['fixture', 'fixture'],
            query_start: [null, null],
          }
        : source === 'statements'
          ? {
              statement_id: ['1'],
              query: ['SELECT ?'],
              calls: [2],
              total_ms: [4],
              window_mean_ms: [2],
              max_since_reset_ms: [5],
              interval_state: ['ok'],
              rows: [2],
              temp_blks_written: [null],
            }
          : source === 'tables'
            ? {
                relation_id: ['1'],
                table: ['fixture'],
                seq_scan: [2],
                idx_scan: [1],
                dead_tuples: [null],
                last_vacuum: [null],
                interval_state: ['counterReset'],
              }
            : source === 'indexes'
              ? {
                  index_id: ['1'],
                  index: ['fixture_pkey'],
                  idx_scan: [2],
                  interval_state: ['gap'],
                }
              : source === 'replication'
                ? { process_id: ['sender:1'], replay_lag_ms: [null] }
                : {
                    backend_type: ['client backend'],
                    reads: [null],
                    interval_state: ['unknown'],
                  };
  const bytes = tableToIPC(tableFromArrays(data));
  return bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer;
}
