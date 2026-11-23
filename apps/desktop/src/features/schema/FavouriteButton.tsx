// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton } from '@openrows/ui';
import { toggleFavourite, useSidebarObjects } from './sidebarStore';
import { sidebarObjectKey } from './sidebarIdentity';
import type { SidebarObject } from './sidebarIdentity';
import { t } from './messages';

/** Whether `object` is a favourite, its toggle's label and the toggle. */
export function useFavourite(object: SidebarObject | null): {
  starred: boolean;
  label: string;
  toggle: () => void;
} {
  // Derive from the subscribed snapshot: the React Compiler memoizes a
  // render-time store read on its unchanged argument, freezing the star.
  const saved = useSidebarObjects(
    object?.profileId ?? '',
    object?.database ?? '',
  );
  const key = object ? sidebarObjectKey(object) : '';
  const starred = saved.favourites.some(
    (item) => sidebarObjectKey(item) === key,
  );
  return {
    starred,
    label: starred ? t('removeFavourite') : t('addFavourite'),
    toggle: () => {
      if (object) toggleFavourite(object);
    },
  };
}

/** The address bar's favourite star (`className` styles it). */
export function FavouriteButton(props: {
  object: SidebarObject;
  className?: string | undefined;
}): React.JSX.Element {
  const { starred, label, toggle } = useFavourite(props.object);
  return (
    <IconButton
      size="sm"
      variant="ghost"
      className={props.className}
      aria-label={label}
      title={label}
      aria-pressed={starred}
      onClick={toggle}
    >
      <Icon name="star" />
    </IconButton>
  );
}
