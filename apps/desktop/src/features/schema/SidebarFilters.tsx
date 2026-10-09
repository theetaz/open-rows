// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { PointMenu } from '../../shell/PointMenu';
import { supportedGroups } from './catalog';
import type { ObjectGroup } from './catalog';
import {
  useSidebarPreferences,
  writeSidebarPreferences,
} from './sidebarPreferences';
import type { SidebarPreferences } from './sidebarPreferences';
import type { Catalog } from './useCatalog';
import styles from './SidebarLists.module.css';

type FilterProps = { catalog: Catalog; preferences: SidebarPreferences };
const toggled = (items: readonly string[], item: string): string[] =>
  items.includes(item)
    ? items.filter((value) => value !== item)
    : [...items, item];

/** Capability-driven kind chips, multi-schema filter and section header menu. */
export function SidebarFilters({
  catalog,
}: {
  catalog: Catalog;
}): React.JSX.Element | null {
  const preferences = useSidebarPreferences(catalog.profileId);
  const features = catalog.inspection.capabilities;
  if (features.status !== 'ready' || !catalog.profileId) return null;
  const groups = supportedGroups(features.data.kinds);
  return (
    <div className={styles.filters}>
      <KindChips catalog={catalog} preferences={preferences} groups={groups} />
      <SchemaFilter catalog={catalog} preferences={preferences} />
      <label>
        <input
          type="checkbox"
          checked={preferences.systemObjects}
          onChange={(event) => {
            catalog.setSystemSchemas(event.target.checked);
          }}
        />
        Show system objects
      </label>
      <SectionMenu
        catalog={catalog}
        preferences={preferences}
        groups={groups}
      />
    </div>
  );
}

function KindChips({
  catalog,
  preferences,
  groups,
}: FilterProps & { groups: readonly ObjectGroup[] }): React.JSX.Element {
  return (
    <div aria-label="Object kinds" className={styles.chips}>
      {groups.map((group) => (
        <Button
          key={group.id}
          size="sm"
          variant="ghost"
          aria-label={`Filter ${group.label}`}
          aria-pressed={preferences.kinds.includes(group.id)}
          onClick={() => {
            writeSidebarPreferences(catalog.profileId, {
              kinds: toggled(preferences.kinds, group.id),
            });
          }}
        >
          {group.label}
        </Button>
      ))}
    </div>
  );
}

function SchemaFilter({
  catalog,
  preferences,
}: FilterProps): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const schemas =
    catalog.schemas.status === 'ready'
      ? catalog.schemas.data.filter(
          (schema) => catalog.systemSchemas || !schema.system,
        )
      : [];
  return (
    <details
      onToggle={(event) => {
        setOpen(event.currentTarget.open);
      }}
    >
      <summary>Filter schemas</summary>
      {open && (
        <select
          multiple
          aria-label="Filter schemas"
          value={[...preferences.schemas]}
          onChange={(event) => {
            writeSidebarPreferences(catalog.profileId, {
              schemas: Array.from(
                event.target.selectedOptions,
                (option) => option.value,
              ),
            });
          }}
        >
          {schemas.map((schema) => (
            <option key={schema.name}>{schema.name}</option>
          ))}
        </select>
      )}
      {open && (
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            writeSidebarPreferences(catalog.profileId, { schemas: [] });
          }}
        >
          Use current schema
        </Button>
      )}
    </details>
  );
}

function SectionMenu({
  catalog,
  preferences,
  groups,
}: FilterProps & { groups: readonly ObjectGroup[] }): React.JSX.Element {
  const [point, setPoint] = useState<{ x: number; y: number } | null>(null);
  const sections = [
    { id: 'favourites', label: 'Favourites' },
    { id: 'recent', label: 'Recent' },
    ...groups,
  ];
  return (
    <>
      <Button
        size="sm"
        variant="ghost"
        aria-haspopup="menu"
        onClick={(event) => {
          const box = event.currentTarget.getBoundingClientRect();
          setPoint({ x: box.left, y: box.bottom });
        }}
      >
        Sidebar sections
      </Button>
      {point && (
        <PointMenu
          {...point}
          label="Sidebar sections"
          onClose={() => {
            setPoint(null);
          }}
        >
          {sections.map((section) => (
            <button
              key={section.id}
              type="button"
              role="menuitemcheckbox"
              aria-checked={!preferences.hidden.includes(section.id)}
              onClick={() => {
                writeSidebarPreferences(catalog.profileId, {
                  hidden: toggled(preferences.hidden, section.id),
                });
              }}
            >
              {section.label}
            </button>
          ))}
        </PointMenu>
      )}
    </>
  );
}
