// SPDX-License-Identifier: Apache-2.0
import { Palette } from '@openrows/ui/palette';
import { OpenDatabasePanel, QuickOpen } from '../schema';
import type { Catalog, ObjectRef } from '../schema';
import { COMMANDS, shortcutText, useBindings } from '../commands';
export type PaletteMode = 'database' | 'quick' | 'commands';
/** Open database panel (⌘K), quick open (⌘P) and the command palette (⌘⇧P). */
export function WorkspacePalettes(props: {
  mode: PaletteMode;
  catalog: Catalog;
  onDatabase: (name: string) => void;
  onObject: (object: ObjectRef) => void;
  onCommand: (id: string) => void;
  onClose: () => void;
  /** Commands the active connection cannot run (e.g. no process list). */
  hidden?: readonly string[];
}): React.JSX.Element {
  if (props.mode === 'database')
    return (
      <OpenDatabasePanel
        catalog={props.catalog}
        onOpen={props.onDatabase}
        onClose={props.onClose}
      />
    );
  if (props.mode === 'commands') return <CommandPalette {...props} />;
  return (
    <QuickOpen
      catalog={props.catalog}
      onChoose={props.onObject}
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
  const hidden = new Set(props.hidden);
  const shown = COMMANDS.filter(({ id }) => !hidden.has(id));
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
