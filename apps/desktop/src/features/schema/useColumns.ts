// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { tableStructure } from '@openrows/ipc';
import type { ObjectDto, TableColumnDto } from '@openrows/ipc';
import type { Resource, Resources } from './useResources';

/** Lazily read relation columns and the tree rows expanded to show them. */
export type ColumnTree = {
  columns: (object: ObjectDto) => Resource<TableColumnDto[]>;
  loadColumns: (object: ObjectDto) => void;
  expanded: (object: ObjectDto) => boolean;
  setExpanded: (object: ObjectDto, expanded: boolean) => void;
};

const path = (object: ObjectDto): string =>
  `${object.schema}
${object.kind}
${object.name}`;

/**
 * Columns are cached under `scope` (connection generation and database), so
 * a catalog refresh drops them; expanding a row whose read failed retries
 * it. Expansion is kept per connection and
 * database for the app session, across refreshes and reconnects.
 */
export function useColumns(
  resources: Resources,
  scope: string,
  target: { profileId: string | null; database: string },
): ColumnTree {
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const { profileId, database } = target;
  const owner = `${profileId ?? ''}
${database}
`;
  const key = (object: ObjectDto): string => `${scope}columns
${path(object)}
`;
  return {
    columns: (object) => resources.get(key(object)),
    loadColumns: (object) => {
      if (!profileId) return;
      const request = {
        profileId,
        database,
        schema: object.schema,
        table: object.name,
      };
      resources.load(key(object), () =>
        tableStructure(request).then((structure) => structure.columns),
      );
    },
    expanded: (object) => open.has(owner + path(object)),
    setExpanded: (object, expanded) => {
      if (expanded && resources.get(key(object)).status === 'error')
        resources.invalidate(key(object));
      setOpen((current) => {
        const next = new Set(current);
        if (expanded) next.add(owner + path(object));
        else next.delete(owner + path(object));
        return next;
      });
    },
  };
}
