// SPDX-License-Identifier: Apache-2.0
import { ConnectionSwitcher } from './ConnectionSwitcher';
import { useSecurity } from '../security/useSecurity';
import type { Connections } from '../connections';
import { Palette } from '@openrows/ui/palette';
import { OpenDatabasePanel, QuickOpen } from '../schema';
import type { Catalog, ObjectRef } from '../schema';
import { COMMANDS, shortcutText, useBindings } from '../commands';
export type PaletteMode = 'database' | 'quick' | 'commands' | 'connection';
/** Open database panel (⌘K), quick open (⌘P) and the command palette (⌘⇧P). */
export function WorkspacePalettes(props: {
  mode: PaletteMode;
  connections: Connections;
  onSavedQuery: (sql: string, title: string) => void;
  onProjectQuery: (path: string) => void;
  catalog: Catalog;
  onDatabase: (name: string) => void;
  onObject: (object: ObjectRef) => void;
  onCommand: (id: string) => void;
  onClose: () => void;
  /** Commands the active connection cannot run (e.g. no process list). */
  hidden?: readonly string[];
  /** Create a database from the Open database panel, where supported. */
  onNewDatabase?: (() => void) | undefined;
}): React.JSX.Element {
  if (props.mode === 'connection')
    return (
      <ConnectionSwitcher
        connections={props.connections}
        onClose={props.onClose}
      />
    );
  if (props.mode === 'database')
    return (
      <OpenDatabasePanel
        catalog={props.catalog}
        onOpen={props.onDatabase}
        onClose={props.onClose}
        onNew={props.onNewDatabase}
      />
    );
  if (props.mode === 'commands') return <CommandPalette {...props} />;
  return (
    <QuickOpen
      catalog={props.catalog}
      onChoose={props.onObject}
      onSavedQuery={props.onSavedQuery}
      onProjectQuery={props.onProjectQuery}
      onClose={props.onClose}
    />
  );
}

/** Every command with its current shortcut. */
function CommandPalette(props: {
  onCommand: (id: string) => void;
  onClose: () => void;
  hidden?: readonly string[] | undefined;
}): React.JSX.Element {
  const bindings = useBindings();
  const security = useSecurity();
  const hidden = new Set(props.hidden);
  const shown = COMMANDS.filter(
    ({ id }) => !hidden.has(id) && (id !== 'lock-app' || security.available),
  );
  const items = shown.map(({ id, label }) => ({
    id,
    label,
    shortcut: shortcutText(bindings, id),
  }));
  return (
    <Palette
      title="Command palette"
      items={items}
      onClose={props.onClose}
      onChoose={props.onCommand}
    />
  );
}
