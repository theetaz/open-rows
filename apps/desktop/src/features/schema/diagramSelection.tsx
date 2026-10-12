// SPDX-License-Identifier: Apache-2.0
import { useEffect, useSyncExternalStore } from 'react';
import type { Catalog } from './useCatalog';
import type { ObjectDto } from '@openrows/ipc';

type Selection = {
  profileId: string;
  database: string;
  names: readonly string[];
};
const selections = new Map<string, Selection>();
const listeners = new Set<() => void>();
const empty: readonly string[] = [];
const keyOf = (catalog: Catalog): string =>
  JSON.stringify([catalog.profileId, catalog.database, catalog.schema]);
const subscribe = (listener: () => void): (() => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

/** The current schema's checked tables, never leaking into another connection or schema. */
export function selectedDiagramTables(catalog: Catalog): readonly string[] {
  return selections.get(keyOf(catalog))?.names ?? empty;
}

/** Independent checkboxes keep ordinary tree focus and preview behavior intact. */
export function DiagramTableCheck(props: {
  catalog: Catalog;
  object: ObjectDto;
}): React.JSX.Element | null {
  const selected = useSyncExternalStore(subscribe, () =>
    selectedDiagramTables(props.catalog),
  );
  if (props.object.kind !== 'table') return null;
  const name = props.object.name;
  return (
    <input
      type="checkbox"
      aria-label={`Select ${name} for ER diagram`}
      checked={selected.includes(name)}
      onClick={(event) => {
        event.stopPropagation();
      }}
      onKeyDown={(event) => {
        event.stopPropagation();
      }}
      onChange={(event) => {
        const next = event.target.checked
          ? [...selected, name]
          : selected.filter((item) => item !== name);
        selections.set(keyOf(props.catalog), {
          profileId: props.catalog.profileId,
          database: props.catalog.database,
          names: next,
        });
        listeners.forEach((listener) => {
          listener();
        });
      }}
    />
  );
}

/** Drop transient checks when this database tree is closed or replaced. */
export function useDiagramSelection(catalog: Catalog): void {
  const { profileId, database } = catalog;
  useEffect(
    () => () => {
      for (const [key, selected] of selections) {
        if (selected.profileId === profileId && selected.database === database)
          selections.delete(key);
      }
      listeners.forEach((listener) => {
        listener();
      });
    },
    [profileId, database],
  );
}
