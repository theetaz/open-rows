// SPDX-License-Identifier: Apache-2.0
import { Button, Icon } from '@openrows/ui';
import {
  isFavourite,
  toggleFavourite,
  useSidebarObjects,
} from './sidebarStore';
import type { SidebarObject } from './sidebarIdentity';

/** Shared star control for object/table headers and the tree context menu. */
export function FavouriteButton(props: {
  object: SidebarObject;
  menu?: boolean;
}): React.JSX.Element {
  useSidebarObjects(props.object.profileId, props.object.database);
  const starred = isFavourite(props.object);
  const label = starred ? 'Remove favourite' : 'Add favourite';
  if (props.menu)
    return (
      <button
        type="button"
        role="menuitem"
        onClick={() => {
          toggleFavourite(props.object);
        }}
      >
        {label}
      </button>
    );
  return (
    <Button
      size="sm"
      variant="ghost"
      aria-label={label}
      aria-pressed={starred}
      onClick={() => {
        toggleFavourite(props.object);
      }}
    >
      <Icon name="star" />
      {label}
    </Button>
  );
}
