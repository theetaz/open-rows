// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, DisclosureSection, Icon, Tree, TreeNode } from '@openrows/ui';
import { objectLabel } from './catalog';
import type { ObjectRef } from './catalog';
import { sidebarObjectKey } from './sidebarIdentity';
import type { SidebarObject } from './sidebarIdentity';
import {
  clearRecent,
  moveFavourite,
  toggleFavourite,
  useSidebarObjects,
} from './sidebarStore';
import { useSidebarPreferences } from './sidebarPreferences';
import type { Catalog } from './useCatalog';
import type { MenuPoint } from './ObjectRow';
import styles from './SidebarLists.module.css';

type Props = {
  catalog: Catalog;
  onOpen: (object: ObjectRef, preview?: boolean) => void;
  onMenu?: ((object: ObjectRef, point: MenuPoint) => void) | undefined;
};

/** Saved identities appear above sections and never cause catalog reads. */
export function SidebarLists(props: Props): React.JSX.Element {
  const saved = useSidebarObjects(
    props.catalog.profileId,
    props.catalog.database,
  );
  const preferences = useSidebarPreferences(props.catalog.profileId);
  return (
    <>
      {!preferences.hidden.includes('favourites') && (
        <SavedSection
          {...props}
          label="Favourites"
          objects={saved.favourites}
        />
      )}
      {!preferences.hidden.includes('recent') && (
        <SavedSection
          {...props}
          label="Recent"
          objects={saved.recent.slice(0, 15)}
        />
      )}
    </>
  );
}

function SavedSection(
  props: Props & { label: string; objects: readonly SidebarObject[] },
): React.JSX.Element {
  const [selected, setSelected] = useState<string | null>(null);
  const active = props.objects.some(
    (object) => sidebarObjectKey(object) === selected,
  )
    ? selected
    : null;
  const favourites = props.label === 'Favourites';
  return (
    <DisclosureSection
      label={props.label}
      count={props.objects.length}
      defaultOpen
    >
      {!favourites && props.objects.length > 0 && (
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            clearRecent(props.catalog.profileId, props.catalog.database);
          }}
        >
          Clear recent objects
        </Button>
      )}
      <div
        onKeyDownCapture={(event) => {
          if (
            !favourites ||
            !event.altKey ||
            !['ArrowUp', 'ArrowDown'].includes(event.key)
          )
            return;
          const index = props.objects.findIndex(
            (item) => sidebarObjectKey(item) === selected,
          );
          const object = props.objects[index];
          if (!object) return;
          event.preventDefault();
          event.stopPropagation();
          moveFavourite(object, index + (event.key === 'ArrowUp' ? -1 : 1));
        }}
      >
        <Tree label={props.label}>
          {props.objects.map((object, index) => (
            <SavedRow
              {...props}
              key={sidebarObjectKey(object)}
              object={object}
              index={index}
              selected={active}
              onSelect={setSelected}
            />
          ))}
        </Tree>
      </div>
    </DisclosureSection>
  );
}

function SavedRow(
  props: Props & {
    label: string;
    objects: readonly SidebarObject[];
    object: SidebarObject;
    index: number;
    selected: string | null;
    onSelect: (key: string) => void;
  },
): React.JSX.Element {
  const { object, catalog } = props;
  const key = sidebarObjectKey(object);
  const missing = catalog.available(object) === false;
  const favourite = props.label === 'Favourites';
  const open = (): void => {
    if (!missing) props.onOpen(object);
  };
  const label = `${object.schema}.${objectLabel(object)}`;
  return (
    <div className={missing ? styles.missing : undefined}>
      <TreeNode
        label={`${label}${missing ? ' (missing)' : ''}`}
        level={1}
        selected={props.selected === key}
        tabStop={props.selected ? props.selected === key : props.index === 0}
        onSelect={() => {
          props.onSelect(key);
        }}
        onOpen={open}
        onPreview={open}
        icon={favourite ? <Icon name="star" /> : undefined}
        title={missing ? 'Missing object' : label}
        name={`${label}${missing ? ' (missing)' : ''}`}
        onContextMenu={(event) => {
          event.preventDefault();
          props.onMenu?.(object, { x: event.clientX, y: event.clientY });
        }}
        drag={favourite ? favouriteDrag(props) : undefined}
        detail={
          missing && favourite ? (
            <button
              type="button"
              aria-label={`Remove ${label}`}
              onClick={(event) => {
                event.stopPropagation();
                toggleFavourite(object);
              }}
            >
              Remove
            </button>
          ) : missing ? (
            'missing'
          ) : undefined
        }
      />
    </div>
  );
}

function favouriteDrag(props: {
  object: SidebarObject;
  index: number;
  objects: readonly SidebarObject[];
}) {
  return {
    draggable: true,
    onDragStart: (event: React.DragEvent) => {
      event.dataTransfer.setData(
        'application/x-openrows-favourite',
        sidebarObjectKey(props.object),
      );
    },
    onDragOver: (event: React.DragEvent) => {
      event.preventDefault();
    },
    onDrop: (event: React.DragEvent) => {
      event.preventDefault();
      const key = event.dataTransfer.getData(
        'application/x-openrows-favourite',
      );
      const object = props.objects.find(
        (item) => sidebarObjectKey(item) === key,
      );
      if (object) moveFavourite(object, props.index);
    },
  };
}
