// SPDX-License-Identifier: Apache-2.0
import { ConnectionRail, connectionActions } from '../connections';
import type { Connections } from '../connections';
import type { TabScope } from '../query';
import type { JobActions } from './JobActions';

/** The connection list: environment groups, servers and their opened databases. */
export function WorkspaceRail(props: {
  connections: Connections;
  /** A connection or one of its databases was chosen. */
  onChosen?: (() => void) | undefined;
  /** Open the process list of a connection's database. */
  onProcesses?: ((scope: TabScope) => void) | undefined;
  /** Open a database Monitor. */
  onMonitor?: ((scope: TabScope) => void) | undefined;
  /** Run a `.sql` file on a connection's database. */
  onRunFile?: ((database: string, profileId: string) => void) | undefined;
  jobs?: JobActions;
}): React.JSX.Element {
  const { connections } = props;
  return (
    <ConnectionRail
      profiles={connections.profiles}
      activeId={connections.active?.id}
      statusOf={connections.statusOf}
      onSelect={(id) => {
        connections.select(id);
        props.onChosen?.();
      }}
      databases={connections.databases}
      restoredDatabases={connections.restoredDatabases}
      database={connections.database}
      onDatabase={connections.openDatabase}
      onCloseDatabase={connections.closeDatabase}
      onProfileDatabase={(id, name) => {
        connections.select(id, name);
        props.onChosen?.();
      }}
      onCloseProfileDatabase={connections.closeProfileDatabase}
      onMonitor={(id, name) => {
        connections.select(id, name);
        props.onMonitor?.({ connectionId: id, database: name });
      }}
      onProcesses={(id, name) => {
        connections.select(id, name);
        props.onProcesses?.({ connectionId: id, database: name });
      }}
      onBackup={(id, name) => {
        props.jobs?.backup(name, undefined, id);
      }}
      onRestore={(id, name) => {
        props.jobs?.restore(name, undefined, id);
      }}
      onRunFile={(id, name) => {
        connections.select(id, name);
        props.onRunFile?.(name, id);
      }}
      actionsFor={(profile) => connectionActions(profile, connections)}
    />
  );
}
