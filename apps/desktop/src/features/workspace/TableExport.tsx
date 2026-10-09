// SPDX-License-Identifier: Apache-2.0
import { ExportSheet, tableExport } from '../export';
import { sessionOf } from '../connections';
import type { Connections } from '../connections';
import type { QueryTabs, TabScope } from '../query';

/** File ▸ Export Table…: the export sheet for the active table tab's table. */
export function TableExport(props: {
  connections: Connections;
  tabs: QueryTabs;
  scope: TabScope | undefined;
  onClose: () => void;
}): React.JSX.Element | null {
  const profile = props.connections.active;
  const tab = props.scope ? props.tabs.activeFor(props.scope) : undefined;
  const table = tab?.kind === 'table' ? tab.table : undefined;
  if (!profile || !table) return null;
  const sessionId = sessionOf(
    props.connections.statusOf(profile.id),
    table.database,
  );
  if (!sessionId) return null;
  return (
    <ExportSheet
      {...tableExport({ ...table, profileId: profile.id, sessionId })}
      onClose={props.onClose}
    />
  );
}
