// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import {
  Button,
  DisclosureSection,
  HighlightText,
  TreeItem,
} from '@openrows/ui';
import type { ObjectDto } from '@openrows/ipc';
import {
  formatEstimate,
  kindInfo,
  objectLabel,
  objectRef,
  sameObject,
  substringIndices,
} from './catalog';
import type { ObjectGroup, ObjectRef } from './catalog';
import type { Catalog } from './useCatalog';
import styles from './ObjectTree.module.css';

type Props = {
  catalog: Catalog;
  group: ObjectGroup;
  search: string;
  selected: ObjectRef | null;
  onSelect: (object: ObjectRef) => void;
  onOpen: (object: ObjectRef) => void;
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
        {items.map((object) => (
          <ObjectRow {...props} key={key(object)} object={object} />
        ))}
      </DisclosureSection>
    </div>
  );
}

const key = (object: ObjectDto): string =>
  `${object.kind}:${objectLabel(object)}`;

function ObjectRow(props: Props & { object: ObjectDto }): React.JSX.Element {
  const { catalog, object, search } = props;
  const label = objectLabel(object);
  const ref = objectRef(catalog.database, object);
  const count = formatEstimate(object.rowEstimate);
  const kind = kindInfo(object.kind);
  return (
    <TreeItem
      label={label}
      {...(count ? { count } : {})}
      glyph={kind.glyph}
      title={`${catalog.database}.${object.schema}.${label} · ${kind.label}${object.comment ? ` · ${object.comment}` : ''}`}
      selected={!!props.selected && sameObject(props.selected, ref)}
      onSelect={() => {
        props.onSelect(ref);
      }}
      onOpen={() => {
        props.onOpen(ref);
      }}
    >
      <HighlightText text={label} indices={substringIndices(label, search)} />
    </TreeItem>
  );
}
