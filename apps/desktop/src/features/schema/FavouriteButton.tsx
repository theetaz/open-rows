// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton } from '@openrows/ui';
import { toggleFavourite, useSidebarObjects } from './sidebarStore';
import { sidebarObjectKey } from './sidebarIdentity';
import type { SidebarObject } from './sidebarIdentity';

/**
 * Shared favourite control: the address bar's star (`className` styles it)
 * and the tree context menu's item.
 */
export function FavouriteButton(props: {
  object: SidebarObject;
  menu?: boolean;
  className?: string | undefined;
}): React.JSX.Element {
  // Derive from the subscribed snapshot: the React Compiler memoizes a
  // render-time store read on its unchanged argument, freezing the star.
  const saved = useSidebarObjects(
    props.object.profileId,
    props.object.database,
  );
  const key = sidebarObjectKey(props.object);
  const starred = saved.favourites.some(
    (item) => sidebarObjectKey(item) === key,
  );
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
    <IconButton
      size="sm"
      variant="ghost"
      className={props.className}
      aria-label={label}
      title={label}
      aria-pressed={starred}
      onClick={() => {
        toggleFavourite(props.object);
      }}
    >
      <Icon name="star" />
    </IconButton>
  );
}
