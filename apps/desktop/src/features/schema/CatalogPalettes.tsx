// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Palette } from '@openrows/ui/palette';
import type { PaletteItem } from '@openrows/ui';
import type { ObjectDto } from '@openrows/ipc';
import { formatEstimate, kindInfo, objectLabel, objectRef } from './catalog';
import type { ObjectRef } from './catalog';
import type { Catalog } from './useCatalog';
import type { Resource } from './useResources';

/** Loading or failure text for a palette whose items are still unknown. */
function statusText(resource: Resource<unknown>, noun: string): string | null {
  if (resource.status === 'error') return resource.message;
  if (resource.status === 'ready') return null;
  return `Loading ${noun}…`;
}

/** ⌘K: the server's databases; system databases only when requested. */
export function DatabaseSwitcher(props: {
  catalog: Catalog;
  onChoose: (name: string) => void;
  onClose: () => void;
}): React.JSX.Element {
  const [system, setSystem] = useState(false);
  const { databases, opened } = props.catalog;
  const items: PaletteItem[] =
    databases.status === 'ready'
      ? databases.data
          .filter((database) => system || !database.system)
          .map((database) => ({
            id: database.name,
            label: database.name,
            glyph: '▱',
            detail: opened.includes(database.name)
              ? 'Already open'
              : (database.comment ?? 'Available'),
          }))
      : [];
  const status = props.catalog.offline ?? statusText(databases, 'databases');
  return (
    <Palette
      title="Switch database"
      onClose={props.onClose}
      onChoose={props.onChoose}
      items={items}
      footer={
        <>
          {status && <span role="status">{status}</span>}
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
        </>
      }
    />
  );
}

const entryId = (object: ObjectDto): string =>
  `${object.kind}:${object.schema}.${objectLabel(object)}`;

function entryItem(object: ObjectDto): PaletteItem {
  const kind = kindInfo(object.kind);
  const estimate = formatEstimate(object.rowEstimate);
  return {
    id: entryId(object),
    label: objectLabel(object),
    glyph: kind.glyph,
    searchText: `${object.name} ${object.schema}`,
    detail: `${kind.label} · ${object.schema}${estimate ? ` · ${estimate} rows (estimate)` : ''}`,
  };
}

/** ⌘P: fuzzy search over the active database's bulk name index. */
export function QuickOpen(props: {
  catalog: Catalog;
  onChoose: (object: ObjectRef) => void;
  onClose: () => void;
}): React.JSX.Element {
  const { search, loadSearch, database } = props.catalog;
  useEffect(() => {
    loadSearch();
  }, [loadSearch]);
  const objects = search.status === 'ready' ? search.data : [];
  const status = props.catalog.offline ?? statusText(search, 'objects');
  return (
    <Palette
      title="Quick open"
      onClose={props.onClose}
      onChoose={(id) => {
        const object = objects.find((item) => entryId(item) === id);
        if (object) props.onChoose(objectRef(database, object));
      }}
      items={objects.map(entryItem)}
      {...(status ? { footer: <span role="status">{status}</span> } : {})}
    />
  );
}
