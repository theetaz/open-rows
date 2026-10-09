// SPDX-License-Identifier: Apache-2.0
/**
 * Back up and restore databases with the installed `pg_dump`, `pg_restore`,
 * `psql`, `mysqldump` and `mysql` (a native copy for SQLite) as background
 * jobs (M2-14).
 */
export { DumpHost } from './DumpHost';
export { openBackup, pickRestoreFor } from './openDump';
export type { DumpTarget } from './dumpStore';
