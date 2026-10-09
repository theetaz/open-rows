// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button, DisclosureSection, Tree } from '@openrows/ui';
import type { ObjectDto } from '@openrows/ipc';
import { nodeKey, objectLabel } from './catalog';
import type { ObjectGroup, ObjectRef } from './catalog';
import { ObjectRow } from './ObjectRow';
import type { MenuPoint } from './ObjectRow';
import type { Catalog } from './useCatalog';
import styles from './ObjectTree.module.css';

type Props = {
  catalog: Catalog;
  group: ObjectGroup;
  search: string;
  selected: string | null;
  onSelect: (key: string) => void;
  onOpen: (object: ObjectRef, preview?: boolean) => void;
  onMenu?: ((object: ObjectRef, point: MenuPoint) => void) | undefined;
};

/** Objects matching the sidebar search (case-insensitive substring). */
export function matching(
  objects: readonly ObjectDto[],
  search: string,
): ObjectDto[] {
  const needle = search.toLowerCase();
  return objects.filter((object) =>
    objectLabel(object).toLowerCase().includes(needle),
  );
}

/** One kind group; loads its objects the first time it is expanded. */
export function ObjectSection(props: Props): React.JSX.Element {
  const { catalog, group, search } = props;
  const [open, setOpen] = useState(group.id === 'tables');
  const resource = catalog.objects(group);
  const expanded = open || !!search;
  const { loadObjects } = catalog;
  useEffect(() => {
    if (expanded && resource.status === 'idle') loadObjects(group);
  }, [expanded, resource.status, loadObjects, group]);
  const items =
    resource.status === 'ready' ? matching(resource.data, search) : [];
  const stop = tabStop(items, props.selected, catalog);
  return (
    <div hidden={!!search && resource.status === 'ready' && !items.length}>
      <DisclosureSection
        label={group.label}
        {...(resource.status === 'ready' ? { count: items.length } : {})}
        defaultOpen={open}
        forceOpen={!!search}
        onOpenChange={setOpen}
      >
        {resource.status === 'loading' && (
          <p className={styles.note} role="status">
            Loading…
          </p>
        )}
        {resource.status === 'error' && (
          <div className={styles.error} role="alert">
            <span>{resource.message}</span>
            <Button size="sm" variant="ghost" onClick={catalog.refresh}>
              Retry
            </Button>
          </div>
        )}
        {resource.status === 'ready' && !resource.data.length && (
          <p className={styles.note}>No {group.label.toLowerCase()}</p>
        )}
        {items.length > 0 && (
          <Tree label={group.label}>
            {items.map((object) => (
              <ObjectRow
                {...props}
                key={nodeKey(object)}
                object={object}
                tabStop={stop}
              />
            ))}
          </Tree>
        )}
      </DisclosureSection>
    </div>
  );
}

/**
 * The section tree's Tab stop: the selected row (or its collapsed parent),
 * else the first object.
 */
function tabStop(
  items: readonly ObjectDto[],
  selected: string | null,
  catalog: Catalog,
): string | undefined {
  for (const object of items) {
    const id = nodeKey(object);
    if (selected === id) return id;
    if (selected?.startsWith(`${id}\n`))
      return catalog.expanded(object) ? selected : id;
  }
  return items[0] && nodeKey(items[0]);
}
