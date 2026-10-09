// SPDX-License-Identifier: Apache-2.0
import type { ProcessDto } from '@openrows/ipc';

/** A synthetic server process; every field can be overridden. */
export function sampleProcess(
  id: number,
  overrides: Partial<ProcessDto> = {},
): ProcessDto {
  return {
    id,
    user: 'app',
    database: 'orders',
    client: '10.0.0.5:51234',
    application: 'psql',
    state: 'active',
    wait: null,
    queryStart: '2026-10-09 10:00:00+00',
    durationMs: 1000,
    query: 'SELECT 1',
    current: false,
    app: false,
    ...overrides,
  };
}
