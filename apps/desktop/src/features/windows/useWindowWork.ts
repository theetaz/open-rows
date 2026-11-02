// SPDX-License-Identifier: Apache-2.0
import type { Connections } from '../connections';
import type { QueryTabs } from '../query';
import { useWindowTitle } from './useWindowTitle';
import { useWindowTransfers } from './useWindowTransfers';

/**
 * This window's part in multi-window work (FR-APP-03): receive tabs moved
 * here and name the native window after its active connection.
 */
export function useWindowWork(input: {
  tabs: QueryTabs;
  connections: Connections;
  database: string;
  enabled: boolean;
}): void {
  useWindowTransfers(input);
  useWindowTitle(input.connections.active?.name, input.database);
}
