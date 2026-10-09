// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Palette } from '@openrows/ui/palette';
import type { PaletteItem } from '@openrows/ui';
import type { ObjectDto } from '@openrows/ipc';
import { formatEstimate, kindInfo, objectLabel, objectRef } from './catalog';
import type { ObjectRef } from './catalog';
import type { Catalog } from './useCatalog';
import { useIndexSearch } from './useCatalogIndex';
import type { Resource } from './useResources';

/** Most quick-open matches listed. */
const QUICK_OPEN_LIMIT = 100;

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
  onClose: () => void;
}): React.JSX.Element {
  const { catalog } = props;
  const [query, setQuery] = useState('');
  const found = useIndexSearch(catalog, {
    text: query,
    schema: null,
    limit: QUICK_OPEN_LIMIT,
  });
  const objects = found?.status === 'ready' ? found.data : [];
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
        const object = objects.find((item) => entryId(item) === id);
        if (object) props.onChoose(objectRef(catalog.database, object));
      }}
      items={objects.map(entryItem)}
      {...(status ? { footer: <span role="status">{status}</span> } : {})}
    />
  );
}
