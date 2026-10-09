// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button, DisclosureSection, Tree } from '@openrows/ui';
import type { ObjectDto } from '@openrows/ipc';
import { nodeKey, objectLabel } from './catalog';
import type { ObjectGroup, ObjectRef } from './catalog';
import { ObjectRow } from './ObjectRow';
import type { MenuPoint } from './ObjectRow';
import { OBJECT_PAGE } from './useCatalog';
import type { Catalog, ObjectPage } from './useCatalog';
import type { Resource } from './useResources';
import styles from './ObjectTree.module.css';

type Props = {
  catalog: Catalog;
  group: ObjectGroup;
  search: string;
  /** Index matches of the search in the selected schema, when it has any. */
  found: Resource<ObjectDto[]> | null;
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

/** The index matches of this section's kinds. */
function inGroup(
  found: Resource<ObjectDto[]>,
  group: ObjectGroup,
): Resource<ObjectDto[]> {
  if (found.status !== 'ready') return found;
  const data = found.data.filter((object) => group.kinds.includes(object.kind));
  return { ...found, data };
}

/** The objects of a loaded page as a list resource. */
function pageObjects(page: Resource<ObjectPage>): Resource<ObjectDto[]> {
  return page.status === 'ready' ? { ...page, data: page.data.objects } : page;
}

/**
 * The section's pages: the first `limit` objects, shown from the previous
 * (smaller) page while "Show more" loads the next one.
 */
function usePages(catalog: Catalog, group: ObjectGroup, load: boolean) {
  const [limit, setLimit] = useState(OBJECT_PAGE);
  const current = catalog.objects(group, limit);
  const previous = catalog.objects(group, limit - OBJECT_PAGE);
  const { loadObjects } = catalog;
  useEffect(() => {
    if (load && current.status === 'idle') loadObjects(group, limit);
  }, [load, current.status, loadObjects, group, limit]);
  const shown =
    current.status !== 'ready' && previous.status === 'ready'
      ? previous
      : current;
  const more =
    shown.status === 'ready' && shown.data.total > shown.data.objects.length
      ? shown.data.total - shown.data.objects.length
      : 0;
  return {
    listed: pageObjects(shown),
    total: shown.status === 'ready' ? shown.data.total : undefined,
    more,
    showMore: () => {
      setLimit((count) => count + OBJECT_PAGE);
    },
  };
}

/**
 * One kind group; loads its objects the first time it is expanded, 5 000
 * at a time. While the index answers the search, it lists the index
 * matches instead.
 */
export function ObjectSection(props: Props): React.JSX.Element {
  const { catalog, group, search, found } = props;
  const [open, setOpen] = useState(group.id === 'tables');
  const expanded = open || !!search;
  const indexed = found !== null;
  const pages = usePages(catalog, group, expanded && !indexed);
  const { listed } = pages;
  const resource = found ? inGroup(found, group) : listed;
  const items =
    resource.status !== 'ready'
      ? []
      : indexed
        ? resource.data
        : matching(resource.data, search);
  const stop = tabStop(items, props.selected, catalog);
  return (
    <div hidden={!!search && resource.status === 'ready' && !items.length}>
      <DisclosureSection
        label={group.label}
        {...sectionCount(resource, pages.total, indexed || !!search, items)}
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
        {!indexed && <ShowMore more={pages.more} onClick={pages.showMore} />}
      </DisclosureSection>
    </div>
  );
}

/** The section's count: all of its objects, or the matches while searching. */
function sectionCount(
  resource: Resource<ObjectDto[]>,
  total: number | undefined,
  filtered: boolean,
  items: readonly ObjectDto[],
): { count?: number } {
  if (resource.status !== 'ready') return {};
  return { count: filtered ? items.length : (total ?? items.length) };
}

/** "Show 5,000 more of 12,345" below a section that holds more objects. */
function ShowMore({
  more,
  onClick,
}: {
  more: number;
  onClick: () => void;
}): React.JSX.Element | null {
  if (more <= 0) return null;
  const next = Math.min(more, OBJECT_PAGE).toLocaleString('en-US');
  return (
    <Button size="sm" variant="ghost" onClick={onClick}>
      Show {next} more of {more.toLocaleString('en-US')}
    </Button>
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
