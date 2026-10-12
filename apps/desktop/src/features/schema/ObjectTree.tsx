// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { ObjectDto } from '@openrows/ipc';
import { objectGroups, supportedGroups } from './catalog';
import { SidebarLists } from './SidebarLists';
import { useSidebarPreferences } from './sidebarPreferences';
import { AdminTree } from './AdminTree';
import type { InspectionTarget } from '../inspection/target';
import type { ObjectGroup, ObjectRef } from './catalog';
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
  onSchemaMenu?: ((schema: string, point: MenuPoint) => void) | undefined;
  search: string;
  onInspect?: ((target: InspectionTarget) => void) | undefined;
  onOpen: (object: ObjectRef, preview?: boolean) => void;
  /** Context menu of a relation row (tables, views, materialized views). */
  onMenu?: ((object: ObjectRef, point: MenuPoint) => void) | undefined;
}): React.JSX.Element {
  const { catalog, search } = props;
  const [selected, setSelected] = useState<string | null>(null);
  const found = useIndexSearch(
    catalog,
    indexQuery(
      catalog,
      search,
      useSidebarPreferences(catalog.profileId).schemas,
    ),
    true,
  );
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
  const features = catalog.inspection.capabilities;
  if (features.status === 'error')
    return <p role="alert">{features.message}</p>;
  if (features.status !== 'ready')
    return <p role="status">Loading catalog capabilities…</p>;
  return (
    <div className={styles.tree} onKeyDown={moveAcrossSections}>
      {!catalog.schema && <p className={styles.note}>No schemas</p>}
      {catalog.refreshing && (
        <p className={styles.refreshing} role="status">
          Refreshing…
        </p>
      )}
      {search && noMatches(catalog, search, found) && (
        <p role="status">No matches</p>
      )}
      <SidebarLists
        catalog={catalog}
        onOpen={props.onOpen}
        onMenu={props.onMenu}
      />
      <CatalogSections
        {...props}
        selected={selected}
        onSelect={setSelected}
        found={found}
      />
      {props.onInspect && (
        <AdminTree
          catalog={catalog.inspection}
          search={search}
          onOpen={props.onInspect}
        />
      )}
    </div>
  );
}

/** The index search of `search` in the selected user schema, if any. */
function indexQuery(
  catalog: Catalog,
  search: string,
  selectedSchemas: readonly string[],
): IndexQuery | null {
  const { schemas, schema } = catalog;
  if (!search || !schema || schemas.status !== 'ready') return null;
  const system = schemas.data.find((item) => item.name === schema)?.system;
  return system === false
    ? {
        text: search,
        schema:
          selectedSchemas.length > 1 ? null : (selectedSchemas[0] ?? schema),
        limit: SEARCH_LIMIT,
      }
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
      resource.status === 'ready' &&
      !matching(resource.data.objects, search).length
    );
  });
}

/** Visible schemas and sections, without changing the independent quick-open index. */
function CatalogSections(
  props: Parameters<typeof ObjectTree>[0] & {
    selected: string | null;
    onSelect: (key: string) => void;
    found: Resource<ObjectDto[]> | null;
  },
): React.JSX.Element {
  const { catalog } = props;
  const preferences = useSidebarPreferences(catalog.profileId);
  const features = catalog.inspection.capabilities;
  const groups =
    features.status === 'ready' ? supportedGroups(features.data.kinds) : [];
  const schemas = preferences.schemas.length
    ? preferences.schemas
    : [catalog.schema];
  const names = catalog.schemas.status === 'ready' ? catalog.schemas.data : [];
  return (
    <>
      {schemas
        .filter((name) =>
          names.some(
            (schema) =>
              schema.name === name &&
              (preferences.systemObjects || !schema.system),
          ),
        )
        .map((schema) => {
          const scoped = {
            ...catalog,
            schema,
            objects: (group: ObjectGroup, limit?: number) =>
              catalog.objects(group, limit, schema),
            loadObjects: (group: ObjectGroup, limit?: number) => {
              catalog.loadObjects(group, limit, schema);
            },
          };
          return (
            <div key={schema}>
              {schemas.length > 1 && (
                <p
                  className={styles.note}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    props.onSchemaMenu?.(schema, {
                      x: event.clientX,
                      y: event.clientY,
                    });
                  }}
                >
                  {schema}
                </p>
              )}
              {groups
                .filter(
                  (group) =>
                    !preferences.hidden.includes(group.id) &&
                    (!preferences.kinds.length ||
                      preferences.kinds.includes(group.id)),
                )
                .map((group) => (
                  <ObjectSection
                    {...props}
                    key={group.id}
                    catalog={scoped}
                    found={
                      names.find((item) => item.name === schema)?.system
                        ? null
                        : props.found
                    }
                    group={group}
                  />
                ))}
            </div>
          );
        })}
    </>
  );
}
