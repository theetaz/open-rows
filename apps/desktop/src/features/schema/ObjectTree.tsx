// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { objectGroups } from './catalog';
import type { ObjectRef } from './catalog';
import { matching, ObjectSection } from './ObjectSection';
import type { Catalog } from './useCatalog';
import styles from './ObjectTree.module.css';

/**
 * Objects of the selected schema grouped by kind. Sections load lazily;
 * search filters loaded names and forces matching sections open. Relations
 * expand to their columns.
 */
export function ObjectTree(props: {
  catalog: Catalog;
  search: string;
  onOpen: (object: ObjectRef) => void;
}): React.JSX.Element {
  const { catalog, search } = props;
  const [selected, setSelected] = useState<string | null>(null);
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
    <div className={styles.tree}>
      {search && noMatches(catalog, search) && <p role="status">No matches</p>}
      {objectGroups.map((group) => (
        <ObjectSection
          key={group.id}
          catalog={catalog}
          group={group}
          search={search}
          selected={selected}
          onSelect={setSelected}
          onOpen={props.onOpen}
        />
      ))}
    </div>
  );
}

/** Every section has loaded and none has a match. */
function noMatches(catalog: Catalog, search: string): boolean {
  return objectGroups.every((group) => {
    const resource = catalog.objects(group);
    return (
      resource.status === 'ready' && !matching(resource.data, search).length
    );
  });
}
