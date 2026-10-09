// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { ObjectDto } from '@openrows/ipc';
import { objectGroups } from './catalog';
import type { ObjectRef } from './catalog';
import type { MenuPoint } from './ObjectRow';
import { matching, ObjectSection } from './ObjectSection';
import { moveAcrossSections } from './sectionKeys';
import type { Catalog } from './useCatalog';
import { useIndexSearch } from './useCatalogIndex';
import type { IndexQuery } from './useCatalogIndex';
import type { Resource } from './useResources';
import styles from './ObjectTree.module.css';

/** Most sidebar search matches listed. */
const SEARCH_LIMIT = 1_000;

/**
 * Objects of the selected schema grouped by kind. Sections load lazily;
 * search asks the database's name index (system schemas, which it skips,
 * filter their loaded sections instead) and forces matching sections open.
 * Relations expand to their columns.
 */
export function ObjectTree(props: {
  catalog: Catalog;
  search: string;
  onOpen: (object: ObjectRef, preview?: boolean) => void;
  /** Context menu of a relation row (tables, views, materialized views). */
  onMenu?: ((object: ObjectRef, point: MenuPoint) => void) | undefined;
}): React.JSX.Element {
  const { catalog, search } = props;
  const [selected, setSelected] = useState<string | null>(null);
  const found = useIndexSearch(catalog, indexQuery(catalog, search), true);
  const { offline, schemas } = catalog;
  if (offline) return <p className={styles.note}>{offline}</p>;
  if (!catalog.database)
    return (
      <p className={styles.noDatabase}>
        No database selected, press ⌘K to select a database
      </p>
    );
  if (schemas.status === 'error')
    return (
      <div className={styles.error} role="alert">
        <span>{schemas.message}</span>
        <Button size="sm" variant="ghost" onClick={catalog.refresh}>
          Retry
        </Button>
      </div>
    );
  if (schemas.status !== 'ready')
    return (
      <p className={styles.note} role="status">
        Loading schemas…
      </p>
    );
  if (!catalog.schema) return <p className={styles.note}>No schemas</p>;
  return (
    <div className={styles.tree} onKeyDown={moveAcrossSections}>
      {catalog.refreshing && (
        <p className={styles.refreshing} role="status">
          Refreshing…
        </p>
      )}
      {search && noMatches(catalog, search, found) && (
        <p role="status">No matches</p>
      )}
      {objectGroups.map((group) => (
        <ObjectSection
          key={group.id}
          catalog={catalog}
          group={group}
          search={search}
          found={found}
          selected={selected}
          onSelect={setSelected}
          onOpen={props.onOpen}
          onMenu={props.onMenu}
        />
      ))}
    </div>
  );
}

/** The index search of `search` in the selected user schema, if any. */
function indexQuery(catalog: Catalog, search: string): IndexQuery | null {
  const { schemas, schema } = catalog;
  if (!search || !schema || schemas.status !== 'ready') return null;
  const system = schemas.data.find((item) => item.name === schema)?.system;
  return system === false
    ? { text: search, schema, limit: SEARCH_LIMIT }
    : null;
}

/** Every section has loaded (or the index answered) and nothing matches. */
function noMatches(
  catalog: Catalog,
  search: string,
  found: Resource<ObjectDto[]> | null,
): boolean {
  if (found) return found.status === 'ready' && !found.data.length;
  return objectGroups.every((group) => {
    const resource = catalog.objects(group);
    return (
      resource.status === 'ready' && !matching(resource.data, search).length
    );
  });
}
