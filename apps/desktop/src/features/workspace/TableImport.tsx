// SPDX-License-Identifier: Apache-2.0
import { ImportSheet } from '../import';
import { sessionOf } from '../connections';
import type { Connections } from '../connections';
import type { ObjectRef } from '../schema';

/** Where an import writes: a database and schema, maybe one of its tables. */
export type ImportTarget = {
  database: string;
  schema: string;
  table: string | null;
};

/**
 * The import target of a tree object, else of the active table tab, else a
 * new table in the selected schema; `null` without a database and schema.
 */
export function importTarget(
  object: ObjectRef | undefined,
  tab: { database: string; schema: string; name: string } | undefined,
  selected: { database: string | null; schema: string },
): ImportTarget | null {
  const database = object?.database ?? tab?.database ?? selected.database;
  const schema = object?.schema ?? tab?.schema ?? selected.schema;
  if (!database || !schema) return null;
  return { database, schema, table: object?.name ?? tab?.name ?? null };
}

/** Opens the import sheet for a tree object, else as `importTarget` decides. */
export function importOpener(
  selected: { database: string; schema: string },
  tab: { database: string; schema: string; name: string } | undefined,
  open: (target: ImportTarget) => void,
): (object?: ObjectRef) => void {
  return (object) => {
    const target = importTarget(object, tab, selected);
    if (target) open(target);
  };
}

/** File ▸ Import… and the tree's Import…: the sheet for an open database. */
export function TableImport(props: {
  connections: Connections;
  target: ImportTarget;
  onClose: () => void;
}): React.JSX.Element | null {
  const profile = props.connections.active;
  if (!profile) return null;
  const sessionId = sessionOf(
    props.connections.statusOf(profile.id),
    props.target.database,
  );
  if (!sessionId) return null;
  return (
    <ImportSheet
      context={{
        ...props.target,
        profileId: profile.id,
        sessionId,
        readOnly: profile.safety === 'readOnly',
      }}
      onClose={props.onClose}
    />
  );
}
