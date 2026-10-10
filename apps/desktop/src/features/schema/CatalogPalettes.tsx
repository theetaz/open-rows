// SPDX-License-Identifier: Apache-2.0
import { useMemo, useState } from 'react';
import { Palette } from '@openrows/ui/palette';
import type { PaletteItem } from '@openrows/ui';
import type { ObjectDto } from '@openrows/ipc';
import { formatEstimate, kindInfo, objectLabel, objectRef } from './catalog';
import type { ObjectRef } from './catalog';
import type { Catalog } from './useCatalog';
import { quickIndex, searchQuick } from './quickSearch';
import { useQuerySearch } from './useQuerySearch';
import { useIndexSearch } from './useCatalogIndex';
import type { Resource } from './useResources';

/** Most quick-open matches listed. */
const QUICK_OPEN_LIMIT = 100;
const EMPTY_OBJECTS: readonly ObjectDto[] = [];

/** Loading or failure text for a palette whose items are still unknown. */
function statusText(resource: Resource<unknown>, noun: string): string | null {
  if (resource.status === 'error') return resource.message;
  if (resource.status === 'ready') return null;
  return `Loading ${noun}…`;
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

/**
 * ⌘P: the active database's prebuilt name index, searched in Rust per
 * keystroke (prefix and substring over qualified names, best first).
 */
export function QuickOpen(props: {
  catalog: Catalog;
  onChoose: (object: ObjectRef) => void;
  onSavedQuery?: ((sql: string, title: string) => void) | undefined;
  onProjectQuery?: ((path: string) => void) | undefined;
  onClose: () => void;
}): React.JSX.Element {
  const { catalog } = props;
  const [query, setQuery] = useState('');
  const queries = useQuerySearch(catalog.profileId);
  const found = useIndexSearch(catalog, {
    text: query,
    schema: null,
    limit: QUICK_OPEN_LIMIT,
  });
  const objects = found?.status === 'ready' ? found.data : EMPTY_OBJECTS;
  const index = useMemo(
    () => [...quickIndex(objects.map(entryItem)), ...queries.index],
    [objects, queries.index],
  );
  const items = searchQuick(index, query, QUICK_OPEN_LIMIT);
  const failed = found?.status === 'error' ? found : null;
  const status =
    catalog.offline ??
    (catalog.database
      ? statusText(failed ?? catalog.index, 'objects')
      : 'No database selected');
  return (
    <Palette
      title="Quick open"
      onClose={props.onClose}
      onQueryChange={setQuery}
      onChoose={(id) => {
        if (id.startsWith('project:')) {
          props.onProjectQuery?.(id.slice(8));
          return;
        }
        const saved = queries.saved.queries.find(
          (item) => `saved:${item.id}` === id,
        );
        if (saved) {
          props.onSavedQuery?.(saved.sql, saved.name);
          return;
        }
        const object = objects.find((item) => entryId(item) === id);
        if (object) props.onChoose(objectRef(catalog.database, object));
      }}
      items={items}
      {...(status ? { footer: <span role="status">{status}</span> } : {})}
    />
  );
}
