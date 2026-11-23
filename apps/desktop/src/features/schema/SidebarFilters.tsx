// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton, Menu } from '@openrows/ui';
import type { MenuEntry } from '@openrows/ui';
import { useMenuButton } from '../../shell/useMenuButton';
import { supportedGroups } from './catalog';
import type { ObjectGroup } from './catalog';
import {
  useSidebarPreferences,
  writeSidebarPreferences,
} from './sidebarPreferences';
import type { SidebarPreferences } from './sidebarPreferences';
import type { Catalog } from './useCatalog';
import styles from './SidebarLists.module.css';
import { t } from './messages';

type FilterProps = {
  catalog: Catalog;
  preferences: SidebarPreferences;
  groups: readonly ObjectGroup[];
};
const toggled = (items: readonly string[], item: string): string[] =>
  items.includes(item)
    ? items.filter((value) => value !== item)
    : [...items, item];

/**
 * One filter button beside the sidebar search: kinds, schemas, system objects
 * and section visibility live in its menu instead of rows of chips.
 */
export function SidebarFilters({
  catalog,
}: {
  catalog: Catalog;
}): React.JSX.Element | null {
  const preferences = useSidebarPreferences(catalog.profileId);
  const { anchor, close, button } = useMenuButton();
  const features = catalog.inspection.capabilities;
  if (features.status !== 'ready' || !catalog.profileId) return null;
  const groups = supportedGroups(features.data.kinds);
  const active = preferences.kinds.length + preferences.schemas.length > 0;
  return (
    <>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('filterObjects')}
        data-active={active}
        className={styles.filterButton}
        {...button}
      >
        <Icon name="filter" />
      </IconButton>
      {anchor && (
        <Menu
          label={t('filterObjects')}
          anchor={anchor}
          align="end"
          entries={filterEntries({ catalog, preferences, groups })}
          onClose={close}
        />
      )}
    </>
  );
}

function write(catalog: Catalog, change: Partial<SidebarPreferences>): void {
  writeSidebarPreferences(catalog.profileId, change);
}

function filterEntries(props: FilterProps): MenuEntry[] {
  const { catalog, preferences } = props;
  const active = preferences.kinds.length + preferences.schemas.length > 0;
  return [
    {
      kind: 'submenu',
      label: t('objectKinds'),
      icon: <Icon name={'table'} />,
      entries: kindEntries(props),
    },
    {
      kind: 'submenu',
      label: t('schemas'),
      icon: <Icon name={'schemaTree'} />,
      entries: schemaEntries(props),
    },
    {
      kind: 'submenu',
      label: t('sections'),
      icon: <Icon name={'folder'} />,
      entries: sectionEntries(props),
    },
    { kind: 'separator' },
    {
      kind: 'item',
      label: t('showSystemObjects'),
      checked: preferences.systemObjects,
      run: () => {
        catalog.setSystemSchemas(!preferences.systemObjects);
      },
    },
    ...(active
      ? [
          { kind: 'separator' } as const,
          {
            kind: 'item' as const,
            label: t('clearFilters'),
            icon: <Icon name={'close'} />,
            run: () => {
              write(catalog, { kinds: [], schemas: [] });
            },
          },
        ]
      : []),
  ];
}

function kindEntries({
  catalog,
  preferences,
  groups,
}: FilterProps): MenuEntry[] {
  return [
    {
      kind: 'item',
      label: t('allKinds'),
      checked: preferences.kinds.length === 0,
      run: () => {
        write(catalog, { kinds: [] });
      },
    },
    { kind: 'separator' },
    ...groups.map((group): MenuEntry => ({
      kind: 'item',
      label: group.label,
      checked: preferences.kinds.includes(group.id),
      run: () => {
        write(catalog, { kinds: toggled(preferences.kinds, group.id) });
      },
    })),
  ];
}

function schemaEntries({ catalog, preferences }: FilterProps): MenuEntry[] {
  const schemas =
    catalog.schemas.status === 'ready'
      ? catalog.schemas.data.filter(
          (schema) => catalog.systemSchemas || !schema.system,
        )
      : [];
  return [
    {
      kind: 'item',
      label: t('currentSchema'),
      checked: preferences.schemas.length === 0,
      run: () => {
        write(catalog, { schemas: [] });
      },
    },
    { kind: 'separator' },
    ...schemas.map(({ name }): MenuEntry => ({
      kind: 'item',
      label: name,
      checked: preferences.schemas.includes(name),
      run: () => {
        write(catalog, { schemas: toggled(preferences.schemas, name) });
      },
    })),
  ];
}

function sectionEntries({
  catalog,
  preferences,
  groups,
}: FilterProps): MenuEntry[] {
  const sections = [{ id: 'favourites', label: t('favourites') }, ...groups];
  return sections.map((section): MenuEntry => ({
    kind: 'item',
    label: section.label,
    checked: !preferences.hidden.includes(section.id),
    run: () => {
      write(catalog, { hidden: toggled(preferences.hidden, section.id) });
    },
  }));
}
