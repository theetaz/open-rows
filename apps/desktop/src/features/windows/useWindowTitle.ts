// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { setWindowTitle } from '@openrows/ipc';

/** The native title of a workspace window showing `connection` and `database`. */
function windowTitle(connection: string | undefined, database: string): string {
  if (!connection) return 'OpenRows';
  return database ? `${connection} — ${database}` : connection;
}

/**
 * Name the native window after its active connection, so the Window menu,
 * Mission Control and Move to Window can tell windows apart. The title bar
 * itself stays hidden.
 */
export function useWindowTitle(
  connection: string | undefined,
  database: string,
): void {
  const title = windowTitle(connection, database);
  useEffect(() => {
    setWindowTitle(title).catch(() => undefined);
  }, [title]);
}
