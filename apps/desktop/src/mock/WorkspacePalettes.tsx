// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Palette } from '@openrows/ui/palette';
import { databases } from './data/databases';
import type { Database, DatabaseObject } from './data/databases';
import { paletteCommands } from './data/commands';
export type PaletteMode = 'database' | 'quick' | 'commands';
const glyphs: Record<DatabaseObject['kind'], string> = {
  Tables: '▦',
  Views: '▤',
  Functions: 'ƒ',
  Triggers: '↯',
  Sequences: '≡',
};
/** Mock catalog adapters for the shared palette. */
export function WorkspacePalettes(props: {
  mode: PaletteMode;
  catalog: Database;
  opened: readonly string[];
  onDatabase: (name: string) => void;
  onObject: (object: DatabaseObject) => void;
  onCommand: (id: string) => void;
  onClose: () => void;
}): React.JSX.Element {
  const [system, setSystem] = useState(false);
  if (props.mode === 'database')
    return (
      <Palette
        title="Switch database"
        onClose={props.onClose}
        onChoose={props.onDatabase}
        items={databases
          .filter((database) => system || !database.system)
          .map((database) => ({
            id: database.name,
            label: database.name,
            glyph: '▱',
            detail: props.opened.includes(database.name)
              ? 'Already open'
              : 'Available',
          }))}
        footer={
          <label>
            <input
              type="checkbox"
              checked={system}
              onChange={(event) => {
                setSystem(event.target.checked);
              }}
            />
            Show system databases
          </label>
        }
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
  const objects = props.catalog.objects.filter((object) =>
    ['Tables', 'Views', 'Functions'].includes(object.kind),
  );
  return (
    <Palette
      title="Quick open"
      onClose={props.onClose}
      onChoose={(id) => {
        const object = objects.find(
          (object) => `${object.schema}.${object.name}` === id,
        );
        if (object) props.onObject(object);
      }}
      items={objects.map((object) => ({
        id: `${object.schema}.${object.name}`,
        label: object.name,
        glyph: glyphs[object.kind],
        searchText: `${object.name} ${object.schema}`,
        detail: `${object.kind} · ${object.schema} · ${object.count ?? '—'} rows (estimate)`,
      }))}
    />
  );
}
