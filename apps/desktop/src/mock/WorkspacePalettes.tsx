// SPDX-License-Identifier: Apache-2.0
import { Palette } from '@openrows/ui/palette';
import { DatabaseSwitcher, QuickOpen } from '../features/schema';
import type { Catalog, ObjectRef } from '../features/schema';
import { paletteCommands } from './data/commands';
export type PaletteMode = 'database' | 'quick' | 'commands';
/** Real catalog palettes (⌘K, ⌘P) and the sample command palette (⌘⇧P). */
export function WorkspacePalettes(props: {
  mode: PaletteMode;
  catalog: Catalog;
  onDatabase: (name: string) => void;
  onObject: (object: ObjectRef) => void;
  onCommand: (id: string) => void;
  onClose: () => void;
}): React.JSX.Element {
  if (props.mode === 'database')
    return (
      <DatabaseSwitcher
        catalog={props.catalog}
        onChoose={props.onDatabase}
        onClose={props.onClose}
      />
    );
  if (props.mode === 'commands')
    return (
      <Palette
        title="Command palette"
        items={paletteCommands}
        onClose={props.onClose}
        onChoose={props.onCommand}
      />
    );
  return (
    <QuickOpen
      catalog={props.catalog}
      onChoose={props.onObject}
      onClose={props.onClose}
    />
  );
}
